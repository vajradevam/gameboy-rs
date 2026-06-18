//! Memory map, cartridge banking and stack access.

use crate::Emulator;
use crate::constants::RETRACE_START;

impl Emulator {
    /// All ROM reads funnel through here so banked regions can be trapped.
    pub(crate) fn read_memory(&self, address: u16) -> u8 {
        match address {
            0x4000..=0x7FFF => {
                let index = address as u32 + ((self.current_rom_bank - 1) * 0x4000);
                self.game_bank[index as usize]
            }
            0xA000..=0xBFFF => {
                let index = (address - 0xA000) as usize;
                self.ram_banks[self.current_ram_bank][index]
            }
            0xFF00 => self.get_joypad_state(),
            _ => self.rom[address as usize],
        }
    }

    /// Write a byte to the bus. Addresses below 0x8000 are cartridge registers.
    pub(crate) fn write_byte(&mut self, address: u16, data: u8) {
        match address {
            0x0000..=0x1FFF => {
                if self.using_mbc1 {
                    if data & 0xF == 0xA {
                        self.enable_ram_bank = true;
                    } else if data == 0 {
                        self.enable_ram_bank = false;
                    }
                } else if self.using_mbc2 && (address & 0x0100) == 0 {
                    if data & 0xF == 0xA {
                        self.enable_ram_bank = true;
                    } else if data == 0 {
                        self.enable_ram_bank = false;
                    }
                }
            }
            0x2000..=0x3FFF => {
                if self.using_mbc1 {
                    let mut bank = if data == 0 { 1 } else { data };
                    bank &= 31;
                    self.current_rom_bank = (self.current_rom_bank & 224) | bank as u32;
                } else if self.using_mbc2 {
                    self.current_rom_bank = (data & 0xF) as u32;
                }
            }
            0x4000..=0x5FFF => {
                if self.using_mbc1 {
                    if self.using_memory_model_16_8 {
                        self.current_ram_bank = 0;
                        let mut high = ((data & 3) as u32) << 5;
                        if self.current_rom_bank & 31 == 0 {
                            high += 1;
                        }
                        self.current_rom_bank = (self.current_rom_bank & 31) | high;
                    } else {
                        self.current_ram_bank = (data & 0x3) as usize;
                    }
                }
            }
            0x6000..=0x7FFF => {
                if self.using_mbc1 {
                    if data & 1 == 1 {
                        self.current_ram_bank = 0;
                        self.using_memory_model_16_8 = false;
                    } else {
                        self.using_memory_model_16_8 = true;
                    }
                }
            }
            0xA000..=0xBFFF => {
                if self.enable_ram_bank && self.using_mbc1 {
                    let index = (address - 0xA000) as usize;
                    self.ram_banks[self.current_ram_bank][index] = data;
                } else if self.using_mbc2 && address < 0xA200 {
                    let index = (address - 0xA000) as usize;
                    self.ram_banks[self.current_ram_bank][index] = data;
                }
            }
            0xC000..=0xDFFF => {
                self.rom[address as usize] = data;
            }
            0xE000..=0xFDFF => {
                self.rom[address as usize] = data;
                self.rom[(address - 0x2000) as usize] = data;
            }
            0xFEA0..=0xFEFF => {}
            0xFF04 => {
                self.rom[0xFF04] = 0;
                self.divider_variable = 0;
            }
            0xFF07 => {
                self.rom[address as usize] = data;
                let clock_speed = match data & 0x03 {
                    0 => 1024,
                    1 => 16,
                    2 => 64,
                    3 => 256,
                    _ => unreachable!(),
                };
                if clock_speed != self.current_clock_speed {
                    self.timer_variable = 0;
                    self.current_clock_speed = clock_speed;
                }
            }
            0xFF44 => {
                self.rom[0xFF44] = 0;
            }
            0xFF45 => {
                self.rom[address as usize] = data;
            }
            0xFF46 => {
                let base = (data as u16) << 8;
                for i in 0..0xA0u16 {
                    let value = self.read_memory(base + i);
                    self.rom[0xFE00 + i as usize] = value;
                }
            }
            0xFF4C..=0xFF7F => {}
            _ => {
                self.rom[address as usize] = data;
            }
        }
    }

    /// Little-endian word at the current program counter.
    pub(crate) fn read_word(&self) -> u16 {
        let low = self.read_memory(self.pc) as u16;
        let high = self.read_memory(self.pc.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    /// Read the immediate byte at the program counter and advance past it.
    pub(crate) fn immediate_byte(&mut self) -> u8 {
        let value = self.read_memory(self.pc);
        self.pc = self.pc.wrapping_add(1);
        value
    }

    pub(crate) fn push_word_onto_stack(&mut self, word: u16) {
        let high = (word >> 8) as u8;
        let low = word as u8;
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, high);
        self.sp = self.sp.wrapping_sub(1);
        self.write_byte(self.sp, low);
    }

    pub(crate) fn pop_word_off_stack(&mut self) -> u16 {
        let low = self.read_memory(self.sp) as u16;
        let high = self.read_memory(self.sp.wrapping_add(1)) as u16;
        self.sp = self.sp.wrapping_add(2);
        (high << 8) | low
    }

    pub(crate) fn reset_screen(&mut self) {
        for row in self.screen.iter_mut() {
            for pixel in row.iter_mut() {
                *pixel = [255, 255, 255];
            }
        }
    }

    pub(crate) fn create_ram_banks(&mut self, _num_banks: usize) {
        // The hardware supports up to 16 switchable banks; bank zero is
        // initialised from the cartridge header area.
        self.ram_banks = vec![[0; 0x2000]; 17];
        self.ram_banks[0].copy_from_slice(&self.rom[0xA000..0xC000]);
    }

    pub(crate) fn reset_cpu(&mut self) -> bool {
        self.reset_screen();
        self.current_ram_bank = 0;
        self.timer_variable = 0;
        self.current_clock_speed = 1024;
        self.divider_variable = 0;
        self.halted = false;
        self.total_opcodes = 0;
        self.joypad_state = 0xFF;
        self.cycles_this_update = 0;
        self.pc = 0x100;
        self.set_af(0x01B0);
        self.set_bc(0x0013);
        self.set_de(0x00D8);
        self.set_hl(0x014D);
        self.sp = 0xFFFE;

        self.rom[0xFF00] = 0xFF;
        self.rom[0xFF05] = 0x00;
        self.rom[0xFF06] = 0x00;
        self.rom[0xFF07] = 0x00;
        self.rom[0xFF10] = 0x80;
        self.rom[0xFF11] = 0xBF;
        self.rom[0xFF12] = 0xF3;
        self.rom[0xFF14] = 0xBF;
        self.rom[0xFF16] = 0x3F;
        self.rom[0xFF17] = 0x00;
        self.rom[0xFF19] = 0xBF;
        self.rom[0xFF1A] = 0x7F;
        self.rom[0xFF1B] = 0xFF;
        self.rom[0xFF1C] = 0x9F;
        self.rom[0xFF1E] = 0xBF;
        self.rom[0xFF20] = 0xFF;
        self.rom[0xFF21] = 0x00;
        self.rom[0xFF22] = 0x00;
        self.rom[0xFF23] = 0xBF;
        self.rom[0xFF24] = 0x77;
        self.rom[0xFF25] = 0xF3;
        self.rom[0xFF26] = 0xF1;
        self.rom[0xFF40] = 0x91;
        self.rom[0xFF42] = 0x00;
        self.rom[0xFF43] = 0x00;
        self.rom[0xFF45] = 0x00;
        self.rom[0xFF47] = 0xFC;
        self.rom[0xFF48] = 0xFF;
        self.rom[0xFF49] = 0xFF;
        self.rom[0xFF4A] = 0x00;
        self.rom[0xFF4B] = 0x00;
        self.rom[0xFFFF] = 0x00;
        self.retrace_ly = RETRACE_START;

        self.debug_value = self.rom[0x40];
        self.enable_ram_bank = false;
        self.using_mbc2 = false;

        match self.read_memory(0x147) {
            0 => self.using_mbc1 = false,
            1 | 2 | 3 => self.using_mbc1 = true,
            5 | 6 => {
                self.using_mbc1 = false;
                self.using_mbc2 = true;
            }
            _ => return false,
        }

        let num_banks = match self.read_memory(0x149) {
            0 => 0,
            1 | 2 => 1,
            3 => 4,
            4 => 16,
            _ => 0,
        };
        self.create_ram_banks(num_banks as usize);

        true
    }
}
