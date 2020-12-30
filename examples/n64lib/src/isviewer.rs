use core::fmt;
use core::ptr::{read_volatile, write_volatile};

struct Stream;

static mut STDOUT: Stream = Stream;
static mut STDERR: Stream = Stream;

const ISVIEWER: *mut u32 = 0xB3FF_0000 as *mut u32;
const SEND: usize = 5;
const BUF: usize = 8;

impl fmt::Write for Stream {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        print(s);
        Ok(())
    }
}

/// Check if Intelligent Systems Viewer 64 is available.
fn is_is64() -> bool {
    let magic = u32::from_ne_bytes(*b"IS64");
    unsafe {
        write_volatile(ISVIEWER, magic);
        read_volatile(ISVIEWER) == magic
    }
}

/// Print a string to IS Viewer 64.
fn print(string: &str) {
    let bytes = string.as_bytes();
    let base = ISVIEWER.wrapping_add(BUF);

    // Write one word at a time
    // It's ugly, but it optimizes really well!
    for (i, chunk) in bytes.chunks(4).enumerate() {
        let val = match *chunk {
            [a, b, c, d] => (a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8 | (d as u32),
            [a, b, c] => (a as u32) << 24 | (b as u32) << 16 | (c as u32) << 8,
            [a, b] => (a as u32) << 24 | (b as u32) << 16,
            [a] => (a as u32) << 24,
            _ => unreachable!(),
        };

        let ptr = base.wrapping_add(i);

        unsafe { write_volatile(ptr, val) };
    }

    // Write the string length
    let ptr = ISVIEWER.wrapping_add(SEND);

    unsafe { write_volatile(ptr, bytes.len() as u32) };
}

/// Initialize global I/O for IS Viewer 64.
pub fn init() {
    // Safe because the mutable borrow is only used while global STDOUT/STDERR is locked
    // and the local Stream type is private.
    if is_is64() {
        unsafe {
            rrt0::io::STDOUT.set_once(|| &mut STDOUT).unwrap();
            rrt0::io::STDERR.set_once(|| &mut STDERR).unwrap();
        }
    }
}
