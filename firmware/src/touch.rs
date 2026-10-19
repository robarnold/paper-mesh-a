//! H752-01 touch adapter and event queue.

use crate::display::NATIVE_SCREEN_SIZE;
use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::{Instant, Timer};
use embedded_graphics::prelude::Point;
use esp_hal::{
    Blocking,
    delay::Delay,
    gpio::{Event as GpioEvent, Flex, InputConfig, Io, Level, Output, OutputConfig},
    i2c::master::I2c,
    interrupt::Priority,
    peripherals,
    system::Cpu,
};
use esp_rtos::embassy::InterruptExecutor;
use log::{error, info};
use paper_mesh_gt911::{CONFIG_BYTES, Event, Gt911};
use paper_mesh_platform::{TouchEvent, rotation::PortraitRotation};
use static_cell::StaticCell;

use crate::bus::Bus;

const ROTATION: PortraitRotation = PortraitRotation::new(NATIVE_SCREEN_SIZE);
static PIN: Mutex<CriticalSectionRawMutex, RefCell<Option<Flex<'static>>>> =
    Mutex::new(RefCell::new(None));
static IRQ: Signal<CriticalSectionRawMutex, Instant> = Signal::new();
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static EVENTS: Channel<CriticalSectionRawMutex, Sample, 32> = Channel::new();
static EXECUTOR: StaticCell<InterruptExecutor<2>> = StaticCell::new();
static STATS: Mutex<CriticalSectionRawMutex, RefCell<Statistics>> =
    Mutex::new(RefCell::new(Statistics::new()));

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub event: TouchEvent,
    pub irq_ms: u64,
    pub service_us: u64,
    pub bus_wait_us: u64,
    pub during_refresh: bool,
}

#[derive(Clone, Copy, Debug)]
pub struct Statistics {
    pub interrupts: u32,
    pub downs: u32,
    pub ups: u32,
    pub during_refresh: u32,
    pub errors: u32,
    pub overflows: u32,
    pub max_service_us: u64,
    pub max_bus_wait_us: u64,
}

impl Statistics {
    const fn new() -> Self {
        Self {
            interrupts: 0,
            downs: 0,
            ups: 0,
            during_refresh: 0,
            errors: 0,
            overflows: 0,
            max_service_us: 0,
            max_bus_wait_us: 0,
        }
    }
}

pub fn statistics() -> Statistics {
    STATS.lock(|stats| *stats.borrow())
}

pub fn next_event() -> Option<Sample> {
    EVENTS.try_receive().ok()
}

pub async fn changed() {
    CHANGED.wait().await;
}

pub fn start(
    bus: &'static Bus,
    reset: peripherals::GPIO9<'static>,
    interrupt: peripherals::GPIO3<'static>,
    io_mux: peripherals::IO_MUX<'static>,
    software_interrupt: peripherals::FROM_CPU_INTR2<'static>,
) {
    assert!(Cpu::current() == Cpu::ProCpu);
    let touch = Touch::new(&mut bus.try_lock().unwrap(), reset, interrupt);
    let Some((touch, pin)) = touch else {
        return;
    };
    PIN.lock(|slot| slot.borrow_mut().replace(pin));
    Io::new(io_mux).set_interrupt_handler(interrupt_handler);
    EXECUTOR
        .init(InterruptExecutor::new(software_interrupt))
        .start(Priority::Priority2)
        .spawn(worker(touch, bus).unwrap());
}

#[esp_hal::handler]
fn interrupt_handler() {
    PIN.lock(|slot| {
        if let Some(pin) = slot.borrow_mut().as_mut()
            && pin.is_interrupt_set()
        {
            let now = Instant::now();
            pin.unlisten();
            pin.clear_interrupt();
            IRQ.signal(now);
        }
    });
}

fn arm_interrupt() {
    PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let pin = slot.as_mut().unwrap();
        pin.clear_interrupt();
        // A pending low level triggers immediately, including after a bus wait.
        pin.listen(GpioEvent::LowLevel);
    });
}

pub struct Touch {
    driver: Gt911,
    _reset: Output<'static>,
    failed: bool,
}

impl Touch {
    pub fn new(
        bus: &mut I2c<'static, Blocking>,
        reset: peripherals::GPIO9<'static>,
        interrupt: peripherals::GPIO3<'static>,
    ) -> Option<(Self, Flex<'static>)> {
        let mut reset = Output::new(reset, Level::High, OutputConfig::default());
        let mut interrupt = Flex::new(interrupt);
        interrupt.set_input_enable(true);
        interrupt.apply_input_config(&InputConfig::default());
        Delay::new().delay_millis(100);
        let mut driver = Gt911::probe(bus);
        if driver.is_none() {
            for address in [0x5d_u8, 0x14] {
                let mut identity = [0; 11];
                let result = bus.write_read(address, &[0x81, 0x40], &mut identity);
                info!(
                    "touch: probe 0x{:02x}: {:?}; identity={:02x?}",
                    address, result, identity
                );
            }
            // Goodix Rev.10, section 4.2: select address 0x5d during reset.
            reset.set_low();
            Delay::new().delay_millis(10);
            interrupt.set_low();
            interrupt.set_output_enable(true);
            Delay::new().delay_millis(1);
            reset.set_high();
            Delay::new().delay_millis(6);
            interrupt.set_low();
            Delay::new().delay_millis(50);
            interrupt.set_output_enable(false);
            Delay::new().delay_millis(50);
            info!("touch: reset completed; address selection=0x5d");
            driver = Gt911::probe(bus);
        }
        let Some(driver) = driver else {
            error!("touch: GT911 not detected at 0x5d or 0x14");
            return None;
        };
        if !matches!((driver.width, driver.height), (540, 960) | (960, 540)) {
            error!(
                "touch: unsupported geometry {}x{}",
                driver.width, driver.height
            );
            return None;
        }
        let mut config = [0; CONFIG_BYTES];
        if let Err(error) = driver.read_config(bus, &mut config) {
            error!("touch: configuration read failed: {:?}", error);
            return None;
        }
        info!(
            "touch: GT911 address=0x{:02x}; firmware=0x{:04x}; vendor={}; raw={}x{}",
            driver.address, driver.firmware, driver.vendor, driver.width, driver.height
        );
        info!("touch: original config 0x8047..0x8100={:02x?}", config);
        if config[0x804d - 0x8047] & 3 != 2 {
            error!("touch: low-level interrupt mode required; configuration unchanged");
            return None;
        }
        Some((
            Self {
                driver,
                _reset: reset,
                failed: false,
            },
            interrupt,
        ))
    }

    fn poll(&mut self, bus: &mut I2c<'static, Blocking>) -> Option<TouchEvent> {
        let result = self.driver.poll(bus);
        if let Some(pressed) = self.driver.take_key_state() {
            crate::buttons::home(Some(pressed), Instant::now().as_millis());
        }
        let event = match result {
            Ok(event) => {
                self.failed = false;
                event?
            }
            Err(error) => {
                crate::buttons::home(None, Instant::now().as_millis());
                STATS.lock(|stats| stats.borrow_mut().errors += 1);
                if self.failed {
                    return None;
                }
                error!("touch: report failed: {:?}; gesture canceled", error);
                self.failed = true;
                EVENTS.clear();
                Event::Cancel
            }
        };
        Some(match event {
            Event::Touch(contact) | Event::Release(contact) => {
                let point = Point::new(i32::from(contact.x), i32::from(contact.y));
                let native = if self.driver.width == 540 {
                    ROTATION
                        .to_native(point)
                        .expect("validated portrait contact")
                } else {
                    point
                };
                ROTATION.touch_to_portrait(match event {
                    Event::Touch(_) => TouchEvent::touch(contact.id, native),
                    _ => TouchEvent::release(contact.id, native),
                })
            }
            Event::Cancel => TouchEvent::Cancel,
        })
    }
}

#[embassy_executor::task]
async fn worker(mut touch: Touch, bus: &'static Bus) {
    assert!(Cpu::current() == Cpu::ProCpu);
    info!("touch: interrupt worker started; core=0; GPIO3 low level; priority=2");
    loop {
        arm_interrupt();
        let irq = IRQ.wait().await;
        let wait_start = Instant::now();
        let mut guard = bus.lock().await;
        let bus_wait_us = wait_start.elapsed().as_micros();
        let during_refresh = crate::display_worker::is_refreshing();
        let event = touch.poll(&mut guard);
        drop(guard);
        let service_us = irq.elapsed().as_micros();
        STATS.lock(|stats| {
            let mut stats = stats.borrow_mut();
            stats.interrupts += 1;
            stats.max_service_us = stats.max_service_us.max(service_us);
            stats.max_bus_wait_us = stats.max_bus_wait_us.max(bus_wait_us);
            if let Some(event) = event {
                stats.downs += u32::from(matches!(event, TouchEvent::Touch(_)));
                stats.ups += u32::from(matches!(event, TouchEvent::Release(_)));
                stats.during_refresh += u32::from(during_refresh);
            }
        });
        if let Some(event) = event {
            let mut sample = Sample {
                event,
                irq_ms: irq.as_millis(),
                service_us,
                bus_wait_us,
                during_refresh,
            };
            if EVENTS.try_send(sample).is_err() {
                EVENTS.clear();
                touch.driver.cancel();
                sample.event = TouchEvent::Cancel;
                EVENTS.try_send(sample).unwrap();
                STATS.lock(|stats| stats.borrow_mut().overflows += 1);
            }
            CHANGED.signal(());
        }
        // Goodix Rev.10, section 5: allow 1 ms before a report retry.
        // The next level interrupt also catches reports that arrive during this delay.
        Timer::after_millis(1).await;
    }
}
