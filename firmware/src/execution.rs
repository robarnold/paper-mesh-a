//! Board execution placement for adapter workers.
//!
//! # Design choices
//!
//! - Keep cores, software interrupts, priorities, and executor storage in the firmware runtime.
//! - Use interrupt executors for input that must progress during synchronous application rendering.
//! - Keep the display executor on core 1, where the suspend coordinator parks it before sleep.
//!
//! See [firmware execution placement](../../docs/platform/runtime.md#firmware-execution-placement).

use embassy_executor::{SendSpawner, Spawner};
use esp_hal::{
    interrupt::Priority,
    peripherals::{CPU_CTRL, FROM_CPU_INTR1, FROM_CPU_INTR2, FROM_CPU_INTR3},
    system::{Cpu, Stack},
};
use esp_rtos::embassy::{Executor, InterruptExecutor};
use static_cell::{ConstStaticCell, StaticCell};

static TOUCH_EXECUTOR: StaticCell<InterruptExecutor<2>> = StaticCell::new();
static GNSS_EXECUTOR: StaticCell<InterruptExecutor<3>> = StaticCell::new();
static DISPLAY_EXECUTOR: StaticCell<Executor> = StaticCell::new();
static DISPLAY_STACK: ConstStaticCell<Stack<8192>> = ConstStaticCell::new(Stack::new());

/// Own the board resources used to start each executor once.
pub struct Execution {
    touch_interrupt: Option<FROM_CPU_INTR2<'static>>,
    gnss_interrupt: Option<FROM_CPU_INTR3<'static>>,
    display_core: Option<(CPU_CTRL<'static>, FROM_CPU_INTR1<'static>)>,
}

impl Execution {
    pub fn new(
        cpu_control: CPU_CTRL<'static>,
        from_cpu_intr1: FROM_CPU_INTR1<'static>,
        from_cpu_intr2: FROM_CPU_INTR2<'static>,
        from_cpu_intr3: FROM_CPU_INTR3<'static>,
    ) -> Self {
        Self {
            touch_interrupt: Some(from_cpu_intr2),
            gnss_interrupt: Some(from_cpu_intr3),
            display_core: Some((cpu_control, from_cpu_intr1)),
        }
    }

    pub fn start_touch(&mut self) -> SendSpawner {
        assert!(Cpu::current() == Cpu::ProCpu);
        let interrupt = self
            .touch_interrupt
            .take()
            .expect("touch executor already started");
        TOUCH_EXECUTOR
            .init(InterruptExecutor::new(interrupt))
            .start(Priority::Priority2)
    }

    pub fn start_gnss(&mut self) -> SendSpawner {
        assert!(Cpu::current() == Cpu::ProCpu);
        let interrupt = self
            .gnss_interrupt
            .take()
            .expect("GNSS executor already started");
        GNSS_EXECUTOR
            .init(InterruptExecutor::new(interrupt))
            .start(Priority::Priority1)
    }

    pub fn start_display(&mut self, initialize: fn(Spawner)) {
        assert!(Cpu::current() == Cpu::ProCpu);
        let (cpu_control, interrupt) = self
            .display_core
            .take()
            .expect("display executor already started");
        esp_rtos::start_second_core(cpu_control, interrupt, DISPLAY_STACK.take(), move || {
            DISPLAY_EXECUTOR.init(Executor::new()).run(initialize);
        });
    }
}
