pub struct InterruptLock(u32);

/// Enter a critical section by disabling interrupts. The lock is released when it is dropped.
/// This lock is re-entrant (may be used recursively).
pub fn interrupt_lock() -> InterruptLock {
    let mut sr: u32;

    unsafe {
        asm!("
            mfc0    $0,$$12
            and     $$8,$1
            mtc0    $$8,$$12"
            : "=r"(sr)
            : "i"(!1)
            : "$8"
            : "volatile"
        );
    }

    InterruptLock(sr)
}

impl Drop for InterruptLock {
    fn drop(&mut self) {
        unsafe {
            asm!("
                mtc0    $0,$$12"
                // todo: Hazards? Two nops after this? ^
                :
                : "r"(self.0)
                :
                : "volatile"
            );
        }
    }
}
