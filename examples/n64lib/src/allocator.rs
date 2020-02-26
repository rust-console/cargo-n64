use crate::lock::interrupt_lock;
use core::alloc::{GlobalAlloc, Layout};

#[link(name = "c")]
extern "C" {
    fn malloc(size: usize) -> *mut u8;
    fn free(ptr: *mut u8);
}

struct N64LibAlloc;

unsafe impl GlobalAlloc for N64LibAlloc {
    unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
        let _lock = interrupt_lock();

        malloc(layout.size())
    }

    unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
        let _lock = interrupt_lock();

        free(ptr);
    }
}

#[global_allocator]
static GLOBAL: N64LibAlloc = N64LibAlloc;

#[alloc_error_handler]
fn alloc_error(_layout: Layout) -> ! {
    loop {}
}

extern "C" {
    static __bss_end: u8;
}

// Newlib's malloc implementation calls sbrk when it needs more memory.
//
// sbrk() maintains a static pointer to the end of the program's image in memory (__bss_end), and
// increments it as needed.
//
// Shouldn't need any locking here since this function is only called from malloc().
#[no_mangle]
extern "C" fn sbrk(increment: usize) -> *const u8 {
    unsafe {
        static mut PTR: *const u8 = unsafe { &__bss_end };

        let prev = PTR;
        PTR = PTR.add(increment);

        prev
    }
}
