//! Pixel processing: scanline timing, background and sprite rendering.

use crate::Emulator;
use crate::constants::{
    Colour, RETRACE_START, SCREEN_WIDTH, VERTICAL_BLANK_SCAN_LINE,
    VERTICAL_BLANK_SCAN_LINE_MAX,
};

impl Emulator {
    pub(crate) fn do_graphics(&mut self, cycles: u32) {
        self.set_lcd_status();

        if self.read_memory(0xFF40) & 0x80 != 0 {
            self.retrace_ly -= cycles as i32;
        }

        if self.rom[0xFF44] > VERTICAL_BLANK_SCAN_LINE_MAX {
            self.rom[0xFF44] = 0;
        }

        if self.retrace_ly <= 0 {
            self.draw_current_line();
        }
    }

    pub(crate) fn draw_current_line(&mut self) {
        if self.read_memory(0xFF40) & 0x80 == 0 {
            return;
        }

        self.rom[0xFF44] = self.rom[0xFF44].wrapping_add(1);
        self.retrace_ly = RETRACE_START;

        let scan_line = self.rom[0xFF44];
        if scan_line == VERTICAL_BLANK_SCAN_LINE {
            self.issue_vertical_blank();
        }
        if scan_line > VERTICAL_BLANK_SCAN_LINE_MAX {
            self.rom[0xFF44] = 0;
        }
        if scan_line < VERTICAL_BLANK_SCAN_LINE {
            self.draw_scan_line();
        }
    }

    fn draw_scan_line(&mut self) {
        let lcd_control = self.read_memory(0xFF40);
        if lcd_control & 0x80 != 0 {
            self.render_background(lcd_control);
            self.render_sprites(lcd_control);
        }
    }

    fn get_colour(&self, colour_number: u8, address: u16) -> Colour {
        let palette = self.read_memory(address);
        let (high, low) = match colour_number {
            0 => (1, 0),
            1 => (3, 2),
            2 => (5, 4),
            3 => (7, 6),
            _ => unreachable!(),
        };
        let colour = ((palette >> high) & 1) << 1 | ((palette >> low) & 1);
        match colour {
            0 => Colour::White,
            1 => Colour::LightGray,
            2 => Colour::DarkGray,
            3 => Colour::Black,
            _ => unreachable!(),
        }
    }

    fn colour_bytes(colour: Colour) -> [u8; 3] {
        match colour {
            Colour::White => [255, 255, 255],
            Colour::LightGray => [0xCC, 0xCC, 0xCC],
            Colour::DarkGray => [0x77, 0x77, 0x77],
            Colour::Black => [0, 0, 0],
        }
    }

    fn render_background(&mut self, lcd_control: u8) {
        if lcd_control & 1 == 0 {
            return;
        }

        let tile_data: u16;
        let background_memory: u16;
        let mut unsigned = true;

        let scroll_y = self.read_memory(0xFF42);
        let scroll_x = self.read_memory(0xFF43);
        let window_y = self.read_memory(0xFF4A);
        let window_x = self.read_memory(0xFF4B).wrapping_sub(7);

        let using_window = lcd_control & (1 << 5) != 0 && window_y <= self.read_memory(0xFF44);

        if lcd_control & (1 << 4) != 0 {
            tile_data = 0x8000;
        } else {
            tile_data = 0x8800;
            unsigned = false;
        }

        if !using_window {
            background_memory = if lcd_control & (1 << 3) != 0 {
                0x9C00
            } else {
                0x9800
            };
        } else {
            background_memory = if lcd_control & (1 << 6) != 0 {
                0x9C00
            } else {
                0x9800
            };
        }

        let y_position = if !using_window {
            scroll_y.wrapping_add(self.read_memory(0xFF44))
        } else {
            self.read_memory(0xFF44).wrapping_sub(window_y)
        };
        let tile_row = ((y_position / 8) as u16) * 32;

        let line_y = self.read_memory(0xFF44) as usize;
        if line_y >= crate::constants::SCREEN_HEIGHT {
            return;
        }

        for pixel in 0..SCREEN_WIDTH {
            let mut x_position = (pixel as u8).wrapping_add(scroll_x);
            if using_window && pixel >= window_x as usize {
                x_position = (pixel as u8).wrapping_sub(window_x);
            }

            let tile_column = (x_position / 8) as u16;
            let tile_number: i16 = if unsigned {
                self.read_memory(background_memory.wrapping_add(tile_row + tile_column)) as i16
            } else {
                (self.read_memory(background_memory.wrapping_add(tile_row + tile_column)) as i8)
                    as i16
            };

            let offset = if unsigned {
                tile_number * 16
            } else {
                (tile_number + 128) * 16
            };
            let tile_location = tile_data.wrapping_add(offset as u16);

            let line = (y_position % 8) * 2;
            let data1 = self.read_memory(tile_location.wrapping_add(line as u16));
            let data2 = self.read_memory(tile_location.wrapping_add(line as u16 + 1));

            let colour_bit = 7 - (x_position % 8);
            let mut colour_number = ((data2 >> colour_bit) & 1) << 1;
            colour_number |= (data1 >> colour_bit) & 1;

            let colour = self.get_colour(colour_number as u8, 0xFF47);
            self.screen[line_y][pixel] = Self::colour_bytes(colour);
        }
    }

    fn render_sprites(&mut self, lcd_control: u8) {
        if lcd_control & 2 == 0 {
            return;
        }

        let use_8x16 = lcd_control & 4 != 0;
        let scan_line = self.read_memory(0xFF44) as i32;

        for sprite in 0..40u16 {
            let index = sprite * 4;
            let y_position = self.read_memory(0xFE00 + index).wrapping_sub(16) as i32;
            let x_position = self.read_memory(0xFE00 + index + 1).wrapping_sub(8) as i32;
            let tile_location = self.read_memory(0xFE00 + index + 2);
            let attributes = self.read_memory(0xFE00 + index + 3);

            let y_flip = attributes & (1 << 6) != 0;
            let x_flip = attributes & (1 << 5) != 0;
            let y_size = if use_8x16 { 16 } else { 8 };

            if scan_line >= y_position && scan_line < y_position + y_size {
                let mut line = scan_line - y_position;
                if y_flip {
                    line = (line - y_size) * -1;
                }
                line *= 2;

                let base = 0x8000u16.wrapping_add(tile_location as u16 * 16);
                let data1 = self.read_memory(base.wrapping_add(line as u16));
                let data2 = self.read_memory(base.wrapping_add(line as u16 + 1));

                for tile_pixel in (0..8).rev() {
                    let colour_bit = if x_flip { 7 - tile_pixel } else { tile_pixel };
                    let mut colour_number = ((data2 >> colour_bit) & 1) << 1;
                    colour_number |= (data1 >> colour_bit) & 1;

                    let palette = if attributes & (1 << 4) != 0 {
                        0xFF49
                    } else {
                        0xFF48
                    };
                    let colour = self.get_colour(colour_number as u8, palette);
                    if colour == Colour::White {
                        continue;
                    }

                    let pixel = x_position + 7 - tile_pixel as i32;
                    if scan_line < 0 || scan_line > 143 || pixel < 0 || pixel >= SCREEN_WIDTH as i32
                    {
                        continue;
                    }

                    if attributes & (1 << 7) != 0 {
                        let existing = self.screen[scan_line as usize][pixel as usize];
                        if existing != [255, 255, 255] {
                            continue;
                        }
                    }

                    self.screen[scan_line as usize][pixel as usize] =
                        Self::colour_bytes(colour);
                }
            }
        }
    }
}
