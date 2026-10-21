//! H752-01 controls. Reset and QON have no readable button signal.

use core::cell::RefCell;
use embassy_sync::{
    blocking_mutex::{Mutex, raw::CriticalSectionRawMutex},
    channel::Channel,
    signal::Signal,
};
use embassy_time::Instant;
use esp_hal::gpio::{Event, Input, InputConfig, Pull};
use esp_hal::peripherals::GPIO0;
use paper_mesh_platform::input::buttons::{
    ButtonCapability, ButtonDebouncer, ButtonDescriptor, ButtonEvent, ButtonId, ButtonState,
};

use crate::bus::Bus;

pub const BOOT: ButtonId = ButtonId(0);
pub const FUNCTION: ButtonId = ButtonId(1);
pub const HOME: ButtonId = ButtonId(2);
pub const CONTROLS: [ButtonDescriptor; 5] = [
    ButtonDescriptor {
        id: BOOT,
        label: "BOOT",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: FUNCTION,
        label: "Function",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: HOME,
        label: "Home",
        capability: ButtonCapability::Edges,
    },
    ButtonDescriptor {
        id: ButtonId(3),
        label: "Reset",
        capability: ButtonCapability::Reset,
    },
    ButtonDescriptor {
        id: ButtonId(4),
        label: "QON",
        capability: ButtonCapability::PowerRecovery,
    },
];

static EVENTS: Channel<CriticalSectionRawMutex, ButtonEvent, 16> = Channel::new();
static CHANGED: Signal<CriticalSectionRawMutex, ()> = Signal::new();
static BOOT_PIN: Mutex<CriticalSectionRawMutex, RefCell<Option<Input<'static>>>> =
    Mutex::new(RefCell::new(None));
static HOME_STATE: Mutex<CriticalSectionRawMutex, RefCell<ButtonDebouncer>> =
    Mutex::new(RefCell::new(ButtonDebouncer::new(HOME, 0)));

fn publish(event: ButtonEvent) {
    if EVENTS.try_send(event).is_err() {
        EVENTS.clear();
        for id in [BOOT, FUNCTION, HOME] {
            let _ = EVENTS.try_send(ButtonEvent {
                id,
                at_ms: event.at_ms,
                state: ButtonState::Cancel,
            });
        }
    }
    CHANGED.signal(());
}

pub fn home(pressed: Option<bool>, at_ms: u64) {
    let event = HOME_STATE.lock(|state| match pressed {
        Some(pressed) => state.borrow_mut().sample(pressed, at_ms),
        None => state.borrow_mut().cancel(at_ms),
    });
    if let Some(event) = event {
        publish(event);
    }
}

pub fn next_event() -> Option<ButtonEvent> {
    EVENTS.try_receive().ok()
}
pub async fn changed() {
    CHANGED.wait().await;
}

pub fn arm_boot_wake() -> Result<(), paper_mesh_platform::suspend::SuspendError> {
    use paper_mesh_platform::suspend::SuspendError;
    BOOT_PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let pin = slot.as_mut().ok_or(SuspendError::Preparation)?;
        if pin.is_low() {
            return Err(SuspendError::WakeActive);
        }
        pin.clear_interrupt();
        pin.listen(Event::LowLevel);
        Ok(())
    })
}

pub fn take_boot_interrupt(disarm: bool) -> bool {
    BOOT_PIN.lock(|slot| {
        let mut slot = slot.borrow_mut();
        let Some(pin) = slot.as_mut() else {
            return false;
        };
        let pending = pin.is_interrupt_set();
        if pending || disarm {
            pin.unlisten();
            pin.clear_interrupt();
        }
        pending
    })
}

#[embassy_executor::task]
pub async fn worker(bus: Option<&'static Bus>, pin: GPIO0<'static>) {
    use crate::suspend::{self, Service};
    suspend::register(Service::Buttons);
    BOOT_PIN.lock(|slot| {
        *slot.borrow_mut() = Some(Input::new(pin, InputConfig::default().with_pull(Pull::Up)))
    });
    let mut boot_state = ButtonDebouncer::new(BOOT, 20);
    let mut function = ButtonDebouncer::new(FUNCTION, 20);
    let mut configured = false;
    for control in CONTROLS {
        log::info!("button: {} {:?}", control.label, control.capability);
    }
    loop {
        suspend::checkpoint(Service::Buttons).await;
        let now = Instant::now().as_millis();
        let pressed = BOOT_PIN.lock(|slot| slot.borrow().as_ref().is_some_and(Input::is_low));
        if let Some(event) = boot_state.sample(pressed, now) {
            publish(event);
        }
        if let Some(bus) = bus {
            let sample = {
                let mut guard = bus.lock().await;
                read_function(&mut *guard, &mut configured)
            };
            let event = match sample {
                Ok(pressed) => function.sample(pressed, Instant::now().as_millis()),
                Err(_) => {
                    configured = false;
                    function.cancel(now)
                }
            };
            if let Some(event) = event {
                publish(event);
            }
        }
        suspend::delay(Service::Buttons, 10).await;
    }
}

fn read_function<I: embedded_hal::i2c::I2c>(
    bus: &mut I,
    configured: &mut bool,
) -> Result<bool, I::Error> {
    let mut value = [0];
    if !*configured {
        // Preserve display outputs and all other expander bits.
        bus.write_read(0x20, &[7], &mut value)?;
        bus.write(0x20, &[7, value[0] | 4])?;
        bus.write_read(0x20, &[5], &mut value)?;
        bus.write(0x20, &[5, value[0] & !4])?;
        *configured = true;
    }
    bus.write_read(0x20, &[1], &mut value)?;
    Ok(value[0] & 4 == 0)
}
