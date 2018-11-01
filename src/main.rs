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
                let hw_frame = (stack_ptr as *const u8)
                    .offset(core::mem::size_of::<SoftwareStackFrame>() as isize)
                    as *const ExceptionFrame;
                let hw_frame = hw_frame.as_ref().unwrap();
                //let hw_frame = core::ptr::read_volatile(hw_frame);
                if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
                    writeln!(hstdout, "ExceptionFrame: {:?}", hw_frame);
                }
                cortex_m::register::psp::write(stack_ptr as u32);
                self.state = TaskState::Running;
            }
            TaskState::Running => panic!("Task was left in state Running!"),
        }
        load_software_frame();
    }

    pub fn stack_okay(&self) -> bool {
        let canary = unsafe { core::ptr::read_volatile(&self.stack.canary as *const u32) };
        canary == STACK_CANARY_VALUE
    }

    pub unsafe fn save_context(&mut self) {
        save_software_frame();
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

    // configures the system timer to trigger a SysTick exception every second
    syst.set_clock_source(SystClkSource::Core);
    //syst.set_reload(16_000_000); // period = 1s
    syst.set_reload(320_000);
    syst.enable_counter();
    syst.enable_interrupt();

    #[allow(clippy::empty_loop)]
    loop {}
}

static mut STDOUT: Option<HStdout> = None;

fn hello_world() -> ! {
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            write!(hstdout, ".");
            cortex_m::asm::delay(1_000_000);
        }
    }
}

fn hallo_chinees() -> ! {
    loop {
        if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
            write!(hstdout, "O");
            cortex_m::asm::delay(1_000_000);
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

const NROF_TASKS: usize = 1;

#[exception]
fn SysTick() {
    static mut TASK_INDEX: usize = 0;
    static mut TASKS: [Option<Task>; NROF_TASKS] = [None];

    if let Some(ref mut task) = TASKS[*TASK_INDEX % NROF_TASKS] {
        unsafe { task.save_context() };
        *TASK_INDEX = *TASK_INDEX + 1;
    } else {
        TASKS[0] = Some(Task::new(nothing));
        //TASKS[1] = Some(Task::new(nothing));
        //TASKS[2] = Some(Task::new(stack_filler));
        //TASKS[2] = Some(Task::new(fibonacci_task));
    }

    unsafe {
        if STDOUT.is_none() {
            STDOUT = Some(hio::hstdout().unwrap());
        }
    }

    if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
        writeln!(hstdout, "Scheduling task {}", *TASK_INDEX);

        if let Some(ref mut task) = TASKS[*TASK_INDEX % NROF_TASKS] {
            unsafe { task.schedule_now() };
            writeln!(hstdout, "Scheduled!");
        } else {
            writeln!(hstdout, "Task does not exist");
        }
    }

    let a = EXC_RETURN_THREAD_PSP;
    unsafe { asm!("bx $0\n\r" :: "r"(a)) };
}

/*
#[exception]
fn SysTick() {
    static mut COUNT: usize = 0;
    unsafe {
        if STDOUT.is_none() {
            STDOUT = Some(hio::hstdout().unwrap());
        }
    }

    if let Some(hstdout) = unsafe { STDOUT.as_mut() } {
        writeln!(hstdout, "Tick {}!", COUNT);
        *COUNT += 1;
    }
}
*/

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
