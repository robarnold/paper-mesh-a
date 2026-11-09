//! H752-01 touch adapter and event queue.
//!
//! # Design choices
//!
//! - Preserve contact identities across touch reports and cancel uncertain sequences
//!   so a missed sample cannot complete a gesture.
//! - Box the worker future once during preparation so its bus capacity does not determine the public task type.
//!
//! See [ESP touch integration](../../../docs/adapters/touch.md).

mod input_lifecycle;
pub use input_lifecycle::TouchInputLifecycle;

use crate::interrupt::GpioInterruptHandler;
use alloc::boxed::Box;
use core::{
    cell::RefCell,
    sync::atomic::{AtomicBool, Ordering},
};
use core::{future::Future, pin::Pin};
use embassy_executor::{SendSpawner, SpawnError};
use embassy_futures::select::{Either, select};
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
    gpio::{Event as GpioEvent, Flex, InputConfig, Level, Output, OutputConfig},
    i2c::master::I2c,
    peripherals,
    system::Cpu,
};
use log::{error, info};
use paper_mesh_gt911::{CONFIG_BYTES, Event, Frame, Gt911};
use paper_mesh_platform::worker::{Pause, PauseWorker};
use paper_mesh_platform::{TouchEvent, input::contacts::TouchFrame, rotation::PortraitRotation};
use static_cell::StaticCell;

use paper_mesh_platform::bus::Arbitrated;
type Bus<const N: usize> = Arbitrated<CriticalSectionRawMutex, I2c<'static, Blocking>, N>;

/// Runtime connections for touch geometry, logical HOME input, wake capture, and diagnostics.
pub struct Config {
    pub rotation: PortraitRotation,
    /// Called from the touch worker. None cancels uncertain HOME input.
    pub did_observe_home: fn(Option<bool>, u64),
    /// Called in GPIO interrupt context. Must not wait or access devices.
    pub did_wake: fn(),
    /// Read the retained wake result after lifecycle restoration.
    pub was_wake_source: fn() -> bool,
    /// Read display activity for diagnostics without waiting.
    pub is_display_refreshing: fn() -> bool,
}

struct InterruptPin {
    pin: Flex<'static>,
    did_wake: fn(),
}

static PIN: Mutex<CriticalSectionRawMutex, RefCell<Option<InterruptPin>>> =
    Mutex::new(RefCell::new(None));
static IRQ: Signal<CriticalSectionRawMutex, Instant> = Signal::new();
static WAKE_ARMED: AtomicBool = AtomicBool::new(false);
static WAKE_READY: AtomicBool = AtomicBool::new(false);
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static EVENTS: Channel<CriticalSectionRawMutex, Sample, 32> = Channel::new();
static INPUT_PAUSE: StaticCell<Pause<CriticalSectionRawMutex>> = StaticCell::new();
static STATS: Mutex<CriticalSectionRawMutex, RefCell<Statistics>> =
    Mutex::new(RefCell::new(Statistics::new()));

#[derive(Clone, Copy, Debug)]
pub struct Sample {
    pub frame: TouchFrame,
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

/// Prepared touch resources. Register the interrupt before launching the worker.
#[must_use]
pub struct Prepared {
    pub interrupt: GpioInterruptHandler,
    pub workers: Workers,
    pub lifecycle: TouchInputLifecycle,
}

#[must_use]
pub struct Workers {
    future: Pin<Box<dyn Future<Output = ()> + Send + 'static>>,
}

impl Workers {
    /// Launch all prepared touch work on the selected executor.
    pub fn spawn(self, spawner: SendSpawner) -> Result<(), SpawnError> {
        spawner.spawn(worker(self)?);
        Ok(())
    }
}

#[embassy_executor::task]
async fn worker(workers: Workers) {
    workers.future.await;
}

pub fn prepare<const N: usize>(
    bus: &'static Bus<N>,
    reset: peripherals::GPIO9<'static>,
    interrupt: peripherals::GPIO3<'static>,
    settings: Config,
) -> Option<Prepared> {
    assert!(Cpu::current() == Cpu::ProCpu);
    let did_wake = settings.did_wake;
    let (touch, pin) = Touch::new(&mut bus.try_lock().unwrap(), reset, interrupt, settings)?;
    PIN.lock(|slot| slot.borrow_mut().replace(InterruptPin { pin, did_wake }));
    let (pause, worker_pause) = INPUT_PAUSE.init(Pause::new()).split();
    Some(Prepared {
        interrupt: GpioInterruptHandler::new(interrupt_handler),
        workers: Workers {
            future: Box::pin(run(touch, bus, worker_pause)),
        },
        lifecycle: TouchInputLifecycle { pause },
    })
}

fn interrupt_handler() {
    PIN.lock(|slot| {
        if let Some(InterruptPin { pin, did_wake }) = slot.borrow_mut().as_mut()
            && pin.is_interrupt_set()
        {
            let now = Instant::now();
            pin.unlisten();
            pin.clear_interrupt();
            if WAKE_ARMED.load(Ordering::Acquire) {
                did_wake();
            } else {
                IRQ.signal(now);
            }
        }
    });
}

pub fn arm_wake() -> Result<(), paper_mesh_platform::suspend::SuspendError> {
    use paper_mesh_platform::suspend::SuspendError;
    PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let pin = &mut slot.as_mut().ok_or(SuspendError::Unsupported)?.pin;
        if !WAKE_READY.load(Ordering::Acquire) || pin.is_low() {
            return Err(SuspendError::WakeActive);
        }
        pin.clear_interrupt();
        WAKE_ARMED.store(true, Ordering::Release);
        pin.listen(GpioEvent::LowLevel);
        Ok(())
    })
}

pub fn disarm_wake() -> bool {
    PIN.lock(|slot| {
        let armed = WAKE_ARMED.swap(false, Ordering::AcqRel);
        if let Some(InterruptPin { pin, .. }) = slot.borrow_mut().as_mut()
            && armed
        {
            let pending = pin.is_interrupt_set();
            pin.unlisten();
            pin.clear_interrupt();
            pending
        } else {
            false
        }
    })
}

fn arm_interrupt() {
    PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let pin = &mut slot.as_mut().unwrap().pin;
        pin.clear_interrupt();
        // A pending low level triggers immediately, including after a bus wait.
        pin.listen(GpioEvent::LowLevel);
    });
}

pub struct Touch {
    settings: Config,
    driver: Gt911,
    _reset: Output<'static>,
    failed: bool,
    home_pressed: bool,
}

impl Touch {
    pub fn new(
        bus: &mut I2c<'static, Blocking>,
        reset: peripherals::GPIO9<'static>,
        interrupt: peripherals::GPIO3<'static>,
        settings: Config,
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
        let portrait = settings.rotation.size();
        let dimensions = (u32::from(driver.width), u32::from(driver.height));
        if dimensions != (portrait.width, portrait.height)
            && dimensions != (portrait.height, portrait.width)
        {
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
        for (index, bytes) in config.chunks(16).enumerate() {
            info!("touch: config 0x{:04x}={:02x?}", 0x8047 + index * 16, bytes);
        }
        info!(
            "touch: configured contact limit={}; key enabled={}",
            config[5] & 15,
            config[7] & 1 != 0
        );
        if config[0x804d - 0x8047] & 3 != 2 {
            error!("touch: low-level interrupt mode required; configuration unchanged");
            return None;
        }
        Some((
            Self {
                settings,
                driver,
                _reset: reset,
                failed: false,
                home_pressed: false,
            },
            interrupt,
        ))
    }

    fn poll(&mut self, bus: &mut I2c<'static, Blocking>, at_ms: u64) -> Option<TouchFrame> {
        let result = self.driver.poll(bus);
        if let Some(pressed) = self.driver.take_key_state() {
            self.home_pressed = pressed;
            (self.settings.did_observe_home)(Some(pressed), Instant::now().as_millis());
        }
        let report = match result {
            Ok(event) => {
                self.failed = false;
                event?
            }
            Err(error) => {
                (self.settings.did_observe_home)(None, Instant::now().as_millis());
                STATS.lock(|stats| stats.borrow_mut().errors += 1);
                if self.failed {
                    return None;
                }
                error!("touch: report failed: {:?}; gesture canceled", error);
                self.failed = true;
                EVENTS.clear();
                Frame::canceled()
            }
        };
        let mut frame = TouchFrame::new(at_ms);
        for event in report.events() {
            frame.push(match event {
                Event::Touch(contact) | Event::Release(contact) => {
                    let point = Point::new(i32::from(contact.x), i32::from(contact.y));
                    let native =
                        if u32::from(self.driver.width) == self.settings.rotation.size().width {
                            self.settings
                                .rotation
                                .to_native(point)
                                .expect("validated portrait contact")
                        } else {
                            point
                        };
                    self.settings.rotation.touch_to_portrait(match event {
                        Event::Touch(_) => TouchEvent::touch(contact.id, native),
                        _ => TouchEvent::release(contact.id, native),
                    })
                }
                Event::Cancel => TouchEvent::Cancel,
            });
        }
        Some(frame)
    }
}

async fn run<const N: usize>(
    mut touch: Touch,
    bus: &'static Bus<N>,
    mut pause: PauseWorker<'static, CriticalSectionRawMutex>,
) {
    assert!(Cpu::current() == Cpu::ProCpu);
    info!("touch: worker started; core=0; GPIO3 low level");
    loop {
        arm_interrupt();
        let irq = match select(pause.requested(), IRQ.wait()).await {
            Either::First(()) => {
                PIN.lock(|slot| {
                    if let Some(InterruptPin { pin, .. }) = slot.borrow_mut().as_mut() {
                        pin.unlisten();
                        pin.clear_interrupt();
                    }
                });
                EVENTS.clear();
                let contacts_idle = touch.driver.contacts_idle();
                WAKE_READY.store(contacts_idle && !touch.home_pressed, Ordering::Release);
                pause.checkpoint().await;
                let pending = PIN.lock(|slot| {
                    slot.borrow()
                        .as_ref()
                        .is_some_and(|state| state.pin.is_low())
                });
                if !contacts_idle || pending || (touch.settings.was_wake_source)() {
                    // Consume contacts from suspend until an empty report confirms release.
                    touch.driver.cancel();
                }
                IRQ.reset();
                continue;
            }
            Either::Second(irq) => irq,
        };
        let wait_start = Instant::now();
        let mut guard = bus.lock().await;
        let bus_wait_us = wait_start.elapsed().as_micros();
        let during_refresh = (touch.settings.is_display_refreshing)();
        let frame = touch.poll(&mut guard, irq.as_millis());
        drop(guard);
        let service_us = irq.elapsed().as_micros();
        STATS.lock(|stats| {
            let mut stats = stats.borrow_mut();
            stats.interrupts += 1;
            stats.max_service_us = stats.max_service_us.max(service_us);
            stats.max_bus_wait_us = stats.max_bus_wait_us.max(bus_wait_us);
            if let Some(frame) = frame {
                for event in frame.events() {
                    stats.downs += u32::from(matches!(event, TouchEvent::Touch(_)));
                    stats.ups += u32::from(matches!(event, TouchEvent::Release(_)));
                }
                stats.during_refresh += u32::from(during_refresh);
            }
        });
        if let Some(frame) = frame {
            let mut sample = Sample {
                frame,
                irq_ms: irq.as_millis(),
                service_us,
                bus_wait_us,
                during_refresh,
            };
            if EVENTS.try_send(sample).is_err() {
                EVENTS.clear();
                touch.driver.cancel();
                sample.frame = TouchFrame::single(TouchEvent::Cancel, irq.as_millis());
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
