pub fn loop_forever() -> ! {
    loop {
        unsafe { asm!("" :::: "volatile") }
    }
}
