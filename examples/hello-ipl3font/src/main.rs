#![deny(clippy::all)]
#![forbid(unsafe_code)]
#![no_std]

// Pull panic into scope
// Required by panic_handler
#[cfg(not(test))]
pub use rrt0;

use n64lib::{ipl3font, vi};

// Colors are 5:5:5:1 RGB with a 16-bit color depth.
#[allow(clippy::unusual_byte_groupings)]
const WHITE: u16 = 0b11111_11111_11111_1;

fn main() {
    vi::init();

    ipl3font::draw_str_centered(WHITE, "Hello, world!");
    vi::swap_buffer();
}
