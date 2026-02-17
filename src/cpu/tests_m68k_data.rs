//! M68k Data Movement Tests
//!
//! Exhaustive tests for M68k data movement operations, specifically MOVEM.
//! Tests all sizes, addressing modes, and edge cases.

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
// MOVEM Tests
// ============================================================================

#[test]
fn test_movem_read_long_postinc() {
    // MOVEM.L (A0)+, D0/D1/A1
    // Opcode: 0100 1100 11 011 000 (Size=L, Dir=M->R, Mode=(A0)+) -> 0x4CD8
    // Mask: 0000 0010 0000 0011 (A1=bit 9, D1=bit 1, D0=bit 0) -> 0x0203
    // Note: Mask for M->R is standard: D0..D7, A0..A7
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4CD8, 0x0203]);

    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0x11111111); // D0
    memory.write_long(0x2004, 0x22222222); // D1
    memory.write_long(0x2008, 0x33333333); // A1

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0x11111111);
    assert_eq!(cpu.d[1], 0x22222222);
    assert_eq!(cpu.a[1], 0x33333333);
    assert_eq!(cpu.a[0], 0x200C); // Incremented by 12 bytes (3 regs * 4)
}

#[test]
fn test_movem_read_word_sign_extend() {
    // MOVEM.W (A0), D0
    // Opcode: 0100 1100 10 010 000 (Size=W, Dir=M->R, Mode=(A0)) -> 0x4C90
    // Mask: 0000 0000 0000 0001 (D0) -> 0x0001
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4C90, 0x0001]);

    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0xFFFF); // -1 as word

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0xFFFFFFFF); // Sign extended
    assert_eq!(cpu.a[0], 0x2000); // (A0) mode does not change A0
}

#[test]
fn test_movem_read_word_sign_extend_addr_reg() {
    // MOVEM.W (A0), A1
    // Opcode: 0100 1100 10 010 000 -> 0x4C90
    // Mask: 0000 0010 0000 0000 (A1) -> 0x0200
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4C90, 0x0200]);

    cpu.a[0] = 0x2000;
    memory.write_word(0x2000, 0x8000); // Negative word

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.a[1], 0xFFFF8000); // Sign extended
}

#[test]
fn test_movem_write_long_predec() {
    // MOVEM.L D0/A0, -(A7)
    // Opcode: 0100 1000 11 100 111 (Size=L, Dir=R->M, Mode=-(A7)) -> 0x48E7
    // Mask: Pre-decrement mask is reversed.
    // Order of transfer: A7..A0, D7..D0.
    // We want A0 and D0.
    // In standard mask: A0 is bit 8, D0 is bit 0.
    // In reversed mask:
    // Implementation iterates 15 down to 0.
    // Checks mask bit (15-i).
    // i=8 (A0). Check bit 7. So Bit 7 is A0.
    // i=0 (D0). Check bit 15. So Bit 15 is D0.
    // Mask = 0x8080 (Bit 15 and Bit 7 set).
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x48E7, 0x8080]);

    cpu.a[7] = 0x8000;
    cpu.d[0] = 0xDDDD0000;
    cpu.a[0] = 0xAAAA0000;

    cpu.step_instruction(&mut memory);

    // Expected behavior:
    // 1. A0 pushed. SP -> 0x7FFC. Mem[0x7FFC] = A0.
    // 2. D0 pushed. SP -> 0x7FF8. Mem[0x7FF8] = D0.
    // Final SP = 0x7FF8.

    assert_eq!(cpu.a[7], 0x7FF8);
    assert_eq!(memory.read_long(0x7FFC), 0xAAAA0000);
    assert_eq!(memory.read_long(0x7FF8), 0xDDDD0000);
}

#[test]
fn test_movem_write_word_control() {
    // MOVEM.W D0/D1, $2000 (Absolute Short)
    // Opcode: 0100 1000 10 111 000 (Size=W, Dir=R->M, Mode=Abs.W) -> 0x48B8
    // Mask: 0000 0000 0000 0011 (D0, D1) -> 0x0003
    // Extension: 0x2000
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x48B8, 0x0003, 0x2000]);

    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x9ABCDEF0;

    cpu.step_instruction(&mut memory);

    // Order: D0 then D1.
    // Addr 0x2000 <- D0.w
    // Addr 0x2002 <- D1.w
    assert_eq!(memory.read_word(0x2000), 0x5678);
    assert_eq!(memory.read_word(0x2002), 0xDEF0);
}

#[test]
fn test_movem_write_word_predec() {
    // MOVEM.W D0/A0, -(A7)
    // Opcode: 0100 1000 10 100 111 (Size=W, Dir=R->M, Mode=-(A7)) -> 0x48A7
    // Mask: 0x8080 (D0 and A0, reversed)
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x48A7, 0x8080]);

    cpu.a[7] = 0x8000;
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x9ABCDEF0;

    cpu.step_instruction(&mut memory);

    // Expected:
    // 1. A0 pushed (Word). SP -> 0x7FFE. Mem[0x7FFE] = A0.w
    // 2. D0 pushed (Word). SP -> 0x7FFC. Mem[0x7FFC] = D0.w

    assert_eq!(cpu.a[7], 0x7FFC);
    assert_eq!(memory.read_word(0x7FFE), 0xDEF0);
    assert_eq!(memory.read_word(0x7FFC), 0x5678);
}
