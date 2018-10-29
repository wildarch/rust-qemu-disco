#![no_main]
#![no_std]
#![feature(asm)]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate cortex_m_semihosting;
extern crate panic_semihosting;

use core::fmt::Write;

use cortex_m::peripheral::syst::SystClkSource;
use cortex_m::Peripherals;
use cortex_m_rt::{entry, exception, ExceptionFrame};
use cortex_m_semihosting::hio::{self, HStdout};

const STACK_SIZE: usize = 1024;
const PSR_DEFAULT: u32 = 0x21000000;
const EXC_RETURN_THREAD_PSP: u32 = 0xFFFFFFFD;

#[repr(C)]
#[derive(Default)]
struct SoftwareStackFrame {
    r4: u32,
    r5: u32,
    r6: u32,
    r7: u32,
    r8: u32,
    r9: u32,
    r10: u32,
    r11: u32,
}

#[repr(C)]
struct Stack {
    data: [u8; STACK_SIZE],
    // This only holds when the task is started,
    // later frames will have data after the frames.
    sw_stack_frame: SoftwareStackFrame,
    hw_stack_frame: ExceptionFrame,
}

#[derive(Debug)]
enum TaskState {
    Created,
    Running,
    Suspended(*mut SoftwareStackFrame),
}

struct Task {
    state: TaskState,
    stack: Stack,
}

impl Task {
    pub fn new(func: fn() -> !) -> Task {
        Task {
            state: TaskState::Created,
            stack: Stack {
                data: [0; STACK_SIZE],
                sw_stack_frame: SoftwareStackFrame::default(),
                hw_stack_frame: ExceptionFrame {
                    r0: 0,
                    r1: 0,
                    r2: 0,
                    r3: 0,
                    r12: 0,
                    pc: func as u32,
                    // TODO point to task cleanup function
                    lr: 0,
                    xpsr: PSR_DEFAULT,
                },
            },
        }
    }

    pub unsafe fn schedule_now(&mut self) {
        match self.state {
            TaskState::Created => {
                let stack_ptr = &mut self.stack.sw_stack_frame as *mut SoftwareStackFrame;
                cortex_m::register::psp::write(stack_ptr as u32);
                self.state = TaskState::Running;
            }
            TaskState::Suspended(stack_ptr) => {
                cortex_m::register::psp::write(stack_ptr as u32);
                self.state = TaskState::Running;
            }
            TaskState::Running => panic!("Task was left in state Running!"),
        }
        load_software_frame();
    }

    pub unsafe fn save_context(&mut self) {
        save_software_frame();
        let stack_ptr = cortex_m::register::psp::read() as *mut SoftwareStackFrame;
        self.state = TaskState::Suspended(stack_ptr)
    }
}

#[entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let mut syst = p.SYST;

    // configures the system timer to trigger a SysTick exception every second
    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(16_000_000); // period = 1s
    syst.enable_counter();
    syst.enable_interrupt();

    loop {}
}

static mut STDOUT: Option<HStdout> = None;

fn hello_world() -> ! {
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            writeln!(hstdout, "Hello, world!");
        }
    }
}

fn hallo_chinees() -> ! {
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            writeln!(hstdout, "Hallo, chinees?");
        }
    }
}

#[exception]
fn SysTick() {
    static mut TASK_INDEX: usize = 0;
    static mut TASKS: [Option<Task>; 2] = [None, None];

    if let Some(ref mut task) = TASKS[*TASK_INDEX] {
        unsafe { task.save_context() };
        *TASK_INDEX = (*TASK_INDEX + 1) % 2;
    } else {
        TASKS[0] = Some(Task::new(hello_world));
        TASKS[1] = Some(Task::new(hallo_chinees));
    }

    unsafe {
        if STDOUT.is_none() {
            STDOUT = Some(hio::hstdout().unwrap());
        }
    }

    if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
        writeln!(hstdout, "Tick!");
        writeln!(hstdout, "Scheduling task {}", *TASK_INDEX);

        if let Some(ref mut task) = TASKS[*TASK_INDEX] {
            unsafe { task.schedule_now() };
        } else {
            writeln!(hstdout, "Task does not exist");
        }
    }
    cortex_m::asm::isb();
    let a = EXC_RETURN_THREAD_PSP;
    unsafe { asm!("bx $0\n\r" :: "r"(a)) };
}

unsafe fn save_software_frame() {
    let _tmp: u32;
    asm!("mrs $0, psp \n\t
          stmfd $0!, {r4-r11}\n\r
          msr psp, $0\n\t" : "=r"(_tmp));
}

unsafe fn load_software_frame() {
    let _tmp: u32;
    asm!("mrs $0, psp \n\t
          ldmfd $0!, {r4-r11}\n\t
          msr psp, $0\n\t" : "=r"(_tmp));
}
