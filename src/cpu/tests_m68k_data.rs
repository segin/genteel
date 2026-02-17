//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement operations.
//! Tests all sizes, flag combinations, and edge cases.

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
// MOVE Tests
// ============================================================================

#[test]
fn test_move_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1
    // Opcode: 0001 (Move.B) 001 (D1) 000 (Mode=Dn) 000 (Mode=Dn) 000 (D0) -> 0x1200
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x55;
    cpu.d[1] = 0xFFFFFFAA;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x55);
    assert_eq!(cpu.d[1] & 0xFFFFFF00, 0xFFFFFF00); // Upper bits unchanged
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, D1
    // Opcode: 0011 (Move.W) 001 (D1) 000 (Mode=Dn) 000 (Mode=Dn) 000 (D0) -> 0x3200
    write_op(&mut memory, &[0x3200]);
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0xFFFF5678;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFFFF, 0x1234);
    assert_eq!(cpu.d[1] & 0xFFFF0000, 0xFFFF0000); // Upper bits unchanged
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1
    // Opcode: 0010 (Move.L) 001 (D1) 000 (Mode=Dn) 000 (Mode=Dn) 000 (D0) -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x00000000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x12345678);
}

#[test]
fn test_move_flags_zero() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1 -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_flags_negative() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1 -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0xFFFFFFFF; // -1
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_immediate_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L #$12345678, D0
    // Opcode: 0010 (Move.L) 000 (D0) 000 (Mode=Dn) 111 (Mode=Imm) 100 (Reg=4) -> 0x203C
    write_op(&mut memory, &[0x203C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x12345678);
}

#[test]
fn test_move_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, (A0)
    // Opcode: 0010 (Move.L) 000 (A0) 010 (Mode=(An)) 000 (Mode=Dn) 000 (D0) -> 0x2080
    write_op(&mut memory, &[0x2080]);
    cpu.d[0] = 0xDEADBEEF;
    cpu.a[0] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_long(0x2000), 0xDEADBEEF);
}

#[test]
fn test_move_from_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L (A0), D0
    // Opcode: 0010 (Move.L) 000 (D0) 000 (Mode=Dn) 010 (Mode=(An)) 000 (A0) -> 0x2010
    write_op(&mut memory, &[0x2010]);
    cpu.a[0] = 0x3000;
    memory.write_long(0x3000, 0xCAFEBABE);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xCAFEBABE);
}

#[test]
fn test_move_postinc() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W (A0)+, D0
    // Opcode: 0011 (Move.W) 000 (D0) 000 (Mode=Dn) 011 (Mode=(An)+) 000 (A0) -> 0x3018
    write_op(&mut memory, &[0x3018]);
    cpu.a[0] = 0x4000;
    memory.write_word(0x4000, 0x1234);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
    assert_eq!(cpu.a[0], 0x4002); // Incremented by 2
}

#[test]
fn test_move_predec() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L -(A0), D0
    // Opcode: 0010 (Move.L) 000 (D0) 000 (Mode=Dn) 100 (Mode=-(An)) 000 (A0) -> 0x2020
    write_op(&mut memory, &[0x2020]);
    cpu.a[0] = 0x5004;
    memory.write_long(0x5000, 0x87654321);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x87654321);
    assert_eq!(cpu.a[0], 0x5000); // Decremented by 4
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w_sign_extend() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.W #$FFFF, A0
    // Opcode: 0011 (Move.W) 001 (A0) 001 (Mode=An) 111 (Mode=Imm) 100 (Reg=4) -> 0x307C
    // Wait, MOVEA destination is bits 11-9 (Register) and 8-6 (Mode=001).
    // Opcode: 0011 000 001 111 100 -> 0x307C
    write_op(&mut memory, &[0x307C, 0xFFFF]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0xFFFFFFFF); // Sign extended

    // Flags should NOT be affected by MOVEA
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::NEGATIVE, false);

    cpu.pc = 0x1000;
    cpu.step_instruction(&mut memory);

    assert!(cpu.get_flag(flags::ZERO)); // Unchanged
    assert!(!cpu.get_flag(flags::NEGATIVE)); // Unchanged
}

#[test]
fn test_movea_l() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.L #$12345678, A0
    // Opcode: 0010 000 001 111 100 -> 0x207C
    write_op(&mut memory, &[0x207C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x12345678);
}
