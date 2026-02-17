//! M68k BCD Instruction Tests
//!
//! Comprehensive tests for BCD operations (SBCD, ABCD, NBCD).
//! Focuses on SBCD as per task requirements.

#![cfg(test)]

use crate::cpu::flags;
use crate::cpu::Cpu;
use crate::memory::{Memory, MemoryInterface};

fn create_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x100000);
    let mut cpu = Cpu::new(&mut memory);
    cpu.pc = 0x1000;
    cpu.a[7] = 0x8000;
    cpu.sr = 0x2700; // Supervisor mode, interrupts disabled
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
// SBCD Tests (Subtract BCD with Extend)
// ============================================================================

// Helper to construct SBCD opcode
// mode: 0 = Data Register (Dy, Dx), 1 = Predecrement (-(Ay), -(Ax))
// rx: Destination Register (Dx or Ax)
// ry: Source Register (Dy or Ay)
fn sbcd_op(rx: u8, ry: u8, mode: u8) -> u16 {
    // 1000 Rx(3) 10000 m(1) Ry(3)
    0x8100 | ((rx as u16) << 9) | ((mode as u16 & 1) << 3) | (ry as u16)
}

#[test]
fn test_sbcd_reg_simple() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);

    cpu.d[0] = 0x25; // 25
    cpu.d[1] = 0x13; // 13
    cpu.set_flag(flags::EXTEND, false); // No borrow in
    cpu.set_flag(flags::ZERO, true);    // Z starts set (standard for BCD loops)

    cpu.step_instruction(&mut memory);

    // 25 - 13 = 12 (0x12)
    assert_eq!(cpu.d[0] & 0xFF, 0x12);
    assert!(!cpu.get_flag(flags::ZERO));   // Result non-zero, Z cleared
    assert!(!cpu.get_flag(flags::CARRY));  // No borrow
    assert!(!cpu.get_flag(flags::EXTEND)); // No borrow
}

#[test]
fn test_sbcd_reg_borrow_low() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);

    cpu.d[0] = 0x25;
    cpu.d[1] = 0x08;
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    // 25 - 08 = 17 (0x17)
    // Calc: 5 - 8 = -3 -> borrow from 20 -> 15 - 8 = 7.
    // 20 - 00 - 10 (borrow) = 10.
    // Result 17.
    assert_eq!(cpu.d[0] & 0xFF, 0x17);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_reg_borrow_high() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);

    cpu.d[0] = 0x15;
    cpu.d[1] = 0x20;
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    // 15 - 20 = 95 (borrow 100 + 15 - 20 = 95)
    // Calc: 5 - 0 = 5.
    // 10 - 20 = -10 -> borrow 100 -> 110 - 20 = 90.
    // Result 95.
    assert_eq!(cpu.d[0] & 0xFF, 0x95);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::CARRY));  // Borrow out
    assert!(cpu.get_flag(flags::EXTEND)); // Borrow out
}

#[test]
fn test_sbcd_reg_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);

    cpu.d[0] = 0x25;
    cpu.d[1] = 0x13;
    cpu.set_flag(flags::EXTEND, true); // Borrow in
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    // 25 - 13 - 1 = 11 (0x11)
    assert_eq!(cpu.d[0] & 0xFF, 0x11);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
    assert!(!cpu.get_flag(flags::EXTEND));
}

#[test]
fn test_sbcd_zero_flag_persistence() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD D1, D0
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);

    cpu.d[0] = 0x22;
    cpu.d[1] = 0x22;
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true); // Z starts set

    cpu.step_instruction(&mut memory);

    // 22 - 22 = 0
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO)); // Z remains set if result is 0
    assert!(!cpu.get_flag(flags::CARRY));

    // Test Z clearing
    cpu.pc = 0x1000; // Reset PC to rerun
    cpu.d[0] = 0x22;
    cpu.d[1] = 0x22;
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, false); // Z starts clear

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(!cpu.get_flag(flags::ZERO)); // Z remains clear even if result is 0 (unusual but correct for SBCD)
                                         // Wait, SBCD definition: "Z: Cleared if the result is non-zero. Unchanged otherwise."
                                         // So if result is 0, Z is unchanged. If it started clear, it stays clear.
}

#[test]
fn test_sbcd_memory_decrement() {
    let (mut cpu, mut memory) = create_cpu();
    // SBCD -(A1), -(A0)
    write_op(&mut memory, &[sbcd_op(0, 1, 1)]);

    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x34);
    memory.write_byte(0x3000, 0x12);
    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    cpu.step_instruction(&mut memory);

    // 34 - 12 = 22
    assert_eq!(memory.read_byte(0x2000), 0x22);
    assert_eq!(cpu.a[0], 0x2000); // Decremented
    assert_eq!(cpu.a[1], 0x3000); // Decremented
}

#[test]
fn test_sbcd_multi_byte_chain() {
    let (mut cpu, mut memory) = create_cpu();

    // Calculate 2000 - 0001 = 1999 using 2 SBCD instructions
    // Byte 0 (Low): 00 - 01
    // Byte 1 (High): 20 - 00

    // SBCD -(A1), -(A0)
    // SBCD -(A1), -(A0)
    let op = sbcd_op(0, 1, 1);
    write_op(&mut memory, &[op, op]);

    cpu.a[0] = 0x2002;
    cpu.a[1] = 0x3002;

    // Dest: 0x2000 (20 00)
    memory.write_byte(0x2000, 0x20);
    memory.write_byte(0x2001, 0x00);

    // Src: 0x0001 (00 01)
    memory.write_byte(0x3000, 0x00);
    memory.write_byte(0x3001, 0x01);

    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true);

    // First SBCD (Low byte)
    cpu.step_instruction(&mut memory);

    // 00 - 01 = 99 with borrow
    assert_eq!(memory.read_byte(0x2001), 0x99);
    assert!(cpu.get_flag(flags::EXTEND)); // Borrow generated
    assert!(!cpu.get_flag(flags::ZERO));  // Result non-zero

    // Second SBCD (High byte)
    cpu.step_instruction(&mut memory);

    // 20 - 00 - 1 (borrow) = 19
    assert_eq!(memory.read_byte(0x2000), 0x19);
    assert!(!cpu.get_flag(flags::EXTEND)); // No borrow out
    assert!(!cpu.get_flag(flags::ZERO));   // Result non-zero

    // Final result in memory: 19 99
}

#[test]
fn test_sbcd_zero_result_chain() {
    let (mut cpu, mut memory) = create_cpu();

    // Calculate 2233 - 2233 = 0000
    // Byte 0: 33 - 33 = 0
    // Byte 1: 22 - 22 = 0

    let op = sbcd_op(0, 1, 1);
    write_op(&mut memory, &[op, op]);

    cpu.a[0] = 0x2002;
    cpu.a[1] = 0x3002;

    memory.write_byte(0x2000, 0x22);
    memory.write_byte(0x2001, 0x33);

    memory.write_byte(0x3000, 0x22);
    memory.write_byte(0x3001, 0x33);

    cpu.set_flag(flags::EXTEND, false);
    cpu.set_flag(flags::ZERO, true); // Z starts set

    // First SBCD
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2001), 0x00);
    assert!(!cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::ZERO)); // Z remains set

    // Second SBCD
    cpu.step_instruction(&mut memory);
    assert_eq!(memory.read_byte(0x2000), 0x00);
    assert!(!cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::ZERO)); // Z remains set (entire 16-bit result is 0)
}

#[test]
fn test_sbcd_correction_boundary() {
    let (mut cpu, mut memory) = create_cpu();
    // Test boundary cases where BCD correction applies

    // 0x10 - 0x01 = 0x09
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x09);

    // 0x00 - 0x01 = 0x99 (Borrow)
    cpu.pc = 0x1000;
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x99);
    assert!(cpu.get_flag(flags::EXTEND));

    // 0x80 - 0x01 = 0x79
    cpu.pc = 0x1000;
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);
    cpu.d[0] = 0x80;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x79);
}

#[test]
fn test_sbcd_invalid_bcd_input() {
    // Behavior for invalid BCD is undefined in manuals, but usually behaves like binary sub + correction.
    // 0x0F - 0x01
    // Binary: 15 - 1 = 14 (0x0E).
    // Correction: No carry out of nibble?
    // Wait, typical algorithm:
    // Diff = 15 - 1 = 14.
    // If Diff > 9 ? No, logic is usually check carry/adjust.

    // Let's rely on the implementation logic we saw:
    // result = (0xF) - (0x1) = 0xE (14).
    // if result < 0 { sub 6 } -> False.
    // High nibble: (0 - 0) = 0.
    // result = 0x0E.

    // On real hardware 0x0F - 0x01 might be 0x0E or something else.
    // Our implementation does nibble math.

    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[sbcd_op(0, 1, 0)]);
    cpu.d[0] = 0x0F;
    cpu.d[1] = 0x01;
    cpu.set_flag(flags::EXTEND, false);
    cpu.step_instruction(&mut memory);

    // Based on current implementation:
    // result_low = 15 - 1 = 14. Not < 0. No adjustment.
    // result_high = 0 - 0 = 0. Not < 0. No adjustment.
    // Final: 0x0E.
    assert_eq!(cpu.d[0] & 0xFF, 0x0E);
}
