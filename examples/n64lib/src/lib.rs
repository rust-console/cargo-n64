#![warn(rust_2018_idioms)]
#![no_std]
#![feature(alloc_error_handler)]
#![feature(asm)]

#[cfg(target_vendor = "nintendo64")]
mod allocator;
pub mod ipl3font;
#[cfg(target_vendor = "nintendo64")]
mod lock;
pub mod util;
pub mod vi;
