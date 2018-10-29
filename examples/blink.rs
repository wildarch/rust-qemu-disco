#![no_main]
#![no_std]

extern crate cortex_m;
extern crate cortex_m_rt;
extern crate cortex_m_semihosting;
extern crate panic_halt;
extern crate stm32f4;

use core::fmt::Write;

use cortex_m::asm;
use cortex_m_rt::entry;
use cortex_m_semihosting::{debug, hio};

use stm32f4::stm32f407;

const DELAY: u32 = 1_000_000_000;

#[entry]
fn main() -> ! {
    let mut stdout = hio::hstdout().unwrap();
    writeln!(stdout, "Hello, blinky!").unwrap();

    let peripherals = stm32f407::Peripherals::take().unwrap();
    // Enable APB1 clock
    let rcc = peripherals.RCC;
    rcc.ahb1enr.write(|w| w.gpioden().enabled());

    let gpiod = peripherals.GPIOD;
    gpiod.moder.write(|w| w.moder15().output());
    gpiod.ospeedr.write(|w| w.ospeedr15().high_speed());
    gpiod.otyper.write(|w| w.ot15().push_pull());
    gpiod.pupdr.write(|w| w.pupdr15().pull_up());

    gpiod.bsrr.write(|w| w.bs15().set());

    asm::delay(DELAY);

    gpiod.bsrr.write(|w| w.br15().reset());

    asm::delay(DELAY);

    gpiod.bsrr.write(|w| w.bs15().set());

    asm::delay(DELAY);

    debug::exit(debug::EXIT_SUCCESS);

    loop {}
}
