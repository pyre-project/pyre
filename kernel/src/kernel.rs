#![no_std]
#![no_main]
#![feature(asm)]

mod drivers;
mod io;

use core::panic::PanicInfo;
use drivers::serial;

#[panic_handler]
fn panic(_info: &PanicInfo) -> ! {
    loop {}
}

#[no_mangle]
fn kernel_main() -> i32 {
    serial::safe_lock(|serial| {
        serial.data_port().write(b'X');
    });

    loop {}
}
