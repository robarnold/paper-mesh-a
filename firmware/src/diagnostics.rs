//! Retain a panic backtrace while USB OTG prevents Serial/JTAG panic output.

use core::sync::atomic::{AtomicU32, Ordering};

const PANIC_MARKER: u32 = 0x5041_4e49;
const FRAME_COUNT: usize = 12;

#[esp_hal::ram(unstable(rtc_slow, persistent))]
static mut MARKER: u32 = 0;
#[esp_hal::ram(unstable(rtc_slow, persistent))]
static mut FRAMES: [u32; FRAME_COUNT] = [0; FRAME_COUNT];

fn marker() -> &'static AtomicU32 {
    // SAFETY: This aligned word has only atomic accesses while firmware runs.
    unsafe { AtomicU32::from_ptr(&raw mut MARKER) }
}

fn frame(index: usize) -> &'static AtomicU32 {
    assert!(index < FRAME_COUNT);
    // SAFETY: The index is bounded. Each aligned word has only atomic accesses.
    unsafe { AtomicU32::from_ptr((&raw mut FRAMES).cast::<u32>().add(index)) }
}

pub fn report_previous_panic() {
    if marker().load(Ordering::Acquire) == PANIC_MARKER {
        marker().store(0, Ordering::Release);
        log::error!("previous panic: retained return addresses follow");
        for index in 0..FRAME_COUNT {
            let address = frame(index).load(Ordering::Acquire);
            if address != 0 {
                log::error!("previous panic: 0x{:08x}", address);
            }
        }
    }
}

#[unsafe(no_mangle)]
extern "C" fn custom_pre_backtrace() {
    let backtrace = esp_backtrace::Backtrace::capture();
    for index in 0..FRAME_COUNT {
        frame(index).store(
            backtrace
                .frames()
                .get(index)
                .map_or(0, |frame| frame.program_counter() as u32),
            Ordering::Release,
        );
    }
    marker().store(PANIC_MARKER, Ordering::Release);
}
