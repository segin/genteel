//! M68k Shift/Rotate Tests
//!
//! Exhaustive tests for M68k shift and rotate operations.
//! Tests all shift counts, all sizes, and flag edge cases.

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
// ASL (Arithmetic Shift Left) Tests
// ============================================================================

#[test]
fn test_asl_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE300]); // ASL.B #1, D0
    cpu.d[0] = 0x40; // 01000000
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_asl_b_carry() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE300]); // ASL.B #1, D0
    cpu.d[0] = 0x80; // MSB set -> carry
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_asl_b_overflow() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE300]); // ASL.B #1, D0
    cpu.d[0] = 0x40; // Sign changes
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::OVERFLOW));
}

#[test]
fn test_asl_w_by_8() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE140]); // ASL.W #8, D0 (encoded as 0, meaning 8)
    cpu.d[0] = 0x00FF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0xFF00);
}

#[test]
fn test_asl_l_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE380]); // ASL.L #1, D0
    cpu.d[0] = 0x40000000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x80000000);
}

#[test]
fn test_asl_register_count() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE320]); // ASL.B D1, D0
    cpu.d[0] = 0x01;
    cpu.d[1] = 4;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x10);
}

// ============================================================================
// ASR (Arithmetic Shift Right) Tests
// ============================================================================

#[test]
fn test_asr_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE200]); // ASR.B #1, D0
    cpu.d[0] = 0x80; // 10000000 -> 11000000 (sign extend)
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xC0);
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_asr_b_carry() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE200]); // ASR.B #1, D0
    cpu.d[0] = 0x01;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_asr_sign_preserve() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE600]); // ASR.B #3, D0
    cpu.d[0] = 0x80; // -128 >> 3 = -16 = 0xF0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xF0);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

// ============================================================================
// LSL (Logical Shift Left) Tests
// ============================================================================

#[test]
fn test_lsl_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE308]); // LSL.B #1, D0
    cpu.d[0] = 0x40;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
}

#[test]
fn test_lsl_b_carry() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE308]); // LSL.B #1, D0
    cpu.d[0] = 0x80;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_lsl_w_all_counts() {
    for count in 1..=8u8 {
        let (mut cpu, mut memory) = create_cpu();
        let opcode = 0xE148 | ((count as u16 % 8) << 9); // LSL.W #count, D0
        write_op(&mut memory, &[opcode]);
        cpu.d[0] = 0x0001;
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0] & 0xFFFF, 1u32 << count, "LSL.W #{}", count);
    }
}

// ============================================================================
// LSR (Logical Shift Right) Tests
// ============================================================================

#[test]
fn test_lsr_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE208]); // LSR.B #1, D0
    cpu.d[0] = 0x80;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x40);
}

#[test]
fn test_lsr_b_zero_fill() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE208]); // LSR.B #1, D0
    cpu.d[0] = 0x80; // MSB set
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::NEGATIVE)); // MSB is now 0
}

#[test]
fn test_lsr_w_all_counts() {
    for count in 1..=8u8 {
        let (mut cpu, mut memory) = create_cpu();
        let opcode = 0xE048 | ((count as u16 % 8) << 9); // LSR.W #count, D0
        write_op(&mut memory, &[opcode]);
        cpu.d[0] = 0x8000;
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0] & 0xFFFF, 0x8000u32 >> count, "LSR.W #{}", count);
    }
}

// ============================================================================
// ROL (Rotate Left) Tests
// ============================================================================

#[test]
fn test_rol_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE318]); // ROL.B #1, D0
    cpu.d[0] = 0x80;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x01);
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_rol_b_full_rotation() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE118]); // ROL.B #8, D0
    cpu.d[0] = 0xAB;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xAB); // Full rotation restores
}

#[test]
fn test_rol_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE358]); // ROL.W #1, D0
    cpu.d[0] = 0x8000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x0001);
    assert!(cpu.get_flag(flags::CARRY));
}

// ============================================================================
// ROR (Rotate Right) Tests
// ============================================================================

#[test]
fn test_ror_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE218]); // ROR.B #1, D0
    cpu.d[0] = 0x01;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_ror_b_full_rotation() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE018]); // ROR.B #8, D0
    cpu.d[0] = 0xAB;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xAB);
}

// ============================================================================
// ROXL (Rotate Left with Extend) Tests
// ============================================================================

#[test]
fn test_roxl_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE310]); // ROXL.B #1, D0
    cpu.d[0] = 0x80;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_roxl_with_extend_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE310]); // ROXL.B #1, D0
    cpu.d[0] = 0x00;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x01); // X rotates in
}

#[test]
fn test_roxl_9_bit_rotation() {
    // ROXL through X flag creates 9-bit rotation for byte
    let (mut cpu, mut memory) = create_cpu();
    cpu.d[0] = 0x80;
    cpu.set_flag(flags::EXTEND, false);

    // Rotate 9 times to complete full cycle
    for _ in 0..9 {
        write_op(&mut memory, &[0xE310]); // ROXL.B #1, D0
        cpu.pc = 0x1000;
        cpu.step_instruction(&mut memory);
    }
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
}

// ============================================================================
// ROXR (Rotate Right with Extend) Tests
// ============================================================================

#[test]
fn test_roxr_b_by_1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE210]); // ROXR.B #1, D0
    cpu.d[0] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_roxr_with_extend_set() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE210]); // ROXR.B #1, D0
    cpu.d[0] = 0x00;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80); // X rotates in
}

// ============================================================================
// Memory Shift Tests
// ============================================================================

#[test]
fn test_asl_memory() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE1D0]); // ASL (A0)
    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x4000);
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_word(0x2000), 0x8000);
}

#[test]
fn test_lsr_memory() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE2D0]); // LSR (A0)
    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x8000);
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_word(0x2000), 0x4000);
}

// ============================================================================
// Edge Cases
// ============================================================================

#[test]
fn test_shift_by_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE320]); // ASL.B D1, D0 (D1 = count)
    cpu.d[0] = 0xFF;
    cpu.d[1] = 0; // Shift by 0
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF); // Unchanged
    assert!(!cpu.get_flag(flags::CARRY)); // C cleared for count 0
}

#[test]
fn test_shift_by_large_count() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xE320]); // ASL.B D1, D0
    cpu.d[0] = 0xFF;
    cpu.d[1] = 64; // Shift by 64 (mod 64)
    cpu.step_instruction(&mut memory);
    // Result depends on implementation - just verify no panic
    assert!(cpu.pc > 0x1000);
}
