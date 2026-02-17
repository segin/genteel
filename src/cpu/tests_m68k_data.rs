//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement instructions.
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
    // Opcode: 0001 (Size=Byte=01) 001 (Dst=D1) 000 (Mode=D0) 000 (Src=D0)
    // Actually:
    // Bits 15-14: 00
    // Bits 13-12: Size (01=byte, 11=word, 10=long)
    // Bits 11-9: Dst Reg
    // Bits 8-6: Dst Mode
    // Bits 5-3: Src Mode
    // Bits 2-0: Src Reg
    // MOVE.B D0, D1
    // 00 01 (Byte) 001 (D1) 000 (Mode Dn) 000 (Mode Dn) 000 (Src D0)
    // -> 0001 001 000 000 000 -> 0x1200
    write_op(&mut memory, &[0x1200]);
    cpu.d[0] = 0x55;
    cpu.d[1] = 0x33;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x55);
}

#[test]
fn test_move_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, D1
    // Size: 11 (Word)
    // 00 11 001 000 000 000 -> 0x3200
    write_op(&mut memory, &[0x3200]);
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x4321;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFFFF, 0x1234);
}

#[test]
fn test_move_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L D0, D1
    // Size: 10 (Long)
    // 00 10 001 000 000 000 -> 0x2200
    write_op(&mut memory, &[0x2200]);
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x11111111;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x12345678);
}

#[test]
fn test_move_flags() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B D0, D1
    write_op(&mut memory, &[0x1200]);

    // Case 1: Negative
    cpu.pc = 0x1000;
    cpu.d[0] = 0x80;
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::NEGATIVE, false);
    cpu.set_flag(flags::OVERFLOW, true); // Should be cleared
    cpu.set_flag(flags::CARRY, true); // Should be cleared
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));

    // Case 2: Zero
    cpu.pc = 0x1000;
    write_op(&mut memory, &[0x1200]); // Re-write op
    cpu.d[0] = 0x00;
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_move_immediate_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.L #$12345678, D0
    // Src Mode: 111 (0x7) Reg: 100 (0x4) -> Immediate
    // Dst Mode: 000 Reg: 000 -> D0
    // Size: 10 (Long)
    // 00 10 000 000 111 100 -> 0x203C
    write_op(&mut memory, &[0x203C, 0x1234, 0x5678]);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x12345678);
}

#[test]
fn test_move_d0_to_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.W D0, (A0)
    // Src Mode: 000 (Dn) Reg: 000 (D0)
    // Dst Mode: 010 (An Indirect) Reg: 000 (A0)
    // Size: 11 (Word)
    // 00 11 000 010 000 000 -> 0x3080
    write_op(&mut memory, &[0x3080]);
    cpu.d[0] = 0xABCD;
    cpu.a[0] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_word(0x2000), 0xABCD);
}

#[test]
fn test_move_memory_to_d0() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVE.B (A0), D0
    // Src Mode: 010 (An Indirect) Reg: 000 (A0)
    // Dst Mode: 000 (Dn) Reg: 000 (D0)
    // Size: 01 (Byte)
    // 00 01 000 000 010 000 -> 0x1010
    write_op(&mut memory, &[0x1010]);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x42);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x42);
}

// ============================================================================
// MOVEA Tests
// ============================================================================

#[test]
fn test_movea_w() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.W D0, A0
    // MOVE.W to Address Register is MOVEA
    // Src Mode: 000 (Dn) Reg: 000 (D0)
    // Dst Mode: 001 (An) Reg: 000 (A0)
    // Size: 11 (Word)
    // 00 11 000 001 000 000 -> 0x3040
    write_op(&mut memory, &[0x3040]);
    cpu.d[0] = 0xFFFF; // -1
    cpu.a[0] = 0x0000;
    cpu.set_flag(flags::ZERO, true); // Should not change
    cpu.step_instruction(&mut memory);

    // MOVEA sign extends word to long
    assert_eq!(cpu.a[0], 0xFFFFFFFF);
    // MOVEA does NOT affect flags
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_movea_l() {
    let (mut cpu, mut memory) = create_cpu();
    // MOVEA.L D0, A0
    // Size: 10 (Long)
    // 00 10 000 001 000 000 -> 0x2040
    write_op(&mut memory, &[0x2040]);
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x00000000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x12345678);
}
