//! IPL3 font renderer.
//!
//! Draws strings using the font embedded in the official Nintendo IPL3 bootcode.
//! See: <https://github.com/PeterLemon/N64/blob/master/BOOTCODE/IPL3Font.asm>

use crate::vi;

/// Glyph width (pixels or bits)
pub const WIDTH: usize = 13;

/// Glyph height (pixels or bits)
pub const HEIGHT: usize = 14;

/// Glyphs available in the embedded font
const GLYPHS: &[u8; 50] = br##"ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789!"#'*+,-./:=?@"##;

/// Glyph index for rendering "unknown" characters
/// Unknown characters are rendered with the "?" glyph
const UNKNOWN: usize = 48;

/// Glyph size (in bytes)
const GLYPH_SIZE: usize = 23;

/// Location of font in ROM
/// This is only guaranteed to be accurate for the CIC-NUS-6102 IPL3.
const GLYPH_ADDR: usize = 0xB000_0B70;

/// Font kerning (in pixels)
const KERNING: usize = 1;

/// Draw a string using the embedded font, centered on the frame buffer.
/// Only supports a small subset of the ASCII character set.
pub fn draw_str_centered(color: u16, string: &str) {
    let x = (vi::WIDTH - string.len() * WIDTH) / 2;
    let y = (vi::HEIGHT - HEIGHT) / 2;

    draw_str(x, y, color, string);
}

/// Draw a string using the embedded font.
/// Only supports a small subset of the ASCII character set.
pub fn draw_str(mut x: usize, mut y: usize, color: u16, string: &str) {
    for mut ch in string.bytes() {
        // Bail if we're trying to draw outside of the frame buffer
        if y >= vi::HEIGHT {
            return;
        }

        // Special handling for space characters
        if ch == b' ' {
            x += WIDTH;
            if x >= vi::WIDTH {
                x = 0;
                y += HEIGHT;
            }
            continue;
        }

        // Special handling for lowercase letters
        if (b'a'..=b'z').contains(&ch) {
            ch -= b'a' - b'A';
        }

        draw_char(x, y, color, ch);
        x += WIDTH + KERNING;
    }
}

/// Draw a character.
/// Only supports a small subset of the ASCII character set.
fn draw_char(x: usize, y: usize, color: u16, ch: u8) {
    let frame_buffer = vi::next_buffer() as usize;

    let index = GLYPHS.iter().position(|c| *c == ch).unwrap_or(UNKNOWN);

    let mut address = GLYPH_ADDR + index * GLYPH_SIZE;
    let mut shift = (4 - (address & 3)) * 8 - 1;
    address &= 0xFFFF_FFFC;
    let mut bits = unsafe { *(address as *const u32) };

    for yy in y..y + HEIGHT {
        // Bail if we're trying to draw outside of the frame buffer
        if yy >= vi::HEIGHT {
            return;
        }

        for xx in x..x + WIDTH {
            if (bits >> shift) & 1 == 1 && xx < vi::WIDTH {
                // Put a pixel into the frame buffer
                let offset = (yy * vi::WIDTH + xx) * 2;
                let p = (frame_buffer + offset) as *mut u16;

                unsafe {
                    *p = color;
                }
            }

            // Advance to the next glyph pixel
            if shift == 0 {
                address += 4;
                bits = unsafe { *(address as *const u32) };
                shift = 31;
            } else {
                shift -= 1;
            }
        }
    }
}
