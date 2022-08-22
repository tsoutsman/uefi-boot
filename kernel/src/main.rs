#![feature(core_intrinsics)]
#![no_std]
#![no_main]

#[no_mangle]
pub extern "C" fn _start() -> ! {
    unsafe { core::arch::asm!("mov x1, #0xbeef") };
    #[allow(clippy::empty_loop)]
    loop {}
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::intrinsics::abort();
}
