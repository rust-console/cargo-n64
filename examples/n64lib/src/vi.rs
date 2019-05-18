//! Video Interface
//!
//! Provides low level access to the N64 vi hardware.

use core::ptr::read_volatile;

// TODO: Heap allocate (needs std and global_allocator)
const FRAME_BUFFER: *mut u16 = 0xA010_0000 as *mut u16;

pub const WIDTH: usize = 320;
pub const HEIGHT: usize = 240;
pub const FRAME_BUFFER_SIZE: usize = WIDTH * HEIGHT * 2;

const VI_BASE: usize = 0xA440_0000;

const VI_STATUS: *mut u32 = VI_BASE as *mut u32;
const VI_DRAM_ADDR: *mut usize = (VI_BASE + 0x04) as *mut usize;
const VI_H_WIDTH: *mut u32 = (VI_BASE + 0x08) as *mut u32;
const VI_V_INTR: *mut u32 = (VI_BASE + 0x0C) as *mut u32;
const VI_CURRENT: *const u32 = (VI_BASE + 0x10) as *const u32;
const VI_TIMING: *mut u32 = (VI_BASE + 0x14) as *mut u32;
const VI_V_SYNC: *mut u32 = (VI_BASE + 0x18) as *mut u32;
const VI_H_SYNC: *mut u32 = (VI_BASE + 0x1C) as *mut u32;
const VI_H_SYNC_LEAP: *mut u32 = (VI_BASE + 0x20) as *mut u32;
const VI_H_VIDEO: *mut u32 = (VI_BASE + 0x24) as *mut u32;
const VI_V_VIDEO: *mut u32 = (VI_BASE + 0x28) as *mut u32;
const VI_V_BURST: *mut u32 = (VI_BASE + 0x2C) as *mut u32;
const VI_X_SCALE: *mut u32 = (VI_BASE + 0x30) as *mut u32;
const VI_Y_SCALE: *mut u32 = (VI_BASE + 0x34) as *mut u32;

const VIDEO_MODE: *const u32 = 0x8000_0300 as *const u32;

pub enum VideoMode {
    PAL,
    NTSC,
    MPAL,
}

/// Video frequency in Hertz
pub fn get_video_frequency() -> u32 {
    match get_video_mode() {
        VideoMode::PAL => 49_656_530,
        VideoMode::NTSC => 48_681_812,
        VideoMode::MPAL => 48_628_316,
    }
}

/// Returns the current video mode
pub fn get_video_mode() -> VideoMode {
    match unsafe { read_volatile(VIDEO_MODE) } {
        0 => VideoMode::PAL,
        1 => VideoMode::NTSC,
        _ => VideoMode::MPAL,
    }
}

/// Busy-wait for VBlank
pub fn wait_for_ready() {
    loop {
        let current_halfline = unsafe { read_volatile(VI_CURRENT) };
        if current_halfline <= 10 {
            break;
        }
    }
}

/// Return a raw pointer to the back buffer
pub fn next_buffer() -> *mut u16 {
    let current_fb = unsafe { read_volatile(VI_DRAM_ADDR) };

    if current_fb & 0xFFFFF != 0 {
        FRAME_BUFFER
    } else {
        (FRAME_BUFFER as usize + FRAME_BUFFER_SIZE) as *mut u16
    }
}

/// Swap frame buffers (display the back buffer)
pub fn swap_buffer() {
    unsafe {
        *VI_DRAM_ADDR = next_buffer() as usize;
    }
}

/// Initialize Video Interface with 320x240x16 resolution and double buffering
pub fn init() {
    // Clear both frame buffers to black, writing two pixels at a time
    let frame_buffer = FRAME_BUFFER as usize;
    for i in 0..WIDTH * HEIGHT {
        let p = (frame_buffer + i * 4) as *mut u32;
        unsafe {
            *p = 0x0001_0001;
        }
    }

    // Initialize VI
    unsafe {
        *VI_STATUS = 0x0000_320E;
        *VI_DRAM_ADDR = frame_buffer;
        *VI_H_WIDTH = WIDTH as u32;
        *VI_V_INTR = 2;
        *VI_TIMING = 0x03E5_2239;
        *VI_V_SYNC = 0x0000_020D;
        *VI_H_SYNC = 0x0000_0C15;
        *VI_H_SYNC_LEAP = 0x0C15_0C15;
        *VI_H_VIDEO = 0x006C_02EC;
        *VI_V_VIDEO = 0x0025_01FF;
        *VI_V_BURST = 0x000E_0204;
        *VI_X_SCALE = 0x0000_0200;
        *VI_Y_SCALE = 0x0000_0400;
    }
}
