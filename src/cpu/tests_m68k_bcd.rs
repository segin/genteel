//! M68k BCD Instructions Tests
//!
//! Tests for ABCD, SBCD, NBCD instructions.

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr |= flags::SUPERVISOR;
    (cpu, memory)
}

fn write_op(memory: &mut Memory, opcodes: &[u16]) {
    let mut addr = 0x1000u32;
    for &op in opcodes {
        memory.write_word(addr, op);
        addr += 2;
    }
}

// ============================================================================
// ABCD Tests
// ============================================================================

#[test]
fn test_abcd_d0_d1_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // ABCD D0, D1
    // Opcode: 1100 001 1 0000 0 000 = 0xC300
    write_op(&mut memory, &[0xC300]);
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x20;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x30);
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_abcd_d0_d1_lower_adjust() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x09;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 9 + 1 = 10 (0x0A) -> Adjusted to 0x10
    assert_eq!(cpu.d[1] & 0xFF, 0x10);
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_abcd_d0_d1_upper_adjust() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x90;
    cpu.d[1] = 0x10;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 90 + 10 = 100 (0xA0) -> Adjusted to 0x00 with Carry
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_abcd_d0_d1_both_adjust() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x99;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 99 + 1 = 100 -> 0x00 with Carry
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_abcd_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x20;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    // 10 + 20 + 1 = 31 (0x31)
    assert_eq!(cpu.d[1] & 0xFF, 0x31);
}

#[test]
fn test_abcd_z_flag_clearing() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x01;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::ZERO, true); // Initially set
    cpu.step_instruction(&mut memory);
    // Result 0x02 != 0, so Z should be cleared
    assert_eq!(cpu.d[1] & 0xFF, 0x02);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_abcd_z_flag_unchanged() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC300]); // ABCD D0, D1
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x00;
    cpu.set_flag(flags::ZERO, true); // Initially set
    cpu.step_instruction(&mut memory);
    // Result 0x00 == 0, so Z should be unchanged (remain set)
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));

    // Test unchanged when initially clear
    cpu.pc = 0x1000; // Reset PC to run again
    cpu.set_flag(flags::ZERO, false); // Initially clear
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO)); // Should remain clear
}

#[test]
fn test_abcd_memory_mode() {
    let (mut cpu, mut memory) = create_cpu();
    // ABCD -(A0), -(A1)
    // Opcode: 1100 001 1 0000 1 000 = 0xC308
    write_op(&mut memory, &[0xC308]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x1FFF, 0x15);
    memory.write_byte(0x2FFF, 0x25);

    cpu.step_instruction(&mut memory);

    // Result at 0x2FFF (pre-decremented A1)
    // 15 + 25 = 40 (0x40)
    assert_eq!(memory.read_byte(0x2FFF), 0x40);
    assert_eq!(cpu.a[0], 0x1FFF);
    assert_eq!(cpu.a[1], 0x2FFF);
}

// ============================================================================
// SBCD Tests
// ============================================================================

#[test]
fn test_sbcd_d0_d1_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    // Opcode: 1000 001 1 0000 0 000 = 0x8300
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x30;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 30 - 10 = 20 (0x20)
    assert_eq!(cpu.d[1] & 0xFF, 0x20);
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_sbcd_borrow() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x8300]); // SBCD D0, D1
    cpu.d[0] = 0x20;
    cpu.d[1] = 0x10;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 10 - 20 = -10 -> 90 with borrow
    assert_eq!(cpu.d[1] & 0xFF, 0x90);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x8300]); // SBCD D0, D1
    cpu.d[0] = 0x05;
    cpu.d[1] = 0x10;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    // 10 - 5 - 1 = 4 (0x04)
    assert_eq!(cpu.d[1] & 0xFF, 0x04);
}

#[test]
fn test_sbcd_z_flag() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x8300]); // SBCD D0, D1
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x10;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 10 - 10 = 00 -> Z remains set
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));
}

// ============================================================================
// NBCD Tests
// ============================================================================

#[test]
fn test_nbcd_basic() {
    let (mut cpu, mut memory) = create_cpu();
    // NBCD D0
    // Opcode: 0100 100 0 00 000 000 = 0x4800
    write_op(&mut memory, &[0x4800]);
    cpu.d[0] = 0x10;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 0 - 10 = 90 with borrow
    assert_eq!(cpu.d[0] & 0xFF, 0x90);
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_nbcd_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4800]); // NBCD D0
    cpu.d[0] = 0x00;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    // 0 - 0 = 0
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_nbcd_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4800]); // NBCD D0
    cpu.d[0] = 0x00;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    // 0 - 0 - 1 = 99 (0x99) with borrow
    assert_eq!(cpu.d[0] & 0xFF, 0x99);
    assert!(cpu.get_flag(flags::CARRY));
}
