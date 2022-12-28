 #![no_std]

use core::{arch::asm, sync::atomic::{Ordering, AtomicU32}};

static COUNTER_FREQUENCY: AtomicU32 = AtomicU32::new(0);

pub fn init() {
    let freq: u16;
    unsafe { asm!("mrs {}, CNTFID0", out(reg) freq) }; 
}
