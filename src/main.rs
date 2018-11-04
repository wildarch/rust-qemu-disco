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
const PSR_DEFAULT: u32 = 0x2100_0000;
const EXC_RETURN_THREAD_PSP: u32 = 0xFFFF_FFFD;
const STACK_CANARY_VALUE: u32 = 0xDEADBEEF;

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
    canary: u32,
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
                canary: STACK_CANARY_VALUE,
                data: [0; STACK_SIZE],
                sw_stack_frame: SoftwareStackFrame::default(),
                hw_stack_frame: ExceptionFrame {
                    r0: 0,
                    r1: 0,
                    r2: 0,
                    r3: 0,
                    r12: 0,
                    pc: func as usize as u32, // Clippy will warn about direct cast
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
        cortex_m::asm::dsb();
        //load_software_frame();
    }

    pub fn stack_okay(&self) -> bool {
        let canary = unsafe { core::ptr::read_volatile(&self.stack.canary as *const u32) };
        canary == STACK_CANARY_VALUE
    }

    pub unsafe fn save_context(&mut self) {
        let stack_ptr = cortex_m::register::psp::read() as *mut SoftwareStackFrame;
        self.state = TaskState::Suspended(stack_ptr);
        if !self.stack_okay() {
            panic!("Stack corrupt!");
        }
    }
}

#[entry]
fn main() -> ! {
    let p = Peripherals::take().unwrap();
    let mut syst = p.SYST;

    syst.set_clock_source(SystClkSource::Core);
    syst.set_reload(16_000_000); // period = 1s
    syst.enable_counter();
    syst.enable_interrupt();

    #[allow(clippy::empty_loop)]
    loop {}
}

static mut STDOUT: Option<HStdout> = None;

fn hello_world() -> ! {
    let mut i: i32 = 0;
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            writeln!(hstdout, ". {}", i);
            i += 1;
            for _ in 0..50 {
                cortex_m::asm::delay(1_000_000);
            }
        }
    }
}

fn hallo_chinees() -> ! {
    let mut i: i32 = 0;
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            writeln!(hstdout, "O {}", i);
            i -= 1;
            for _ in 0..50 {
                cortex_m::asm::delay(1_000_000);
            }
        }
    }
}

fn nothing() -> ! {
    loop {}
}

fn stack_filler() -> ! {
    let mut data = [0u16; STACK_SIZE / 8];

    loop {
        for (i, entry) in data.iter_mut().enumerate() {
            *entry = i as u16;
        }
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            write!(hstdout, "{}, ", data[data.len() - 1]);
            cortex_m::asm::delay(1_000_000);
        }
    }
}

#[exception]
fn SysTick() {
    unsafe {
        save_software_frame();
        cortex_m::asm::dsb();
        context_switcher();
        cortex_m::asm::dsb();
        load_software_frame();
        if cfg!(debug_assertions) {
            // Restores the stack to the original state
            // and jumps to the EXC_RETURN address for User mode PSP.
            // Note that this is a dangerous tactic, as it assumes a
            // certain stack size for this function.
            asm!("
                 add sp, 16\n\r
                 bx $0\n\r" :: "r"(EXC_RETURN_THREAD_PSP) :: "volatile")
        } else {
            asm!("
                 add sp, 8\n\r
                 bx $0\n\r" :: "r"(EXC_RETURN_THREAD_PSP) :: "volatile")
        }
    };
}

const NROF_TASKS: usize = 2;

#[inline(never)]
unsafe fn context_switcher() {
    static mut TASK_INDEX: usize = 0;
    static mut TASKS: [Option<Task>; NROF_TASKS] = [None, None];

    if let Some(ref mut task) = TASKS[TASK_INDEX] {
        if let TaskState::Running = task.state {
            task.save_context();
            TASK_INDEX = (TASK_INDEX + 1) % NROF_TASKS;
        } else {
            panic!("Task was not running!");
        }
    } else {
        TASKS[0] = Some(Task::new(hallo_chinees));
        TASKS[1] = Some(Task::new(hello_world));
    }

    if STDOUT.is_none() {
        STDOUT = Some(hio::hstdout().unwrap());
    }

    if let Some(hstdout) = STDOUT.as_mut() {
        if let Some(ref mut task) = TASKS[TASK_INDEX] {
            task.schedule_now();
            writeln!(hstdout, "\nScheduled task: {}", TASK_INDEX);
        } else {
            writeln!(hstdout, "Task does not exist");
        }
    }
}

#[exception]
fn DefaultHandler(i: i16) {
    panic!("Default handler called! ({})", i);
}

#[inline(always)]
unsafe fn save_software_frame() {
    let _tmp: u32;
    asm!("mrs $0, psp \n\t
          stmfd $0!, {r4-r11}\n\r
          msr psp, $0\n\t" : "=r"(_tmp) ::: "volatile");
}

#[inline(always)]
unsafe fn load_software_frame() {
    let _tmp: u32;
    asm!("mrs $0, psp \n\t
          ldmfd $0!, {r4-r11}\n\t
          msr psp, $0\n\t" : "=r"(_tmp) ::: "volatile");
}
