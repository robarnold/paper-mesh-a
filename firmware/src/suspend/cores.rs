//! Stop core 1 at a lock-free point before core 0 enters light sleep.

use core::sync::atomic::Ordering;
use embassy_sync::{blocking_mutex::raw::CriticalSectionRawMutex, signal::Signal};
use embassy_time::{Instant, Timer};
use esp_hal::{
    peripherals::CPU_CTRL,
    rtc_cntl::{
        WakeLock,
        sleep::{LowPower, RtcSleepConfig},
    },
    system::{Cpu, CpuControl},
};
use paper_mesh_platform::suspend::SuspendError;

use super::{
    WAKE_BITS,
    park::Park,
    trace::{self, Stage},
};

static PARK: Park = Park::new();
static REQUEST: Signal<CriticalSectionRawMutex, ()> = Signal::new();

pub async fn request_park() -> Result<(), SuspendError> {
    if !PARK.request() {
        return Err(SuspendError::Busy);
    }
    REQUEST.signal(());
    while !PARK.parked() {
        Timer::after_millis(1).await;
    }
    Ok(())
}

pub async fn park_requested() {
    while !PARK.requested() {
        REQUEST.wait().await;
    }
}

#[esp_hal::ram]
pub fn park() {
    assert!(Cpu::current() == Cpu::AppCpu);
    // Mask this core only. A global critical section would retain a shared lock.
    xtensa_lx::interrupt::free(|| {
        if !PARK.claim() {
            return;
        }
        while PARK.parked() {
            core::hint::spin_loop();
        }
        PARK.finish();
    });
}

pub fn release() {
    PARK.release();
}

pub fn sleep(low_power: &mut LowPower<'_>, deadline: Instant) -> Result<bool, SuspendError> {
    assert!(Cpu::current() == Cpu::ProCpu);
    assert!(PARK.parked());
    trace::mark(Stage::CoreParked);
    xtensa_lx::interrupt::free(|| {
        WakeLock::release();
        let result = if WakeLock::is_active() {
            Err(SuspendError::WakeLocked)
        } else {
            // SAFETY: The RTOS uses its ordinary idle hook. Only this coordinator controls
            // the stall registers after core 1 acknowledges its lock-free stop point.
            let mut control = CpuControl::new(unsafe { CPU_CTRL::steal() });
            // SAFETY: This function runs on core 0. Core 1 has interrupts masked and holds no lock.
            unsafe { control.park_core(Cpu::AppCpu) };
            low_power.set_wakeup_deadline(
                esp_hal::time::Instant::EPOCH
                    + esp_hal::time::Duration::from_micros(deadline.as_micros()),
            );
            trace::mark(Stage::EnteringSleep);
            let entered = WAKE_BITS.load(Ordering::Acquire) == 0 && Instant::now() < deadline;
            if entered {
                low_power.sleep_light(RtcSleepConfig::default());
            }
            trace::mark(Stage::SleepReturned);
            low_power.clear_wakeup_deadline();
            super::disarm();
            control.unpark_core(Cpu::AppCpu);
            Ok(entered)
        };
        WakeLock::acquire();
        // TIMG0 stops during sleep. Rearm it against the HAL's restored monotonic time.
        esp_rtos::rearm_alarm();
        trace::mark(Stage::AlarmRestored);
        result
    })
}
