//! A Game Boy (DMG) emulator core.
//!
//! The public surface is intentionally small: load a ROM, initialise the CPU,
//! then call [`Emulator::update`] once per frame and read the framebuffer with
//! [`Emulator::screen`].

mod constants;
mod cpu;
mod memory;
mod ppu;
mod system;

use constants::{Screen, SCREEN_HEIGHT, SCREEN_WIDTH, TARGET_CYCLES};

pub use constants::{FLAG_C, FLAG_H, FLAG_N, FLAG_Z, SCREEN_HEIGHT as HEIGHT, SCREEN_WIDTH as WIDTH};

/// The complete emulator state: CPU, memory/banking, PPU, timers and joypad.
pub struct Emulator {
    /// RGB framebuffer, indexed as `[y][x][channel]`.
    pub screen: Box<Screen>,

    pub(crate) rom: Box<[u8]>,
    pub(crate) game_bank: Box<[u8]>,
    pub(crate) ram_banks: Vec<[u8; 0x2000]>,

    pub(crate) a: u8,
    pub(crate) f: u8,
    pub(crate) b: u8,
    pub(crate) c: u8,
    pub(crate) d: u8,
    pub(crate) e: u8,
    pub(crate) h: u8,
    pub(crate) l: u8,

    pub(crate) pc: u16,
    pub(crate) sp: u16,

    pub(crate) cycles_this_update: u32,
    pub(crate) total_opcodes: u64,

    pub(crate) current_rom_bank: u32,
    pub(crate) current_ram_bank: usize,
    pub(crate) using_memory_model_16_8: bool,
    pub(crate) enable_ram_bank: bool,
    pub(crate) using_mbc1: bool,
    pub(crate) using_mbc2: bool,

    pub(crate) enable_interrupts: bool,
    pub(crate) pending_interrupt_disabled: bool,
    pub(crate) pending_interrupt_enabled: bool,
    pub(crate) halted: bool,

    pub(crate) retrace_ly: i32,
    pub(crate) joypad_state: u8,
    pub(crate) game_loaded: bool,

    pub(crate) timer_variable: u32,
    pub(crate) divider_variable: u32,
    pub(crate) current_clock_speed: u32,
    pub(crate) debug_value: u8,
}

impl Emulator {
    pub fn new() -> Self {
        let mut emulator = Self {
            screen: Box::new([[[255; 3]; SCREEN_WIDTH]; SCREEN_HEIGHT]),
            rom: vec![0; 0x10000].into_boxed_slice(),
            game_bank: vec![0; 0x200000].into_boxed_slice(),
            ram_banks: Vec::new(),
            a: 0,
            f: 0,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            pc: 0,
            sp: 0,
            cycles_this_update: 0,
            total_opcodes: 0,
            current_rom_bank: 1,
            current_ram_bank: 0,
            using_memory_model_16_8: true,
            enable_ram_bank: false,
            using_mbc1: false,
            using_mbc2: false,
            enable_interrupts: false,
            pending_interrupt_disabled: false,
            pending_interrupt_enabled: false,
            halted: false,
            retrace_ly: constants::RETRACE_START,
            joypad_state: 0,
            game_loaded: false,
            timer_variable: 0,
            divider_variable: 0,
            current_clock_speed: 1024,
            debug_value: 0,
        };
        emulator.reset_screen();
        emulator
    }

    /// Load a cartridge image into the emulator.
    pub fn load_rom(&mut self, rom_name: &str) -> bool {
        if self.game_loaded {
            self.stop_game();
        }

        self.game_loaded = true;
        self.rom.fill(0);
        self.game_bank.fill(0);

        let bytes = match std::fs::read(rom_name) {
            Ok(bytes) => bytes,
            Err(err) => {
                eprintln!("failed to open {}: {}", rom_name, err);
                self.game_loaded = false;
                return false;
            }
        };

        let count = bytes.len().min(self.game_bank.len());
        self.game_bank[..count].copy_from_slice(&bytes[..count]);
        // The first 32 KiB is mapped read-only and never changes.
        self.rom[..0x8000].copy_from_slice(&self.game_bank[..0x8000]);
        self.current_rom_bank = 1;

        true
    }

    /// Prepare the CPU and cartridge state after a ROM has been loaded.
    pub fn init_game(&mut self) -> bool {
        self.reset_cpu()
    }

    /// Run one frame's worth of machine cycles, filling the framebuffer.
    pub fn update(&mut self) {
        self.cycles_this_update = 0;

        while self.cycles_this_update < TARGET_CYCLES {
            let before = self.cycles_this_update;
            self.execute_next_opcode();
            let cycles = self.cycles_this_update - before;

            self.do_timers(cycles);
            self.do_graphics(cycles);
            self.do_input();
            self.do_interrupts();
        }
    }

    pub fn stop_game(&mut self) {
        self.game_loaded = false;
    }

    pub fn is_game_loaded(&self) -> bool {
        self.game_loaded
    }

    pub fn key_pressed(&mut self, key: u8) {
        self.on_key_pressed(key as u32);
    }

    pub fn key_released(&mut self, key: u8) {
        self.on_key_released(key as u32);
    }

    pub fn total_opcodes(&self) -> u64 {
        self.total_opcodes
    }

    pub fn program_counter(&self) -> u16 {
        self.pc
    }

    pub fn registers(&self) -> (u16, u16, u16, u16, u16) {
        (
            self.get_af(),
            self.get_bc(),
            self.get_de(),
            self.get_hl(),
            self.sp,
        )
    }

    pub fn zero_flag(&self) -> bool {
        self.flag(constants::FLAG_Z_BIT)
    }

    pub fn subtract_flag(&self) -> bool {
        self.flag(constants::FLAG_N_BIT)
    }

    pub fn half_carry_flag(&self) -> bool {
        self.flag(constants::FLAG_H_BIT)
    }

    pub fn carry_flag(&self) -> bool {
        self.flag(constants::FLAG_C_BIT)
    }

    pub fn screen(&self) -> &Screen {
        &self.screen
    }
}

impl Default for Emulator {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests;
