//! M68k Data Movement Tests
//!
//! Tests for MOVE, MOVEA, etc.

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr = 0x2700; // Supervisor
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
// MOVE Tests (Register to Register)
// ============================================================================

#[test]
fn test_move_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1
    // Opcode: 1 (Byte) 001 (D1) 000 (DataReg) 000 (DataReg) 000 (D0) -> 0x1200
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xFFFFFFFF;

    cpu.step_instruction(&mut memory);

    // Only lower byte of D1 should change to 0x78
    assert_eq!(cpu.d[1], 0xFFFFFF78);

    // Flags: N=0, Z=0
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, D1
    // Opcode: 3 (Word) 001 (D1) 000 (DataReg) 000 (DataReg) 000 (D0) -> 0x3200
    write_op(&mut memory, &[0x3200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xFFFFFFFF;

    cpu.step_instruction(&mut memory);

    // Lower word of D1 should change to 0x5678
    assert_eq!(cpu.d[1], 0xFFFF5678);

    // Flags: N=0, Z=0 (0x5678 is positive)
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1
    // Opcode: 2 (Long) 001 (D1) 000 (DataReg) 000 (DataReg) 000 (D0) -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0xFFFFFFFF;

    cpu.step_instruction(&mut memory);

    // All of D1 should change
    assert_eq!(cpu.d[1], 0x12345678);

    // Flags: N=0, Z=0
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_flags_negative() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1 (0x1200)
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x80; // Negative byte

    cpu.step_instruction(&mut memory);

    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_flags_zero() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1 (0x1200)
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x00;

    cpu.step_instruction(&mut memory);

    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

// ============================================================================
// MOVE Tests (Memory to Register)
// ============================================================================

#[test]
fn test_move_immediate_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L #$12345678, D0
    // Opcode: 2 (Long) 000 (D0) 000 (DataReg) 111 (Mode 7) 100 (Immediate) -> 0x203C
    // Extension: 0x1234, 0x5678
    write_op(&mut memory, &[0x203C, 0x1234, 0x5678]);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0x12345678);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_absolute_short_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W $2000.W, D0
    // Opcode: 3 (Word) 000 (D0) 000 (DataReg) 111 (Mode 7) 000 (Abs Short) -> 0x3038
    // Extension: 0x2000
    write_op(&mut memory, &[0x3038, 0x2000]);

    // Address 0x2000 is sign extended to 0x00002000
    memory.write_word(0x2000, 0xABCD);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0] & 0xFFFF, 0xABCD);
    assert!(cpu.get_flag(flags::NEGATIVE)); // 0xABCD is negative as word
}

#[test]
fn test_move_indirect_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L (A0), D0
    // Opcode: 2 (Long) 000 (D0) 000 (DataReg) 010 (Indirect) 000 (A0) -> 0x2010
    write_op(&mut memory, &[0x2010]);

    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0xDEADBEEF);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0xDEADBEEF);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_postinc_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W (A0)+, D0
    // Opcode: 3 (Word) 000 (D0) 000 (DataReg) 011 (PostInc) 000 (A0) -> 0x3018
    write_op(&mut memory, &[0x3018]);

    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x1234);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
    assert_eq!(cpu.a[0], 0x2002); // Incremented by 2
}

#[test]
fn test_move_predec_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L -(A0), D0
    // Opcode: 2 (Long) 000 (D0) 000 (DataReg) 100 (PreDec) 000 (A0) -> 0x2020
    write_op(&mut memory, &[0x2020]);

    cpu.a[0] = 0x2004;
    memory.write_long(0x2000, 0xCAFEBABE);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0xCAFEBABE);
    assert_eq!(cpu.a[0], 0x2000); // Decremented by 4
}

// ============================================================================
// MOVE Tests (Register to Memory)
// ============================================================================

#[test]
fn test_move_d0_to_absolute_long() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, $00003000
    // Opcode: 1 (Byte) 111 (Mode 7) 001 (Abs Long) 000 (DataReg) 000 (D0) -> 0x13C0
    // Extension: 0x0000, 0x3000
    write_op(&mut memory, &[0x13C0, 0x0000, 0x3000]);

    cpu.d[0] = 0x55;

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x55);
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w_d0_a0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.W D0, A0
    // Opcode: 3 (Word) 001 (A0) 001 (AddrReg) 000 (DataReg) 000 (D0) -> 0x3040
    write_op(&mut memory, &[0x3040]);

    cpu.d[0] = 0xFFFF; // -1 as word
    cpu.a[0] = 0;

    // Set some flags to ensure they are NOT changed
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::NEGATIVE, false);

    cpu.step_instruction(&mut memory);

    // Should be sign extended
    assert_eq!(cpu.a[0], 0xFFFFFFFF);

    // Flags should NOT change for MOVEA
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_movea_l_d0_a0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.L D0, A0
    // Opcode: 2 (Long) 001 (A0) 001 (AddrReg) 000 (DataReg) 000 (D0) -> 0x2040
    write_op(&mut memory, &[0x2040]);

    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0;

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.a[0], 0x12345678);
}
