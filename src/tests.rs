//! Unit tests for the CPU, memory map and frame loop.

use crate::Emulator;
use crate::constants::{FLAG_C, FLAG_H, FLAG_N, FLAG_Z, TARGET_CYCLES};

fn with_program(program: &[u8]) -> Emulator {
    let mut emulator = Emulator::new();
    emulator.rom[0x100..0x100 + program.len()].copy_from_slice(program);
    emulator.pc = 0x100;
    emulator.sp = 0xFFFE;
    emulator
}

fn step(emulator: &mut Emulator) {
    emulator.execute_next_opcode();
}

#[test]
fn nop_advances_four_cycles() {
    let mut emu = with_program(&[0x00]);
    step(&mut emu);
    assert_eq!(emu.program_counter(), 0x101);
    assert_eq!(emu.cycles_this_update, 4);
    assert_eq!(emu.total_opcodes(), 1);
}

#[test]
fn immediate_loads_and_register_moves() {
    let mut emu = with_program(&[0x3E, 0x42, 0x47]);
    step(&mut emu);
    step(&mut emu);
    assert_eq!(emu.registers().0 >> 8, 0x42);
    assert_eq!(emu.registers().1 >> 8, 0x42);
    assert_eq!(emu.program_counter(), 0x103);
    assert_eq!(emu.cycles_this_update, 12);
}

#[test]
fn add_sets_half_carry() {
    let mut emu = with_program(&[0x3E, 0x0F, 0xC6, 0x01]);
    step(&mut emu);
    step(&mut emu);
    assert_eq!(emu.registers().0 >> 8, 0x10);
    assert!(emu.half_carry_flag());
    assert!(!emu.zero_flag());
    assert!(!emu.carry_flag());
    assert_eq!(emu.cycles_this_update, 16);
}

#[test]
fn subtraction_sets_carry_and_half_carry() {
    let mut emu = with_program(&[0x3E, 0x00, 0xD6, 0x01]);
    step(&mut emu);
    step(&mut emu);
    assert_eq!(emu.registers().0 >> 8, 0xFF);
    assert!(emu.carry_flag());
    assert!(emu.half_carry_flag());
    assert!(emu.subtract_flag());
}

#[test]
fn increment_wraps_and_sets_zero() {
    let mut emu = with_program(&[0x3C]);
    emu.a = 0xFF;
    step(&mut emu);
    assert_eq!(emu.a, 0);
    assert!(emu.zero_flag());
    assert!(emu.half_carry_flag());
    assert!(!emu.subtract_flag());
}

#[test]
fn absolute_and_relative_jumps() {
    let mut emu = with_program(&[0xC3, 0x50, 0x00]);
    step(&mut emu);
    assert_eq!(emu.program_counter(), 0x50);
    assert_eq!(emu.cycles_this_update, 12);

    let mut emu = with_program(&[0x18, 0xFE]);
    step(&mut emu);
    assert_eq!(emu.program_counter(), 0x100);
    assert_eq!(emu.cycles_this_update, 8);
}

#[test]
fn call_pushes_return_address_and_ret_restores_it() {
    let mut emu = with_program(&[0xCD, 0x10, 0x01]);
    emu.rom[0x110] = 0xC9;
    step(&mut emu);
    assert_eq!(emu.program_counter(), 0x110);
    assert_eq!(emu.sp, 0xFFFC);
    assert_eq!(emu.read_memory(0xFFFC), 0x03);
    assert_eq!(emu.read_memory(0xFFFD), 0x01);

    step(&mut emu);
    assert_eq!(emu.program_counter(), 0x103);
    assert_eq!(emu.sp, 0xFFFE);
}

#[test]
fn extended_rlc_rotates_through_carry() {
    let mut emu = with_program(&[0xCB, 0x00]);
    emu.b = 0x80;
    step(&mut emu);
    assert_eq!(emu.b, 0x01);
    assert!(emu.carry_flag());
    assert!(!emu.zero_flag());
    assert_eq!(emu.cycles_this_update, 8);
}

#[test]
fn extended_bit_sets_zero_when_clear() {
    let mut emu = with_program(&[0xCB, 0x40]);
    emu.b = 0x01;
    step(&mut emu);
    assert!(!emu.zero_flag());
    assert!(emu.half_carry_flag());
    assert!(!emu.subtract_flag());
    assert_eq!(emu.cycles_this_update, 8);

    let mut emu = with_program(&[0xCB, 0x41]);
    emu.b = 0x01;
    step(&mut emu);
    assert!(emu.zero_flag());
}

#[test]
fn mbc1_bank_switch_and_rom_read() {
    let mut emu = with_program(&[]);
    emu.using_mbc1 = true;
    emu.write_byte(0x2000, 5);
    assert_eq!(emu.current_rom_bank, 5);
    emu.game_bank[5 * 0x4000] = 0xAB;
    assert_eq!(emu.read_memory(0x4000), 0xAB);
}

#[test]
fn echo_ram_and_dma_transfer() {
    let mut emu = with_program(&[]);
    emu.write_byte(0xE000, 0x5A);
    assert_eq!(emu.rom[0xE000], 0x5A);
    assert_eq!(emu.rom[0xC000], 0x5A);

    emu.rom[0xC123] = 0xAB;
    emu.write_byte(0xFF46, 0xC1);
    assert_eq!(emu.rom[0xFE00 + 0x23], 0xAB);
}

#[test]
fn daa_adjusts_after_addition() {
    let mut emu = with_program(&[0x27]);
    emu.a = 0x9A;
    step(&mut emu);
    assert_eq!(emu.a, 0x00);
    assert!(emu.zero_flag());
}

#[test]
fn update_runs_a_full_frame() {
    // An endless JP 0x100 keeps the CPU busy for the whole frame.
    let mut emu = with_program(&[0xC3, 0x00, 0x01]);
    emu.update();
    assert!(emu.total_opcodes() > 0);
    assert!(emu.cycles_this_update >= TARGET_CYCLES);
    assert_eq!(emu.program_counter(), 0x100);
}

#[test]
fn flags_are_publicly_observable() {
    let mut emu = Emulator::new();
    emu.f = FLAG_Z | FLAG_N | FLAG_H | FLAG_C;
    assert!(emu.zero_flag());
    assert!(emu.subtract_flag());
    assert!(emu.half_carry_flag());
    assert!(emu.carry_flag());
}


#[test]
fn pop_af_masks_the_low_nibble_of_f() {
    let mut emu = with_program(&[0xF1]);
    emu.sp = 0xFFFC;
    emu.write_byte(0xFFFC, 0xBF);
    emu.write_byte(0xFFFD, 0xAB);
    step(&mut emu);
    assert_eq!(emu.registers().0 >> 8, 0xAB);
    assert_eq!(emu.f, FLAG_Z | FLAG_H | FLAG_C);
}

#[test]
fn adc_propagates_carry_into_the_result() {
    let mut emu = with_program(&[0xCE, 0xFF]);
    emu.a = 0x00;
    emu.f = FLAG_C;
    step(&mut emu);
    assert_eq!(emu.a, 0x00);
    assert!(emu.carry_flag());
    assert!(emu.half_carry_flag());
    assert!(emu.zero_flag());
}

#[test]
fn sbc_propagates_borrow_into_the_result() {
    let mut emu = with_program(&[0xDE, 0x00]);
    emu.a = 0x00;
    emu.f = FLAG_C;
    step(&mut emu);
    assert_eq!(emu.a, 0xFF);
    assert!(emu.carry_flag());
    assert!(emu.half_carry_flag());
    assert!(emu.subtract_flag());
}

#[test]
fn cb_srl_and_swap_decode_to_distinct_ops() {
    let mut emu = with_program(&[0xCB, 0x38]);
    emu.b = 0xFF;
    step(&mut emu);
    assert_eq!(emu.b, 0x7F);
    assert!(emu.carry_flag());

    let mut emu = with_program(&[0xCB, 0x37]);
    emu.a = 0xAB;
    step(&mut emu);
    assert_eq!(emu.a, 0xBA);
}

#[test]
fn accumulator_rotates_always_clear_zero() {
    let mut emu = with_program(&[0x07]);
    emu.a = 0x00;
    emu.f = FLAG_Z;
    step(&mut emu);
    assert_eq!(emu.a, 0x00);
    assert!(!emu.zero_flag());
}

#[test]
fn halt_wakes_when_an_interrupt_arrives() {
    let mut emu = with_program(&[0x76, 0x00]);
    step(&mut emu);
    assert!(emu.halted);
    emu.rom[0xFF0F] = 1 << 2;
    emu.rom[0xFFFF] = 1 << 2;
    emu.do_interrupts();
    assert!(!emu.halted);
    assert_eq!(emu.program_counter(), 0x101);
}

#[test]
fn halt_bug_reexecutes_the_following_byte() {
    let mut emu = with_program(&[0x76, 0x11, 0x04, 0x0C]);
    emu.rom[0xFFFF] = 0x11;
    emu.rom[0xFF0F] = 0x10;
    step(&mut emu);
    assert!(!emu.halted);
    assert!(emu.halt_bug);
    step(&mut emu);
    assert!(!emu.halt_bug);
    assert_eq!(emu.get_de(), 0x0411);
}

#[test]
fn if_register_reads_with_upper_bits_set() {
    let emu = with_program(&[]);
    assert_eq!(emu.read_memory(0xFF0F), 0xE0);
}

#[test]
fn add_hl_sets_half_carry_from_bit_eleven() {
    let mut emu = with_program(&[0x09]);
    emu.set_hl(0x0FFF);
    emu.set_bc(0x0001);
    step(&mut emu);
    assert_eq!(emu.get_hl(), 0x1000);
    assert!(emu.half_carry_flag());
}
