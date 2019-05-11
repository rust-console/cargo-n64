#![warn(rust_2018_idioms)]
#![no_std]

// Pull panic into scope
// Required by panic_handler
pub use rrt0::panic;

use n64lib::{ipl3font, vi};

// Colors are 5:5:5:1 RGB with a 16-bit color depth.
const WHITE: u16 = 0b11111_11111_11111_1;

fn main() {
    vi::init();

    ipl3font::draw_str_centered(WHITE, "Hello, world!");
    vi::swap_buffer();

    loop {}
}
