//! LR35902 CPU: registers, instruction dispatch and ALU helpers.

use crate::Emulator;
use crate::constants::{FLAG_C, FLAG_H, FLAG_N, FLAG_Z, FLAG_C_BIT, FLAG_H_BIT, FLAG_N_BIT, FLAG_Z_BIT};

#[derive(Clone, Copy)]
enum Reg8 {
    A,
    B,
    C,
    D,
    E,
    H,
    L,
}

#[derive(Clone, Copy)]
enum Reg16 {
    Af,
    Bc,
    De,
    Hl,
    Sp,
}

#[derive(Clone, Copy)]
enum Unary {
    Rlc,
    Rrc,
    Rl,
    Rr,
    Sla,
    Sra,
    Srl,
    Swap,
}

impl Emulator {
    // ---- register access -------------------------------------------------

    #[inline]
    pub(crate) fn get_af(&self) -> u16 {
        ((self.a as u16) << 8) | self.f as u16
    }

    #[inline]
    pub(crate) fn set_af(&mut self, value: u16) {
        self.a = (value >> 8) as u8;
        // The low nibble of F is hard-wired to zero on the hardware.
        self.f = value as u8 & 0xF0;
    }

    #[inline]
    pub(crate) fn get_bc(&self) -> u16 {
        ((self.b as u16) << 8) | self.c as u16
    }

    #[inline]
    pub(crate) fn set_bc(&mut self, value: u16) {
        self.b = (value >> 8) as u8;
        self.c = value as u8;
    }

    #[inline]
    pub(crate) fn get_de(&self) -> u16 {
        ((self.d as u16) << 8) | self.e as u16
    }

    #[inline]
    pub(crate) fn set_de(&mut self, value: u16) {
        self.d = (value >> 8) as u8;
        self.e = value as u8;
    }

    #[inline]
    pub(crate) fn get_hl(&self) -> u16 {
        ((self.h as u16) << 8) | self.l as u16
    }

    #[inline]
    pub(crate) fn set_hl(&mut self, value: u16) {
        self.h = (value >> 8) as u8;
        self.l = value as u8;
    }

    #[inline]
    fn get8(&self, reg: Reg8) -> u8 {
        match reg {
            Reg8::A => self.a,
            Reg8::B => self.b,
            Reg8::C => self.c,
            Reg8::D => self.d,
            Reg8::E => self.e,
            Reg8::H => self.h,
            Reg8::L => self.l,
        }
    }

    #[inline]
    fn set8(&mut self, reg: Reg8, value: u8) {
        match reg {
            Reg8::A => self.a = value,
            Reg8::B => self.b = value,
            Reg8::C => self.c = value,
            Reg8::D => self.d = value,
            Reg8::E => self.e = value,
            Reg8::H => self.h = value,
            Reg8::L => self.l = value,
        }
    }

    #[inline]
    fn get16(&self, reg: Reg16) -> u16 {
        match reg {
            Reg16::Af => self.get_af(),
            Reg16::Bc => self.get_bc(),
            Reg16::De => self.get_de(),
            Reg16::Hl => self.get_hl(),
            Reg16::Sp => self.sp,
        }
    }

    #[inline]
    fn set16(&mut self, reg: Reg16, value: u16) {
        match reg {
            Reg16::Af => self.set_af(value),
            Reg16::Bc => self.set_bc(value),
            Reg16::De => self.set_de(value),
            Reg16::Hl => self.set_hl(value),
            Reg16::Sp => self.sp = value,
        }
    }

    #[inline]
    fn reg_from_z(z: u8) -> Reg8 {
        match z {
            0 => Reg8::B,
            1 => Reg8::C,
            2 => Reg8::D,
            3 => Reg8::E,
            4 => Reg8::H,
            5 => Reg8::L,
            7 => Reg8::A,
            _ => unreachable!(),
        }
    }

    #[inline]
    pub(crate) fn flag(&self, bit: u8) -> bool {
        self.f & (1 << bit) != 0
    }

    // ---- instruction helpers --------------------------------------------

    #[inline]
    fn check_zero(&mut self, value: u8) {
        if value == 0 {
            self.f |= FLAG_Z;
        }
    }

    #[inline]
    fn load_immediate(&mut self, dst: Reg8) {
        let value = self.immediate_byte();
        self.set8(dst, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn move_reg(&mut self, dst: Reg8, src: Reg8) {
        let value = self.get8(src);
        self.set8(dst, value);
        self.cycles_this_update += 4;
    }

    #[inline]
    fn load_hl(&mut self, dst: Reg8) {
        let value = self.read_memory(self.get_hl());
        self.set8(dst, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn load_address(&mut self, dst: Reg8, address: u16) {
        let value = self.read_memory(address);
        self.set8(dst, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn store_hl(&mut self, src: Reg8) {
        let value = self.get8(src);
        self.write_byte(self.get_hl(), value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn store_address(&mut self, address: u16, src: Reg8) {
        let value = self.get8(src);
        self.write_byte(address, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn load16(&mut self, dst: Reg16) {
        let value = self.read_word();
        self.pc = self.pc.wrapping_add(2);
        self.set16(dst, value);
        self.cycles_this_update += 12;
    }

    #[inline]
    fn push(&mut self, reg: Reg16) {
        let value = self.get16(reg);
        self.push_word_onto_stack(value);
        self.cycles_this_update += 16;
    }

    #[inline]
    fn pop(&mut self, reg: Reg16) {
        let value = self.pop_word_off_stack();
        self.set16(reg, value);
        self.cycles_this_update += 12;
    }

    #[inline]
    fn alu_add(&mut self, value: u8, add_carry: bool, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.a;
        let carry = if add_carry && self.flag(FLAG_C_BIT) {
            1u16
        } else {
            0
        };
        let total = before as u16 + value as u16 + carry;

        self.a = total as u8;
        self.f = 0;
        self.check_zero(self.a);

        if (before & 0xF) as u16 + (value & 0xF) as u16 + carry > 0xF {
            self.f |= FLAG_H;
        }
        if total > 0xFF {
            self.f |= FLAG_C;
        }
    }

    #[inline]
    fn alu_sub(&mut self, value: u8, sub_carry: bool, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.a;
        let carry = if sub_carry && self.flag(FLAG_C_BIT) {
            1i32
        } else {
            0
        };
        let total = before as i32 - value as i32 - carry;

        self.a = total as u8;
        self.f = 0;
        self.check_zero(self.a);
        self.f |= FLAG_N;

        if total < 0 {
            self.f |= FLAG_C;
        }
        if (before as i32 & 0xF) - (value as i32 & 0xF) - carry < 0 {
            self.f |= FLAG_H;
        }
    }

    #[inline]
    fn alu_and(&mut self, value: u8, cycles: u32) {
        self.cycles_this_update += cycles;
        self.a &= value;
        self.f = 0;
        self.check_zero(self.a);
        self.f |= FLAG_H;
    }

    #[inline]
    fn alu_or(&mut self, value: u8, cycles: u32) {
        self.cycles_this_update += cycles;
        self.a |= value;
        self.f = 0;
        self.check_zero(self.a);
    }

    #[inline]
    fn alu_xor(&mut self, value: u8, cycles: u32) {
        self.cycles_this_update += cycles;
        self.a ^= value;
        self.f = 0;
        self.check_zero(self.a);
    }

    #[inline]
    fn alu_cp(&mut self, value: u8, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.a;
        let result = before.wrapping_sub(value);
        self.f = 0;
        self.check_zero(result);
        self.f |= FLAG_N;
        if before < value {
            self.f |= FLAG_C;
        }
        let half = (before as i16 & 0xF) - (value as i16 & 0xF);
        if half < 0 {
            self.f |= FLAG_H;
        }
    }

    #[inline]
    fn inc_reg8(&mut self, reg: Reg8, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.get8(reg);
        let after = before.wrapping_add(1);
        self.set8(reg, after);

        if after == 0 {
            self.f |= FLAG_Z;
        } else {
            self.f &= !FLAG_Z;
        }
        self.f &= !FLAG_N;
        if before & 0xF == 0xF {
            self.f |= FLAG_H;
        } else {
            self.f &= !FLAG_H;
        }
    }

    #[inline]
    fn dec_reg8(&mut self, reg: Reg8, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.get8(reg);
        let after = before.wrapping_sub(1);
        self.set8(reg, after);

        if after == 0 {
            self.f |= FLAG_Z;
        } else {
            self.f &= !FLAG_Z;
        }
        self.f |= FLAG_N;
        if before & 0xF == 0 {
            self.f |= FLAG_H;
        } else {
            self.f &= !FLAG_H;
        }
    }

    #[inline]
    fn inc_hl(&mut self, cycles: u32) {
        self.cycles_this_update += cycles;
        let address = self.get_hl();
        let before = self.read_memory(address);
        let after = before.wrapping_add(1);
        self.write_byte(address, after);

        if after == 0 {
            self.f |= FLAG_Z;
        } else {
            self.f &= !FLAG_Z;
        }
        self.f &= !FLAG_N;
        if before & 0xF == 0xF {
            self.f |= FLAG_H;
        } else {
            self.f &= !FLAG_H;
        }
    }

    #[inline]
    fn dec_hl(&mut self, cycles: u32) {
        self.cycles_this_update += cycles;
        let address = self.get_hl();
        let before = self.read_memory(address);
        let after = before.wrapping_sub(1);
        self.write_byte(address, after);

        if after == 0 {
            self.f |= FLAG_Z;
        } else {
            self.f &= !FLAG_Z;
        }
        self.f |= FLAG_N;
        if before & 0xF == 0 {
            self.f |= FLAG_H;
        } else {
            self.f &= !FLAG_H;
        }
    }

    #[inline]
    fn add_hl16(&mut self, value: u16, cycles: u32) {
        self.cycles_this_update += cycles;
        let before = self.get_hl();
        self.set_hl(before.wrapping_add(value));

        self.f &= !FLAG_N;
        if before as u32 + value as u32 > 0xFFFF {
            self.f |= FLAG_C;
        } else {
            self.f &= !FLAG_C;
        }

        // Half carry is a carry out of bit 11.
        if (before & 0x0FFF) + (value & 0x0FFF) > 0x0FFF {
            self.f |= FLAG_H;
        } else {
            self.f &= !FLAG_H;
        }
    }

    #[inline]
    fn inc16(&mut self, reg: Reg16, cycles: u32) {
        self.cycles_this_update += cycles;
        let value = self.get16(reg);
        self.set16(reg, value.wrapping_add(1));
    }

    #[inline]
    fn dec16(&mut self, reg: Reg16, cycles: u32) {
        self.cycles_this_update += cycles;
        let value = self.get16(reg);
        self.set16(reg, value.wrapping_sub(1));
    }

    #[inline]
    fn jump_absolute(&mut self) {
        self.cycles_this_update += 12;
        let target = self.read_word();
        self.pc = self.pc.wrapping_add(2);
        self.pc = target;
    }

    #[inline]
    fn jump_conditional(&mut self, bit: u8, condition: bool) {
        self.cycles_this_update += 12;
        let target = self.read_word();
        self.pc = self.pc.wrapping_add(2);
        if self.flag(bit) == condition {
            self.pc = target;
        }
    }

    #[inline]
    fn jump_relative(&mut self, bit: Option<u8>, condition: bool) {
        self.cycles_this_update += 8;
        let offset = self.read_memory(self.pc) as i8 as u16;
        match bit {
            None => self.pc = self.pc.wrapping_add(offset),
            Some(b) if self.flag(b) == condition => self.pc = self.pc.wrapping_add(offset),
            _ => {}
        }
        self.pc = self.pc.wrapping_add(1);
    }

    #[inline]
    fn call(&mut self, bit: Option<u8>, condition: bool) {
        self.cycles_this_update += 12;
        let target = self.read_word();
        self.pc = self.pc.wrapping_add(2);
        if bit.map_or(true, |b| self.flag(b) == condition) {
            let address = self.pc;
            self.push_word_onto_stack(address);
            self.pc = target;
        }
    }

    #[inline]
    fn ret(&mut self, bit: Option<u8>, condition: bool) {
        self.cycles_this_update += 8;
        if bit.map_or(true, |b| self.flag(b) == condition) {
            self.pc = self.pop_word_off_stack();
        }
    }

    #[inline]
    fn restart(&mut self, target: u8) {
        let address = self.pc;
        self.push_word_onto_stack(address);
        self.cycles_this_update += 32;
        self.pc = target as u16;
    }

    #[inline]
    fn unary_value(&mut self, value: u8, op: Unary, set_zero: bool) -> u8 {
        let mut result = value;
        match op {
            Unary::Rlc => {
                let msb = result & 0x80 != 0;
                self.f = 0;
                result <<= 1;
                if msb {
                    self.f |= FLAG_C;
                    result |= 1;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Rrc => {
                let lsb = result & 1 != 0;
                self.f = 0;
                result >>= 1;
                if lsb {
                    self.f |= FLAG_C;
                    result |= 0x80;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Rl => {
                let carry = self.flag(FLAG_C_BIT);
                let msb = result & 0x80 != 0;
                self.f = 0;
                result <<= 1;
                if msb {
                    self.f |= FLAG_C;
                }
                if carry {
                    result |= 1;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Rr => {
                let carry = self.flag(FLAG_C_BIT);
                let lsb = result & 1 != 0;
                self.f = 0;
                result >>= 1;
                if lsb {
                    self.f |= FLAG_C;
                }
                if carry {
                    result |= 0x80;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Sla => {
                let msb = result & 0x80 != 0;
                result <<= 1;
                self.f = 0;
                if msb {
                    self.f |= FLAG_C;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Sra => {
                let lsb = result & 1 != 0;
                let msb = result & 0x80 != 0;
                self.f = 0;
                result >>= 1;
                if msb {
                    result |= 0x80;
                }
                if lsb {
                    self.f |= FLAG_C;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Srl => {
                let lsb = result & 1 != 0;
                self.f = 0;
                result >>= 1;
                if lsb {
                    self.f |= FLAG_C;
                }
                if set_zero {
                    self.check_zero(result);
                }
            }
            Unary::Swap => {
                self.f = 0;
                result = (result >> 4) | (result << 4);
                if set_zero {
                    self.check_zero(result);
                }
            }
        }
        result
    }

    #[inline]
    fn unary_reg(&mut self, reg: Reg8, op: Unary, cycles: u32) {
        self.cycles_this_update += cycles;
        let value = self.get8(reg);
        let result = self.unary_value(value, op, true);
        self.set8(reg, result);
    }

    #[inline]
    fn unary_hl(&mut self, op: Unary, cycles: u32) {
        self.cycles_this_update += cycles;
        let address = self.get_hl();
        let value = self.read_memory(address);
        let result = self.unary_value(value, op, true);
        self.write_byte(address, result);
    }

    #[inline]
    fn unary_accum(&mut self, op: Unary, cycles: u32) {
        self.cycles_this_update += cycles;
        let value = self.a;
        let result = self.unary_value(value, op, false);
        self.a = result;
    }

    #[inline]
    fn test_bit_value(&mut self, value: u8, bit: u8, cycles: u32) {
        self.cycles_this_update += cycles;
        if value & (1 << bit) != 0 {
            self.f &= !FLAG_Z;
        } else {
            self.f |= FLAG_Z;
        }
        self.f &= !FLAG_N;
        self.f |= FLAG_H;
    }

    #[inline]
    fn test_bit_reg(&mut self, reg: Reg8, bit: u8, cycles: u32) {
        let value = self.get8(reg);
        self.test_bit_value(value, bit, cycles);
    }

    #[inline]
    fn test_bit_hl(&mut self, bit: u8) {
        let value = self.read_memory(self.get_hl());
        self.test_bit_value(value, bit, 16);
    }

    #[inline]
    fn reset_bit_reg(&mut self, reg: Reg8, bit: u8) {
        let value = self.get8(reg) & !(1 << bit);
        self.set8(reg, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn reset_bit_hl(&mut self, bit: u8) {
        let address = self.get_hl();
        let value = self.read_memory(address) & !(1 << bit);
        self.write_byte(address, value);
        self.cycles_this_update += 16;
    }

    #[inline]
    fn set_bit_reg(&mut self, reg: Reg8, bit: u8) {
        let value = self.get8(reg) | (1 << bit);
        self.set8(reg, value);
        self.cycles_this_update += 8;
    }

    #[inline]
    fn set_bit_hl(&mut self, bit: u8) {
        let address = self.get_hl();
        let value = self.read_memory(address) | (1 << bit);
        self.write_byte(address, value);
        self.cycles_this_update += 16;
    }

    #[inline]
    fn daa(&mut self) {
        self.cycles_this_update += 4;
        let subtract = self.flag(FLAG_N_BIT);
        let half_carry = self.flag(FLAG_H_BIT);
        let carry_in = self.flag(FLAG_C_BIT);
        let mut carry = false;

        if !subtract {
            if carry_in || self.a > 0x99 {
                self.a = self.a.wrapping_add(0x60);
                carry = true;
            }
            if half_carry || self.a & 0x0F > 0x09 {
                self.a = self.a.wrapping_add(0x06);
            }
        } else if carry_in {
            carry = true;
            self.a = self
                .a
                .wrapping_add(if half_carry { 0x9A } else { 0xA0 });
        } else if half_carry {
            self.a = self.a.wrapping_add(0xFA);
        }

        self.f = 0;
        if self.a == 0 {
            self.f |= FLAG_Z;
        }
        if subtract {
            self.f |= FLAG_N;
        }
        if carry {
            self.f |= FLAG_C;
        }
    }

    // ---- opcode dispatch -------------------------------------------------

    #[inline]
    pub(crate) fn execute_next_opcode(&mut self) -> u8 {
        let pc = self.pc;
        let mut opcode = self.rom[pc as usize];
        if (0x4000..=0x7FFF).contains(&pc) || (0xA000..=0xBFFF).contains(&pc) {
            opcode = self.read_memory(pc);
        }

        if !self.halted {
            self.pc = self.pc.wrapping_add(1);
            // HALT with a pending interrupt and IME=0 re-executes the byte
            // following the HALT instruction instead of advancing past it.
            if self.halt_bug {
                self.pc = self.pc.wrapping_sub(1);
                self.halt_bug = false;
            }
            self.total_opcodes += 1;
            self.execute_opcode(opcode);
        } else {
            self.cycles_this_update += 4;
        }

        // DI and EI take effect after the following instruction.
        if self.pending_interrupt_disabled
            && self.read_memory(self.pc.wrapping_sub(1)) != 0xF3
        {
            self.pending_interrupt_disabled = false;
            self.enable_interrupts = false;
        }
        if self.pending_interrupt_enabled
            && self.read_memory(self.pc.wrapping_sub(1)) != 0xFB
        {
            self.pending_interrupt_enabled = false;
            self.enable_interrupts = true;
        }

        opcode
    }

    pub(crate) fn execute_opcode(&mut self, opcode: u8) {
        match opcode {
            0x00 => self.cycles_this_update += 4,

            // 8-bit loads into registers
            0x06 => self.load_immediate(Reg8::B),
            0x0E => self.load_immediate(Reg8::C),
            0x16 => self.load_immediate(Reg8::D),
            0x1E => self.load_immediate(Reg8::E),
            0x26 => self.load_immediate(Reg8::H),
            0x2E => self.load_immediate(Reg8::L),

            // register to register
            0x7F => self.move_reg(Reg8::A, Reg8::A),
            0x78 => self.move_reg(Reg8::A, Reg8::B),
            0x79 => self.move_reg(Reg8::A, Reg8::C),
            0x7A => self.move_reg(Reg8::A, Reg8::D),
            0x7B => self.move_reg(Reg8::A, Reg8::E),
            0x7C => self.move_reg(Reg8::A, Reg8::H),
            0x7D => self.move_reg(Reg8::A, Reg8::L),
            0x40 => self.move_reg(Reg8::B, Reg8::B),
            0x41 => self.move_reg(Reg8::B, Reg8::C),
            0x42 => self.move_reg(Reg8::B, Reg8::D),
            0x43 => self.move_reg(Reg8::B, Reg8::E),
            0x44 => self.move_reg(Reg8::B, Reg8::H),
            0x45 => self.move_reg(Reg8::B, Reg8::L),
            0x48 => self.move_reg(Reg8::C, Reg8::B),
            0x49 => self.move_reg(Reg8::C, Reg8::C),
            0x4A => self.move_reg(Reg8::C, Reg8::D),
            0x4B => self.move_reg(Reg8::C, Reg8::E),
            0x4C => self.move_reg(Reg8::C, Reg8::H),
            0x4D => self.move_reg(Reg8::C, Reg8::L),
            0x50 => self.move_reg(Reg8::D, Reg8::B),
            0x51 => self.move_reg(Reg8::D, Reg8::C),
            0x52 => self.move_reg(Reg8::D, Reg8::D),
            0x53 => self.move_reg(Reg8::D, Reg8::E),
            0x54 => self.move_reg(Reg8::D, Reg8::H),
            0x55 => self.move_reg(Reg8::D, Reg8::L),
            0x58 => self.move_reg(Reg8::E, Reg8::B),
            0x59 => self.move_reg(Reg8::E, Reg8::C),
            0x5A => self.move_reg(Reg8::E, Reg8::D),
            0x5B => self.move_reg(Reg8::E, Reg8::E),
            0x5C => self.move_reg(Reg8::E, Reg8::H),
            0x5D => self.move_reg(Reg8::E, Reg8::L),
            0x60 => self.move_reg(Reg8::H, Reg8::B),
            0x61 => self.move_reg(Reg8::H, Reg8::C),
            0x62 => self.move_reg(Reg8::H, Reg8::D),
            0x63 => self.move_reg(Reg8::H, Reg8::E),
            0x64 => self.move_reg(Reg8::H, Reg8::H),
            0x65 => self.move_reg(Reg8::H, Reg8::L),
            0x68 => self.move_reg(Reg8::L, Reg8::B),
            0x69 => self.move_reg(Reg8::L, Reg8::C),
            0x6A => self.move_reg(Reg8::L, Reg8::D),
            0x6B => self.move_reg(Reg8::L, Reg8::E),
            0x6C => self.move_reg(Reg8::L, Reg8::H),
            0x6D => self.move_reg(Reg8::L, Reg8::L),

            // register to memory at HL
            0x70 => self.store_hl(Reg8::B),
            0x71 => self.store_hl(Reg8::C),
            0x72 => self.store_hl(Reg8::D),
            0x73 => self.store_hl(Reg8::E),
            0x74 => self.store_hl(Reg8::H),
            0x75 => self.store_hl(Reg8::L),

            // memory at HL to register
            0x7E => self.load_hl(Reg8::A),
            0x46 => self.load_hl(Reg8::B),
            0x4E => self.load_hl(Reg8::C),
            0x56 => self.load_hl(Reg8::D),
            0x5E => self.load_hl(Reg8::E),
            0x66 => self.load_hl(Reg8::H),
            0x6E => self.load_hl(Reg8::L),

            // indirect reads
            0x0A => {
                let address = self.get_bc();
                self.load_address(Reg8::A, address);
            }
            0x1A => {
                let address = self.get_de();
                self.load_address(Reg8::A, address);
            }
            0xF2 => {
                let address = 0xFF00 + self.c as u16;
                self.load_address(Reg8::A, address);
            }

            // A to register
            0x47 => self.move_reg(Reg8::B, Reg8::A),
            0x4F => self.move_reg(Reg8::C, Reg8::A),
            0x57 => self.move_reg(Reg8::D, Reg8::A),
            0x5F => self.move_reg(Reg8::E, Reg8::A),
            0x67 => self.move_reg(Reg8::H, Reg8::A),
            0x6F => self.move_reg(Reg8::L, Reg8::A),

            // A to memory
            0x02 => {
                let address = self.get_bc();
                self.store_address(address, Reg8::A);
            }
            0x12 => {
                let address = self.get_de();
                self.store_address(address, Reg8::A);
            }
            0x77 => self.store_hl(Reg8::A),
            0xE2 => {
                let address = 0xFF00 + self.c as u16;
                self.store_address(address, Reg8::A);
            }

            // A to/from HL with auto increment/decrement
            0x3A => {
                self.load_hl(Reg8::A);
                self.dec16(Reg16::Hl, 0);
            }
            0x2A => {
                self.load_hl(Reg8::A);
                self.inc16(Reg16::Hl, 0);
            }
            0x32 => {
                self.store_hl(Reg8::A);
                self.dec16(Reg16::Hl, 0);
                self.cycles_this_update += 8;
            }
            0x22 => {
                self.store_hl(Reg8::A);
                self.inc16(Reg16::Hl, 0);
                self.cycles_this_update += 8;
            }

            // 16-bit loads
            0x01 => self.load16(Reg16::Bc),
            0x11 => self.load16(Reg16::De),
            0x21 => self.load16(Reg16::Hl),
            0x31 => self.load16(Reg16::Sp),
            0xF9 => {
                let value = self.get_hl();
                self.sp = value;
                self.cycles_this_update += 8;
            }

            // stack operations
            0xF5 => self.push(Reg16::Af),
            0xC5 => self.push(Reg16::Bc),
            0xD5 => self.push(Reg16::De),
            0xE5 => self.push(Reg16::Hl),
            0xF1 => self.pop(Reg16::Af),
            0xC1 => self.pop(Reg16::Bc),
            0xD1 => self.pop(Reg16::De),
            0xE1 => self.pop(Reg16::Hl),

            // 8-bit add
            0x87 => {
                let v = self.a;
                self.alu_add(v, false, 4);
            }
            0x80 => {
                let v = self.b;
                self.alu_add(v, false, 4);
            }
            0x81 => {
                let v = self.c;
                self.alu_add(v, false, 4);
            }
            0x82 => {
                let v = self.d;
                self.alu_add(v, false, 4);
            }
            0x83 => {
                let v = self.e;
                self.alu_add(v, false, 4);
            }
            0x84 => {
                let v = self.h;
                self.alu_add(v, false, 4);
            }
            0x85 => {
                let v = self.l;
                self.alu_add(v, false, 4);
            }
            0x86 => {
                let v = self.read_memory(self.get_hl());
                self.alu_add(v, false, 8);
            }
            0xC6 => {
                let v = self.immediate_byte();
                self.alu_add(v, false, 8);
            }

            // 8-bit add with carry
            0x8F => {
                let v = self.a;
                self.alu_add(v, true, 4);
            }
            0x88 => {
                let v = self.b;
                self.alu_add(v, true, 4);
            }
            0x89 => {
                let v = self.c;
                self.alu_add(v, true, 4);
            }
            0x8A => {
                let v = self.d;
                self.alu_add(v, true, 4);
            }
            0x8B => {
                let v = self.e;
                self.alu_add(v, true, 4);
            }
            0x8C => {
                let v = self.h;
                self.alu_add(v, true, 4);
            }
            0x8D => {
                let v = self.l;
                self.alu_add(v, true, 4);
            }
            0x8E => {
                let v = self.read_memory(self.get_hl());
                self.alu_add(v, true, 8);
            }
            0xCE => {
                let v = self.immediate_byte();
                self.alu_add(v, true, 8);
            }

            // 8-bit subtract
            0x97 => {
                let v = self.a;
                self.alu_sub(v, false, 4);
            }
            0x90 => {
                let v = self.b;
                self.alu_sub(v, false, 4);
            }
            0x91 => {
                let v = self.c;
                self.alu_sub(v, false, 4);
            }
            0x92 => {
                let v = self.d;
                self.alu_sub(v, false, 4);
            }
            0x93 => {
                let v = self.e;
                self.alu_sub(v, false, 4);
            }
            0x94 => {
                let v = self.h;
                self.alu_sub(v, false, 4);
            }
            0x95 => {
                let v = self.l;
                self.alu_sub(v, false, 4);
            }
            0x96 => {
                let v = self.read_memory(self.get_hl());
                self.alu_sub(v, false, 8);
            }
            0xD6 => {
                let v = self.immediate_byte();
                self.alu_sub(v, false, 8);
            }

            // 8-bit subtract with carry
            0x9F => {
                let v = self.a;
                self.alu_sub(v, true, 4);
            }
            0x98 => {
                let v = self.b;
                self.alu_sub(v, true, 4);
            }
            0x99 => {
                let v = self.c;
                self.alu_sub(v, true, 4);
            }
            0x9A => {
                let v = self.d;
                self.alu_sub(v, true, 4);
            }
            0x9B => {
                let v = self.e;
                self.alu_sub(v, true, 4);
            }
            0x9C => {
                let v = self.h;
                self.alu_sub(v, true, 4);
            }
            0x9D => {
                let v = self.l;
                self.alu_sub(v, true, 4);
            }
            0x9E => {
                let v = self.read_memory(self.get_hl());
                self.alu_sub(v, true, 8);
            }
            0xDE => {
                let v = self.immediate_byte();
                self.alu_sub(v, true, 8);
            }

            // logical AND
            0xA7 => {
                let v = self.a;
                self.alu_and(v, 4);
            }
            0xA0 => {
                let v = self.b;
                self.alu_and(v, 4);
            }
            0xA1 => {
                let v = self.c;
                self.alu_and(v, 4);
            }
            0xA2 => {
                let v = self.d;
                self.alu_and(v, 4);
            }
            0xA3 => {
                let v = self.e;
                self.alu_and(v, 4);
            }
            0xA4 => {
                let v = self.h;
                self.alu_and(v, 4);
            }
            0xA5 => {
                let v = self.l;
                self.alu_and(v, 4);
            }
            0xA6 => {
                let v = self.read_memory(self.get_hl());
                self.alu_and(v, 8);
            }
            0xE6 => {
                let v = self.immediate_byte();
                self.alu_and(v, 8);
            }

            // logical OR
            0xB7 => {
                let v = self.a;
                self.alu_or(v, 4);
            }
            0xB0 => {
                let v = self.b;
                self.alu_or(v, 4);
            }
            0xB1 => {
                let v = self.c;
                self.alu_or(v, 4);
            }
            0xB2 => {
                let v = self.d;
                self.alu_or(v, 4);
            }
            0xB3 => {
                let v = self.e;
                self.alu_or(v, 4);
            }
            0xB4 => {
                let v = self.h;
                self.alu_or(v, 4);
            }
            0xB5 => {
                let v = self.l;
                self.alu_or(v, 4);
            }
            0xB6 => {
                let v = self.read_memory(self.get_hl());
                self.alu_or(v, 8);
            }
            0xF6 => {
                let v = self.immediate_byte();
                self.alu_or(v, 8);
            }

            // logical XOR
            0xAF => {
                let v = self.a;
                self.alu_xor(v, 4);
            }
            0xA8 => {
                let v = self.b;
                self.alu_xor(v, 4);
            }
            0xA9 => {
                let v = self.c;
                self.alu_xor(v, 4);
            }
            0xAA => {
                let v = self.d;
                self.alu_xor(v, 4);
            }
            0xAB => {
                let v = self.e;
                self.alu_xor(v, 4);
            }
            0xAC => {
                let v = self.h;
                self.alu_xor(v, 4);
            }
            0xAD => {
                let v = self.l;
                self.alu_xor(v, 4);
            }
            0xAE => {
                let v = self.read_memory(self.get_hl());
                self.alu_xor(v, 8);
            }
            0xEE => {
                let v = self.immediate_byte();
                self.alu_xor(v, 8);
            }

            // compare
            0xBF => {
                let v = self.a;
                self.alu_cp(v, 4);
            }
            0xB8 => {
                let v = self.b;
                self.alu_cp(v, 4);
            }
            0xB9 => {
                let v = self.c;
                self.alu_cp(v, 4);
            }
            0xBA => {
                let v = self.d;
                self.alu_cp(v, 4);
            }
            0xBB => {
                let v = self.e;
                self.alu_cp(v, 4);
            }
            0xBC => {
                let v = self.h;
                self.alu_cp(v, 4);
            }
            0xBD => {
                let v = self.l;
                self.alu_cp(v, 4);
            }
            0xBE => {
                let v = self.read_memory(self.get_hl());
                self.alu_cp(v, 8);
            }
            0xFE => {
                let v = self.immediate_byte();
                self.alu_cp(v, 8);
            }

            // increment / decrement
            0x3C => self.inc_reg8(Reg8::A, 4),
            0x04 => self.inc_reg8(Reg8::B, 4),
            0x0C => self.inc_reg8(Reg8::C, 4),
            0x14 => self.inc_reg8(Reg8::D, 4),
            0x1C => self.inc_reg8(Reg8::E, 4),
            0x24 => self.inc_reg8(Reg8::H, 4),
            0x2C => self.inc_reg8(Reg8::L, 4),
            0x34 => self.inc_hl(12),
            0x3D => self.dec_reg8(Reg8::A, 4),
            0x05 => self.dec_reg8(Reg8::B, 4),
            0x0D => self.dec_reg8(Reg8::C, 4),
            0x15 => self.dec_reg8(Reg8::D, 4),
            0x1D => self.dec_reg8(Reg8::E, 4),
            0x25 => self.dec_reg8(Reg8::H, 4),
            0x2D => self.dec_reg8(Reg8::L, 4),
            0x35 => self.dec_hl(12),

            // 16-bit arithmetic
            0x09 => {
                let v = self.get_bc();
                self.add_hl16(v, 8);
            }
            0x19 => {
                let v = self.get_de();
                self.add_hl16(v, 8);
            }
            0x29 => {
                let v = self.get_hl();
                self.add_hl16(v, 8);
            }
            0x39 => {
                let v = self.sp;
                self.add_hl16(v, 8);
            }
            0x03 => self.inc16(Reg16::Bc, 8),
            0x13 => self.inc16(Reg16::De, 8),
            0x23 => self.inc16(Reg16::Hl, 8),
            0x33 => self.inc16(Reg16::Sp, 8),
            0x0B => self.dec16(Reg16::Bc, 8),
            0x1B => self.dec16(Reg16::De, 8),
            0x2B => self.dec16(Reg16::Hl, 8),
            0x3B => self.dec16(Reg16::Sp, 8),

            // jumps
            0xE9 => {
                self.cycles_this_update += 4;
                self.pc = self.get_hl();
            }
            0xC3 => self.jump_absolute(),
            0xC2 => self.jump_conditional(FLAG_Z_BIT, false),
            0xCA => self.jump_conditional(FLAG_Z_BIT, true),
            0xD2 => self.jump_conditional(FLAG_C_BIT, false),
            0xDA => self.jump_conditional(FLAG_C_BIT, true),

            // relative jumps
            0x18 => self.jump_relative(None, false),
            0x20 => self.jump_relative(Some(FLAG_Z_BIT), false),
            0x28 => self.jump_relative(Some(FLAG_Z_BIT), true),
            0x30 => self.jump_relative(Some(FLAG_C_BIT), false),
            0x38 => self.jump_relative(Some(FLAG_C_BIT), true),

            // calls
            0xCD => self.call(None, false),
            0xC4 => self.call(Some(FLAG_Z_BIT), false),
            0xCC => self.call(Some(FLAG_Z_BIT), true),
            0xD4 => self.call(Some(FLAG_C_BIT), false),
            0xDC => self.call(Some(FLAG_C_BIT), true),

            // returns
            0xC9 => self.ret(None, false),
            0xC0 => self.ret(Some(FLAG_Z_BIT), false),
            0xC8 => self.ret(Some(FLAG_Z_BIT), true),
            0xD0 => self.ret(Some(FLAG_C_BIT), false),
            0xD8 => self.ret(Some(FLAG_C_BIT), true),

            // restarts
            0xC7 => self.restart(0x00),
            0xCF => self.restart(0x08),
            0xD7 => self.restart(0x10),
            0xDF => self.restart(0x18),
            0xE7 => self.restart(0x20),
            0xEF => self.restart(0x28),
            0xF7 => self.restart(0x30),
            0xFF => self.restart(0x38),

            0x27 => self.daa(),
            0xCB => self.execute_extended_opcode(),

            // accumulator rotates
            0x07 => self.unary_accum(Unary::Rlc, 8),
            0x0F => self.unary_accum(Unary::Rrc, 8),
            0x17 => self.unary_accum(Unary::Rl, 8),
            0x1F => self.unary_accum(Unary::Rr, 8),

            // return from interrupt
            0xD9 => {
                self.pc = self.pop_word_off_stack();
                self.enable_interrupts = true;
                self.cycles_this_update += 8;
            }

            // load SP into little-endian address nn
            0x08 => {
                let address = self.read_word();
                self.pc = self.pc.wrapping_add(2);
                let sp = self.sp;
                self.write_byte(address, sp as u8);
                self.write_byte(address.wrapping_add(1), (sp >> 8) as u8);
                self.cycles_this_update += 20;
            }

            // load immediate into memory at HL
            0x36 => {
                self.cycles_this_update += 12;
                let value = self.immediate_byte();
                self.write_byte(self.get_hl(), value);
            }

            // load A from address nn
            0xFA => {
                self.cycles_this_update += 16;
                let address = self.read_word();
                self.pc = self.pc.wrapping_add(2);
                self.a = self.read_memory(address);
            }

            0x3E => {
                self.cycles_this_update += 8;
                self.a = self.immediate_byte();
            }

            // store A at address nn
            0xEA => {
                self.cycles_this_update += 16;
                let address = self.read_word();
                self.pc = self.pc.wrapping_add(2);
                self.write_byte(address, self.a);
            }

            0xF3 => {
                self.pending_interrupt_disabled = true;
                self.cycles_this_update += 4;
            }
            0xFB => {
                self.pending_interrupt_enabled = true;
                self.cycles_this_update += 4;
            }

            0xE0 => {
                let offset = self.immediate_byte();
                let address = 0xFF00 + offset as u16;
                self.write_byte(address, self.a);
                self.cycles_this_update += 12;
            }
            0xF0 => {
                let offset = self.immediate_byte();
                let address = 0xFF00 + offset as u16;
                self.a = self.read_memory(address);
                self.cycles_this_update += 12;
            }

            0x2F => {
                self.cycles_this_update += 4;
                self.a ^= 0xFF;
                self.f |= FLAG_N;
                self.f |= FLAG_H;
            }

            0x76 => {
                self.cycles_this_update += 4;
                if self.rom[0xFF0F] & self.rom[0xFFFF] & 0x1F != 0 {
                    if self.enable_interrupts {
                        self.pc = self.pc.wrapping_sub(1);
                    } else {
                        self.halt_bug = true;
                    }
                } else {
                    self.halted = true;
                }
            }

            0x3F => {
                self.cycles_this_update += 4;
                if self.flag(FLAG_C_BIT) {
                    self.f &= !FLAG_C;
                } else {
                    self.f |= FLAG_C;
                }
                self.f &= !FLAG_H;
                self.f &= !FLAG_N;
            }

            0x37 => {
                self.cycles_this_update += 4;
                self.f |= FLAG_C;
                self.f &= !FLAG_H;
                self.f &= !FLAG_N;
            }

            0xF8 => {
                // LD HL, SP+e
                self.cycles_this_update += 12;
                let offset = self.immediate_byte();
                let sp = self.sp;
                self.set_hl(sp.wrapping_add(offset as i8 as u16));
                self.f = 0;
                if (sp & 0xF) + (offset & 0xF) as u16 > 0xF {
                    self.f |= FLAG_H;
                }
                if (sp & 0xFF) as u16 + offset as u16 > 0xFF {
                    self.f |= FLAG_C;
                }
            }

            0xE8 => {
                // ADD SP, e
                self.cycles_this_update += 16;
                let offset = self.immediate_byte();
                let sp = self.sp;
                self.sp = sp.wrapping_add(offset as i8 as u16);
                self.f = 0;
                if (sp & 0xF) + (offset & 0xF) as u16 > 0xF {
                    self.f |= FLAG_H;
                }
                if (sp & 0xFF) as u16 + offset as u16 > 0xFF {
                    self.f |= FLAG_C;
                }
            }

            0x10 => {
                self.pc = self.pc.wrapping_add(1);
                self.cycles_this_update += 4;
            }

            other => panic!("unhandled opcode {:#x}", other),
        }
    }

    pub(crate) fn execute_extended_opcode(&mut self) {
        let pc = self.pc;
        let mut opcode = self.rom[pc as usize];
        if (0x4000..=0x7FFF).contains(&pc) || (0xA000..=0xBFFF).contains(&pc) {
            opcode = self.read_memory(pc);
        }
        self.pc = self.pc.wrapping_add(1);

        let x = opcode >> 6;
        let y = (opcode >> 3) & 7;
        let z = opcode & 7;

        match x {
            0 => {
                let op = match y {
                    0 => Unary::Rlc,
                    1 => Unary::Rrc,
                    2 => Unary::Rl,
                    3 => Unary::Rr,
                    4 => Unary::Sla,
                    5 => Unary::Sra,
                    6 => Unary::Swap,
                    7 => Unary::Srl,
                    _ => unreachable!(),
                };
                if z == 6 {
                    // SRL on memory uses the same cycle count as its register
                    // form in the original implementation.
                    let cycles = if matches!(op, Unary::Srl) { 8 } else { 16 };
                    self.unary_hl(op, cycles);
                } else {
                    self.unary_reg(Self::reg_from_z(z), op, 8);
                }
            }
            1 => {
                if z == 6 {
                    self.test_bit_hl(y);
                } else {
                    self.test_bit_reg(Self::reg_from_z(z), y, 8);
                }
            }
            2 => {
                if z == 6 {
                    self.reset_bit_hl(y);
                } else {
                    self.reset_bit_reg(Self::reg_from_z(z), y);
                }
            }
            3 => {
                if z == 6 {
                    self.set_bit_hl(y);
                } else {
                    self.set_bit_reg(Self::reg_from_z(z), y);
                }
            }
            _ => unreachable!(),
        }
    }
}

#[cfg(test)]
mod daa_tests {
    use super::*;
    use crate::Emulator;

    fn update_crc(bytes: &mut [u8; 4], value: u8) {
        let mut b = bytes[3];
        let mut c = bytes[2];
        let mut d = bytes[1];
        let mut a = bytes[0] ^ value;
        for _ in 0..8 {
            let mut carry = b & 1;
            b >>= 1;
            let next = c & 1;
            c = (c >> 1) | (carry << 7);
            carry = next;
            let next = d & 1;
            d = (d >> 1) | (carry << 7);
            carry = next;
            let next = a & 1;
            a = (a >> 1) | (carry << 7);
            carry = next;
            if carry != 0 {
                b ^= 0xED;
                c ^= 0xB8;
                d ^= 0x83;
                a ^= 0x20;
            }
        }
        *bytes = [a, d, c, b];
    }

    #[test]
    fn daa_matches_blargg_checksum() {
        let mut emu = Emulator::new();
        let mut crc = [0xFFu8; 4];
        for e in (0..=255u8).step_by(0x10) {
            for d in 0..=255u8 {
                emu.a = d;
                emu.f = e;
                emu.daa();
                let a = emu.a;
                let f = emu.f;
                update_crc(&mut crc, a);
                update_crc(&mut crc, f);
            }
        }
        let raw = u32::from_be_bytes([crc[3], crc[2], crc[1], crc[0]]);
        assert_eq!(raw, 0x9560_7275, "raw crc {raw:#010x}");
    }
}
