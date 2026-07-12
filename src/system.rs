//! Timers, interrupts, joypad and LCD status.

use crate::Emulator;
use crate::constants::{RETRACE_START, VERTICAL_BLANK_SCAN_LINE};

impl Emulator {
    #[inline]
    pub(crate) fn do_input(&mut self) {}

    /// TIMA increments on the falling edge of a divider bit. The trigger bit
    /// depends on the TAC frequency select: 512, 8, 32 or 128 T-cycles.
    const TIMA_TRIGGER_BITS: [u64; 4] = [512, 8, 32, 128];

    #[inline]
    fn advance_divider(&mut self, cycles: u32) {
        let old = self.div_counter;
        let new = old.wrapping_add(cycles as u64);
        self.div_counter = new;

        // The visible DIV register is the high byte of the 16-bit divider.
        self.rom[0xFF04] = (new >> 8) as u8;

        let tac = self.rom[0xFF07];
        if tac & 0x04 != 0 {
            let period = 2 * Self::TIMA_TRIGGER_BITS[(tac & 0x03) as usize];
            let edges = (new / period).wrapping_sub(old / period);
            for _ in 0..edges {
                let overflow = self.rom[0xFF05] == 0xFF;
                self.rom[0xFF05] = if overflow {
                    self.rom[0xFF06]
                } else {
                    self.rom[0xFF05].wrapping_add(1)
                };
                if overflow {
                    self.request_interrupt(2);
                }
            }
        }
    }

    pub(crate) fn do_timers(&mut self, cycles: u32) {
        self.advance_divider(cycles);
    }

    /// Move the timer forward to a memory access happening partway through an
    /// instruction. The cycles are recorded so `update` does not apply them
    /// twice when it advances the timer for the whole instruction.
    #[inline]
    pub(crate) fn advance_timer_to_access(&mut self, cycles: u32) {
        self.timer_pre_advanced += cycles;
        self.advance_divider(cycles);
    }

    pub(crate) fn do_interrupts(&mut self) {
        let pending = self.rom[0xFF0F] & self.rom[0xFFFF];

        // HALT wakes up when an enabled interrupt is flagged, even when
        // interrupts are globally disabled; the handler only runs with IME=1.
        if self.halted && pending != 0 {
            self.halted = false;
        }

        if !self.enable_interrupts {
            return;
        }

        for bit in 0..8u8 {
            let mask = 1 << bit;
            if pending & mask != 0 {
                self.service_interrupt(bit);
                break;
            }
        }
    }

    fn service_interrupt(&mut self, number: u8) {
        self.push_word_onto_stack(self.pc);
        self.halted = false;
        self.pc = match number {
            0 => 0x40, // vertical blank
            1 => 0x48, // LCD status
            2 => 0x50, // timer
            4 => 0x60, // joypad
            _ => return,
        };
        self.enable_interrupts = false;
        self.rom[0xFF0F] &= !(1 << number);
    }

    #[inline]
    pub(crate) fn request_interrupt(&mut self, bit: u8) {
        self.rom[0xFF0F] |= 1 << bit;
    }

    #[inline]
    pub(crate) fn issue_vertical_blank(&mut self) {
        self.request_interrupt(0);
    }

    /// The joypad register is driven by the direction/button select lines.
    #[inline]
    pub(crate) fn get_joypad_state(&self) -> u8 {
        let mut result = self.rom[0xFF00] ^ 0xFF;
        if result & (1 << 4) == 0 {
            let upper = (self.joypad_state >> 4) | 0xF0;
            result &= upper;
        } else if result & (1 << 5) == 0 {
            let lower = (self.joypad_state & 0xF) | 0xF0;
            result &= lower;
        }
        result
    }

    pub(crate) fn on_key_pressed(&mut self, key: u32) {
        let previously_unset = self.joypad_state & (1 << key) == 0;
        self.joypad_state &= !(1 << key);

        let button = key > 3;
        let key_request = self.rom[0xFF00];
        let request = if button {
            key_request & (1 << 5) == 0
        } else {
            key_request & (1 << 4) == 0
        };

        if request && !previously_unset {
            self.request_interrupt(4);
        }
    }

    #[inline]
    pub(crate) fn on_key_released(&mut self, key: u32) {
        self.joypad_state |= 1 << key;
    }

    #[inline]
    pub(crate) fn get_lcd_mode(&self) -> u8 {
        self.rom[0xFF41] & 0x3
    }

    pub(crate) fn set_lcd_status(&mut self) {
        let mut lcd_status = self.rom[0xFF41];

        if self.rom[0xFF40] & 0x80 == 0 {
            self.retrace_ly = RETRACE_START;
            self.rom[0xFF44] = 0;
            lcd_status &= 0xFC;
            lcd_status |= 1;
            self.rom[0xFF41] = lcd_status;
            return;
        }

        let ly = self.rom[0xFF44];
        let current_mode = self.get_lcd_mode();
        let mode: u8;
        let mut request_interrupt = false;

        if ly >= VERTICAL_BLANK_SCAN_LINE {
            mode = 1;
            lcd_status |= 1;
            lcd_status &= !2;
            request_interrupt = lcd_status & (1 << 4) != 0;
        } else {
            let mode2_boundary = RETRACE_START - 80;
            let mode3_boundary = mode2_boundary - 172;

            if self.retrace_ly >= mode2_boundary {
                mode = 2;
                lcd_status |= 2;
                lcd_status &= !1;
                request_interrupt = lcd_status & (1 << 5) != 0;
            } else if self.retrace_ly >= mode3_boundary {
                mode = 3;
                lcd_status |= 2;
                lcd_status |= 1;
            } else {
                mode = 0;
                lcd_status &= !2;
                lcd_status &= !1;
                request_interrupt = lcd_status & (1 << 3) != 0;
            }
        }

        if request_interrupt && current_mode != mode {
            self.request_interrupt(1);
        }

        if ly == self.rom[0xFF45] {
            lcd_status |= 4;
            if lcd_status & (1 << 6) != 0 {
                self.request_interrupt(1);
            }
        } else {
            lcd_status &= !4;
        }

        self.rom[0xFF41] = lcd_status;
    }
}
