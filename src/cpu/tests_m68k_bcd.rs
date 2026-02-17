//! M68k BCD Tests
//!
//! Tests for BCD arithmetic operations (ABCD, SBCD, NBCD).

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
// SBCD Tests
// ============================================================================

#[test]
fn test_sbcd_reg_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    // Opcode: 1000 001 1 0000 0 000 = 0x8300
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x11; // Src
    cpu.d[1] = 0x22; // Dest
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true); // Pre-set Z (SBCD expects Z set usually)

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[1] & 0xFF, 0x11); // 22 - 11 = 11
    assert!(!cpu.get_flag(flags::ZERO)); // Result non-zero, Z cleared
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_borrow_low() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x05;
    cpu.d[1] = 0x14; // 14 - 05 = 09
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[1] & 0xFF, 0x09);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_borrow_high() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x20;
    cpu.d[1] = 0x10; // 10 - 20 = 90 + borrow
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[1] & 0xFF, 0x90);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x10; // 10 - 00 - 1 = 09
    cpu.set_flag(flags::EXTEND, true);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[1] & 0xFF, 0x09);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND)); // Borrow resolved
}

#[test]
fn test_sbcd_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD -(A1), -(A0)
    // Opcode: 1000 000 1 0000 1 001 = 0x8109
    // Rx=A0 (000), Ry=A1 (001)
    write_op(&mut memory, &[0x8109]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x1FFF, 0x22); // Dest
    memory.write_byte(0x2FFF, 0x11); // Src
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x1FFF), 0x11); // 22 - 11 = 11
    assert_eq!(cpu.a[0], 0x1FFF);
    assert_eq!(cpu.a[1], 0x2FFF);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_sbcd_zero_flag_persistence() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D0, D1
    write_op(&mut memory, &[0x8300]);
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x00;
    cpu.set_flag(flags::EXTEND, false);

    // Case 1: Z is set initially. Result 0. Z should remain set.
    cpu.set_flag(flags::ZERO, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));

    // Case 2: Z is clear initially. Result 0. Z should remain clear.
    cpu.pc = 0x1000; // Reset PC to run same instruction
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x00;
    cpu.set_flag(flags::ZERO, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_sbcd_chain() {
    let (mut cpu, mut memory) = create_cpu();
    // Simulate 100 - 1 = 099
    // Memory layout:
    // 0x2002: 0x01 (Low byte of 100) -> Wait, 100 is 0x0100 BCD.
    // Let's do 2-byte BCD: 0x0100 (100) - 0x0001 (1)

    // Address A0 points to end of destination (0x0100)
    // Address A1 points to end of source (0x0001)

    cpu.a[0] = 0x2002;
    cpu.a[1] = 0x3002;

    // Dest: 01 00
    memory.write_byte(0x2000, 0x01);
    memory.write_byte(0x2001, 0x00);

    // Src: 00 01
    memory.write_byte(0x3000, 0x00);
    memory.write_byte(0x3001, 0x01);

    // SBCD -(A1), -(A0)
    write_op(&mut memory, &[0x8109, 0x8109]); // Run twice

    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true); // Initialize Z for multi-precision

    // First byte: 00 - 01 = 99, borrow
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2001), 0x99);
    assert!(cpu.get_flag(flags::EXTEND)); // Borrow
    assert!(!cpu.get_flag(flags::ZERO));  // Result non-zero

    // Second byte: 01 - 00 - 1 (borrow) = 00
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2000), 0x00);
    assert!(!cpu.get_flag(flags::EXTEND)); // No borrow
    // Z flag logic: If result is zero, Z is unchanged.
    // Previous result was non-zero, so Z was cleared.
    // So final Z should be clear, indicating the whole 2-byte number is non-zero.
    assert!(!cpu.get_flag(flags::ZERO));
}
