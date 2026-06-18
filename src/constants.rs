//! Shared constants for the emulator core.

pub const SCREEN_WIDTH: usize = 160;
pub const SCREEN_HEIGHT: usize = 144;

pub const FLAG_Z_BIT: u8 = 7;
pub const FLAG_N_BIT: u8 = 6;
pub const FLAG_H_BIT: u8 = 5;
pub const FLAG_C_BIT: u8 = 4;

pub const FLAG_Z: u8 = 1 << FLAG_Z_BIT;
pub const FLAG_N: u8 = 1 << FLAG_N_BIT;
pub const FLAG_H: u8 = 1 << FLAG_H_BIT;
pub const FLAG_C: u8 = 1 << FLAG_C_BIT;

pub const VERTICAL_BLANK_SCAN_LINE: u8 = 0x90;
pub const VERTICAL_BLANK_SCAN_LINE_MAX: u8 = 0x99;
pub const RETRACE_START: i32 = 456;

/// Number of machine cycles emulated per rendered frame.
pub const TARGET_CYCLES: u32 = 70221;

pub type Screen = [[[u8; 3]; SCREEN_WIDTH]; SCREEN_HEIGHT];

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum Colour {
    White,
    LightGray,
    DarkGray,
    Black,
}
