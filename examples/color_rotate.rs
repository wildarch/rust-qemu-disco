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
use cortex_m_semihosting::hio;

use stm32f4::stm32f407;

const DELAY: u32 = 100_000_000;

#[entry]
fn main() -> ! {
    let mut stdout = hio::hstdout().unwrap();
    writeln!(stdout, "Hello, blinky!").unwrap();

    let peripherals = stm32f407::Peripherals::take().unwrap();
    // Enable APB1 clock
    let rcc = peripherals.RCC;
    rcc.ahb1enr.write(|w| w.gpioden().enabled());

    let gpiod = peripherals.GPIOD;
    gpiod.moder.write(|w| {
        w.moder15()
            .output()
            .moder14()
            .output()
            .moder13()
            .output()
            .moder12()
            .output()
    });

    gpiod.ospeedr.write(|w| {
        w.ospeedr15()
            .high_speed()
            .ospeedr14()
            .high_speed()
            .ospeedr13()
            .high_speed()
            .ospeedr12()
            .high_speed()
    });

    gpiod.otyper.write(|w| {
        w.ot15()
            .push_pull()
            .ot14()
            .push_pull()
            .ot13()
            .push_pull()
            .ot12()
            .push_pull()
    });

    gpiod.pupdr.write(|w| {
        w.pupdr15()
            .pull_up()
            .pupdr14()
            .pull_up()
            .pupdr13()
            .pull_up()
            .pupdr12()
            .pull_up()
    });
    for _ in 0..10 {
        gpiod.bsrr.write(|w| w.bs15().set());
        asm::delay(DELAY);
        gpiod.bsrr.write(|w| w.br15().reset());

        gpiod.bsrr.write(|w| w.bs14().set());
        asm::delay(DELAY);
        gpiod.bsrr.write(|w| w.br14().reset());

        gpiod.bsrr.write(|w| w.bs13().set());
        asm::delay(DELAY);
        gpiod.bsrr.write(|w| w.br13().reset());

        gpiod.bsrr.write(|w| w.bs12().set());
        asm::delay(DELAY);
        gpiod.bsrr.write(|w| w.br12().reset());
    }

    loop {}
}
