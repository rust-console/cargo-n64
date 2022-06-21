#![deny(clippy::all)]
#![no_main]
#![no_std]

use n64lib::{ipl3font, vi};

// Colors are 5:5:5:1 RGB with a 16-bit color depth.
#[allow(clippy::unusual_byte_groupings)]
const WHITE: u16 = 0b11111_11111_11111_1;

#[no_mangle]
fn main() {
    vi::init();

    ipl3font::draw_str_centered(WHITE, "Hello, world!");
    vi::swap_buffer();
}
