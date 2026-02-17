//! M68k BCD Arithmetic Tests
//!
//! Comprehensive unit tests for M68k BCD (Binary Coded Decimal) instructions.
//! Focuses on SBCD, ABCD, and NBCD logic.

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
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
    // SBCD D1, D0
    // Opcode: 1000 000 1 0000 0 001 = 0x8101
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x45; // 45
    cpu.d[1] = 0x23; // 23
    cpu.set_flag(flags::ZERO, true); // Z starts set
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 45 - 23 = 22
    assert_eq!(cpu.d[0] & 0xFF, 0x22);
    assert!(!cpu.get_flag(flags::ZERO)); // Z cleared because result non-zero
    assert!(!cpu.get_flag(flags::EXTEND)); // No borrow
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_sbcd_reg_borrow() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x23; // 23
    cpu.d[1] = 0x45; // 45
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 23 - 45 = 78 (modulo 100) and Borrow
    // 123 - 45 = 78
    assert_eq!(cpu.d[0] & 0xFF, 0x78);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_sbcd_reg_input_borrow() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x45; // 45
    cpu.d[1] = 0x23; // 23
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, true); // Borrow In

    cpu.step_instruction(&mut memory);

    // 45 - 23 - 1 = 21
    assert_eq!(cpu.d[0] & 0xFF, 0x21);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_correction_low() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x15; // 15
    cpu.d[1] = 0x08; // 08
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 15 - 08 = 07
    // Low nibble: 5 - 8 = -3 -> subtract 6 -> -9. Borrow from high.
    // High nibble: 1 - 0 - 1 = 0.
    // Result: 07.
    assert_eq!(cpu.d[0] & 0xFF, 0x07);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_correction_high() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x80; // 80
    cpu.d[1] = 0x81; // 81
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 80 - 81 = 99 and Borrow
    // 180 - 81 = 99
    assert_eq!(cpu.d[0] & 0xFF, 0x99);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_zero_flag_unchanged_if_zero() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x55;
    cpu.d[1] = 0x55;
    cpu.set_flag(flags::ZERO, true); // Z starts set
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 55 - 55 = 00
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO)); // Z remains set
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_zero_flag_cleared_if_nonzero() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x56;
    cpu.d[1] = 0x55;
    cpu.set_flag(flags::ZERO, true); // Z starts set
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 56 - 55 = 01
    assert_eq!(cpu.d[0] & 0xFF, 0x01);
    assert!(!cpu.get_flag(flags::ZERO)); // Z cleared
}

#[test]
fn test_sbcd_zero_flag_unchanged_if_zero_input_nonzero() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[0x8101]);

    cpu.d[0] = 0x55;
    cpu.d[1] = 0x55;
    cpu.set_flag(flags::ZERO, false); // Z starts clear
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 55 - 55 = 00
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(!cpu.get_flag(flags::ZERO)); // Z remains clear (unchanged)
}

#[test]
fn test_sbcd_mem_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD -(A1), -(A0)
    // Opcode: 1000 000 1 0000 1 001 = 0x8109
    write_op(&mut memory, &[0x8109]);

    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x45);
    memory.write_byte(0x3000, 0x23); // Src is A1? No, wait.
    // SBCD Dy, Dx -> Dest = Dx - Dy - X
    // SBCD -(Ay), -(Ax) -> Dest = (Ax) - (Ay) - X
    // Opcode 1000 Rx 1 0000 1 Ry
    // Rx is destination register (Ax)
    // Ry is source register (Ay)
    // So Rx=0 (A0), Ry=1 (A1) -> 0x8109

    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 45 - 23 = 22
    assert_eq!(memory.read_byte(0x2000), 0x22);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);
}

#[test]
fn test_sbcd_multi_byte_chain() {
    let (mut cpu, mut memory) = create_cpu();
    // Multi-byte subtraction: 400 - 1 = 399
    // Bytes: 04 00 - 00 01 = 03 99

    // Low byte: SBCD -(A1), -(A0)
    // High byte: SBCD -(A1), -(A0)
    write_op(&mut memory, &[0x8109, 0x8109]);

    cpu.a[0] = 0x2002;
    cpu.a[1] = 0x3002;
    memory.write_byte(0x2000, 0x04);
    memory.write_byte(0x2001, 0x00);

    memory.write_byte(0x3000, 0x00);
    memory.write_byte(0x3001, 0x01);

    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    // Step 1: Low byte
    // 00 - 01 = 99, Borrow
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2001), 0x99);
    assert!(cpu.get_flag(flags::EXTEND));

    // Step 2: High byte
    // 04 - 00 - 1 = 03
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2000), 0x03);
    assert!(!cpu.get_flag(flags::EXTEND));

    // Result: 0399
}

// ============================================================================
// ABCD Tests (Brief Check)
// ============================================================================

#[test]
fn test_abcd_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // ABCD D1, D0
    write_op(&mut memory, &[0xC101]);

    cpu.d[0] = 0x45;
    cpu.d[1] = 0x23;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 45 + 23 = 68
    assert_eq!(cpu.d[0] & 0xFF, 0x68);
    assert!(!cpu.get_flag(flags::ZERO));
}

// ============================================================================
// NBCD Tests (Brief Check)
// ============================================================================

#[test]
fn test_nbcd_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // NBCD D0
    // Opcode: 0100 100 0 00 000 000 = 0x4800
    write_op(&mut memory, &[0x4800]);

    cpu.d[0] = 0x45;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 0 - 45 = 55 (borrow)
    assert_eq!(cpu.d[0] & 0xFF, 0x55);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::EXTEND));
}
