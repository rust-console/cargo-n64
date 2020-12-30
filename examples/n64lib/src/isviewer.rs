use core::fmt;
use core::ptr::{read_volatile, write_volatile};

struct Stream;

static mut STDOUT: Stream = Stream;
static mut STDERR: Stream = Stream;

const IS64_MAGIC: *mut u32 = 0xB3FF_0000 as *mut u32;
const IS64_SEND: *mut u32 = 0xB3FF_0014 as *mut u32;
const IS64_BUFFER: *mut u32 = 0xB3FF_0020 as *mut u32;

// Rough estimate based on Cen64
const BUFFER_SIZE: usize = 0x1000 - 0x20;

impl fmt::Write for Stream {
    fn write_str(&mut self, s: &str) -> fmt::Result {
        print(s);
        Ok(())
    }
}

/// Check if Intelligent Systems Viewer 64 is available.
fn is_is64() -> bool {
    let magic = u32::from_be_bytes(*b"IS64");

    // SAFETY: It is always safe to read and write the magic value; static memory-mapped address.
    unsafe {
        write_volatile(IS64_MAGIC, magic);
        read_volatile(IS64_MAGIC) == magic
    }
}

/// Print a string to IS Viewer 64.
///
/// # Panics
///
/// Asserts that the maximum string length is just under 4KB.
fn print(string: &str) {
    assert!(string.len() < BUFFER_SIZE);

    let bytes = string.as_bytes();

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

        let ptr = IS64_BUFFER.wrapping_add(i);

        // SAFETY: Bounds checking has already been performed.
        unsafe { write_volatile(ptr, val) };
    }

    // Write the string length
    // SAFETY: It is always safe to write the length; static memory-mapped address.
    unsafe { write_volatile(IS64_SEND, bytes.len() as u32) };
}

/// Initialize global I/O for IS Viewer 64.
pub fn init() {
    // SAFETY: The mutable static is only accessed while the global STDOUT/STDERR lock is held,
    // and the local Stream type is private.
    if is_is64() {
        unsafe {
            rrt0::io::STDOUT.set_once(|| &mut STDOUT).unwrap();
            rrt0::io::STDERR.set_once(|| &mut STDERR).unwrap();
        }
    }
}
