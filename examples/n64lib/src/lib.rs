#![warn(rust_2018_idioms)]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(asm)]

mod allocator;
pub mod ipl3font;
mod lock;
pub mod vi;
