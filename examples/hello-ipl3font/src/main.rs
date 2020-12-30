#![deny(clippy::all)]
#![forbid(unsafe_code)]
#![no_std]

use n64lib::{ipl3font, isviewer, prelude::*, vi};

// Colors are 5:5:5:1 RGB with a 16-bit color depth.
#[allow(clippy::unusual_byte_groupings)]
const WHITE: u16 = 0b11111_11111_11111_1;

fn main() {
    println!("It is safe to print without a `Stream`, you just won't see this!");

    isviewer::init();

    println!("Now that the `isviewer::Stream` has been configured as our global STDOUT...");
    eprintln!("These macros work about how you expect!");
    println!();
    println!("Supports formatting: {:#06x}", WHITE);
    println!();

    vi::init();

    ipl3font::draw_str_centered(WHITE, "Hello, world!");

    vi::swap_buffer();

    panic!("Panic also works! :)");
}
