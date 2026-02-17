//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement instructions (MOVE, MOVEA, etc.).
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
    // Opcode: 00 (00=Byte) 001 (D1) 000 (Mode=D) 000 (D0) -> 0x1200
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x55;
    cpu.d[1] = 0x33;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x55);
    // 0x55 is non-zero, so Z should be 0.
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_b_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x1200]); // MOVE.B D0, D1
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x33;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_b_negative() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x1200]); // MOVE.B D0, D1
    cpu.d[0] = 0x80;
    cpu.d[1] = 0x00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x80);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, D1
    // Opcode: 00 (11=Word) 001 (D1) 000 (Mode=D) 000 (D0) -> 0x3200
    write_op(&mut memory, &[0x3200]);
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x4321;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFFFF, 0x1234);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1
    // Opcode: 00 (10=Long) 001 (D1) 000 (Mode=D) 000 (D0) -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x11111111;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x12345678);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_immediate_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L #$12345678, D0
    // Opcode: 00 (10=Long) 000 (D0) 000 (Mode=D) 111 (Mode=Imm) 100 (Reg=Imm) -> 0x203C
    // Wait, source is Immediate (#<data>) which is mode 111 reg 100.
    // Dst is D0 (mode 000 reg 000).
    // Opcode format: 00 ss ddd mmm mmm rrr
    // ss: 10 (Long)
    // ddd: 000 (D0)
    // mmm (dst mode): 000
    // mmm (src mode): 111
    // rrr (src reg): 100
    // Binary: 00 10 000 000 111 100 -> 0x203C
    write_op(&mut memory, &[0x203C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x12345678);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_move_d0_to_memory_absolute_long() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, (0x2000).L
    // Opcode: 00 (10) 001 (AbsLong=111 001) 000 000 -> 0x23C0
    // Dst mode: 111, Dst Reg: 001
    // Src mode: 000, Src Reg: 000 (D0)
    // Binary: 00 10 001 111 000 000 -> 0x23C0
    write_op(&mut memory, &[0x23C0, 0x0000, 0x2000]);
    cpu.d[0] = 0xCAFEBABE;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_long(0x2000), 0xCAFEBABE);
}

#[test]
fn test_move_memory_to_d0_indirect() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W (A0), D0
    // Opcode: 00 (11) 000 (D0) 000 010 (Indirect A0) 000 (A0) -> 0x3010
    // Src: Mode 010, Reg 000 (A0)
    // Dst: Mode 000, Reg 000 (D0)
    write_op(&mut memory, &[0x3010]);
    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x1234);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
}

#[test]
fn test_move_postinc() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B (A0)+, (A1)+
    // Opcode: 00 (01) 001 (A1) 011 (PostInc) 011 (PostInc) 000 (A0) -> 0x12D8
    // Dst: Mode 011 (PostInc), Reg 001 (A1)
    // Src: Mode 011 (PostInc), Reg 000 (A0)
    // Binary: 00 01 001 011 011 000 -> 0x12D8
    write_op(&mut memory, &[0x12D8]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x2000, 0x42);
    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x42);
    assert_eq!(cpu.a[0], 0x2001);
    assert_eq!(cpu.a[1], 0x3001);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_move_predec() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L -(A0), -(A1)
    // Opcode: 00 (10) 001 (A1) 100 (PreDec) 100 (PreDec) 000 (A0) -> 0x2320
    // Dst: Mode 100, Reg 001 (A1)
    // Src: Mode 100, Reg 000 (A0)
    write_op(&mut memory, &[0x2320]);
    cpu.a[0] = 0x2004;
    cpu.a[1] = 0x3004;
    memory.write_long(0x2000, 0xDEADBEEF);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_long(0x3000), 0xDEADBEEF);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.W #$FFFF, A0
    // Opcode: 00 (11) 000 (A0) 001 (Mode=A) 111 (Imm) 100 -> 0x307C
    // Dst: Mode 001 (Address Reg), Reg 000 (A0)
    write_op(&mut memory, &[0x307C, 0xFFFF]);
    cpu.step_instruction(&mut memory);

    // Sign extension check: 0xFFFF -> 0xFFFFFFFF
    assert_eq!(cpu.a[0], 0xFFFFFFFF);
    // Flags should NOT be affected by MOVEA
    assert!(!cpu.get_flag(flags::ZERO)); // Default initial is ? assume flags are 0 or random but step_instruction shouldn't change them if they were set.
    // Let's set flags and see if they change.
    cpu.set_flag(flags::ZERO, true);
    // Rerun instruction (need to reset PC and memory)
    cpu.pc = 0x1000;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_movea_l() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.L #$12345678, A0
    // Opcode: 00 (10) 000 (A0) 001 (Mode=A) 111 (Imm) 100 -> 0x207C
    write_op(&mut memory, &[0x207C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.a[0], 0x12345678);
}
