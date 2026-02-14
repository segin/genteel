//! Extended M68k CPU Tests
//!
//! Comprehensive unit tests and property-based tests for M68k instructions.

#![cfg(test)]

use super::*;
use crate::memory::{Memory, MemoryInterface};
use proptest::prelude::*;

fn create_test_cpu() -> (Cpu, Memory) {
    let mut memory = Memory::new(0x10000);
    // Set up minimal vector table
    memory.write_long(0, 0x1000); // SSP
    memory.write_long(4, 0x100); // Reset vector PC
    let cpu = Cpu::new(&mut memory);
    (cpu, memory)
}

// ============ CHK Tests ============

#[test]
fn test_chk_in_bounds() {
    let (mut cpu, mut memory) = create_test_cpu();

    // CHK D1, D0 - opcode 4181
    memory.write_word(0x100, 0x4181);
    cpu.d[0] = 10; // Value to check
    cpu.d[1] = 50; // Upper bound

    cpu.step_instruction(&mut memory);

    // Should not trap - value in bounds [0..50]
    assert_eq!(cpu.pc, 0x102); // Normal execution continues
}

#[test]
fn test_chk_negative() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Set up CHK exception vector (vector 6 = address 0x18)
    memory.write_long(0x18, 0x4000);

    // CHK D1, D0
    memory.write_word(0x100, 0x4181);
    cpu.d[0] = 0xFFFF; // -1 as i16
    cpu.d[1] = 50;

    cpu.step_instruction(&mut memory);

    // Should trap to vector 6
    assert_eq!(cpu.pc, 0x4000);
    assert!(cpu.get_flag(flags::NEGATIVE)); // N set for negative
}

#[test]
fn test_chk_exceeds_bound() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Set up CHK exception vector
    memory.write_long(0x18, 0x5000);

    // CHK D1, D0
    memory.write_word(0x100, 0x4181);
    cpu.d[0] = 100; // Exceeds bound
    cpu.d[1] = 50; // Upper bound

    cpu.step_instruction(&mut memory);

    // Should trap
    assert_eq!(cpu.pc, 0x5000);
    assert!(!cpu.get_flag(flags::NEGATIVE)); // N clear for exceeds
}

// ============ TAS Tests ============

#[test]
fn test_tas_zero() {
    let (mut cpu, mut memory) = create_test_cpu();

    // TAS (A0) - opcode 4AD0
    memory.write_word(0x100, 0x4AD0);
    cpu.a[0] = 0x2000;
    memory.write_byte(0x2000, 0x00);

    cpu.step_instruction(&mut memory);

    // Flags from original value (0)
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));

    // High bit now set
    assert_eq!(memory.read_byte(0x2000), 0x80);
}

#[test]
fn test_tas_negative() {
    let (mut cpu, mut memory) = create_test_cpu();

    // TAS D0 - opcode 4AC0
    memory.write_word(0x100, 0x4AC0);
    cpu.d[0] = 0x80;

    cpu.step_instruction(&mut memory);

    // High bit already set
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert_eq!(cpu.d[0] & 0xFF, 0x80);
}

// ============ MOVEM Tests ============

#[test]
fn test_movem_to_memory() {
    let (mut cpu, mut memory) = create_test_cpu();

    // MOVEM.L D0-D1, (A0)
    // Opcode: 0100 1000 1101 0000 = 0x48D0 (MOVEM.L regs to (A0))
    memory.write_word(0x100, 0x48D0);
    memory.write_word(0x102, 0x0003); // D0, D1 = bits 0,1

    cpu.a[0] = 0x2000;
    cpu.d[0] = 0x11111111;
    cpu.d[1] = 0x22222222;

    cpu.step_instruction(&mut memory);

    // Check memory
    assert_eq!(memory.read_long(0x2000), 0x11111111);
    assert_eq!(memory.read_long(0x2004), 0x22222222);
}

#[test]
fn test_movem_from_memory() {
    let (mut cpu, mut memory) = create_test_cpu();

    // MOVEM.L (A0)+, D0-D1
    // Opcode: 4CD8, mask follows
    memory.write_word(0x100, 0x4CD8);
    memory.write_word(0x102, 0x0003); // D0, D1

    cpu.a[0] = 0x2000;
    memory.write_long(0x2000, 0xDEADBEEF);
    memory.write_long(0x2004, 0xCAFEBABE);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.d[0], 0xDEADBEEF);
    assert_eq!(cpu.d[1], 0xCAFEBABE);
    assert_eq!(cpu.a[0], 0x2008); // Post-incremented
}

// ============ PEA Tests ============

#[test]
fn test_pea() {
    let (mut cpu, mut memory) = create_test_cpu();

    // PEA (A0) - opcode 4850
    memory.write_word(0x100, 0x4850);
    cpu.a[0] = 0x12345678;
    cpu.a[7] = 0x3000;

    cpu.step_instruction(&mut memory);

    // Effective address pushed to stack
    assert_eq!(cpu.a[7], 0x2FFC);
    assert_eq!(memory.read_long(0x2FFC), 0x12345678);
}

// ============ SR Operation Tests ============

#[test]
fn test_move_to_sr() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.sr = 0x2700; // Supervisor mode

    // MOVE #$2300, SR - set new SR
    memory.write_word(0x100, 0x46FC);
    memory.write_word(0x102, 0x2300);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.sr, 0x2300);
}

#[test]
fn test_move_to_sr_privilege_violation() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.sr = 0x0000; // User mode

    // Set up privilege violation vector (vector 8)
    memory.write_long(0x20, 0x6000);

    // MOVE #$2300, SR
    memory.write_word(0x100, 0x46FC);
    memory.write_word(0x102, 0x2300);

    cpu.step_instruction(&mut memory);

    // Should trap
    assert_eq!(cpu.pc, 0x6000);
    assert!((cpu.sr & 0x2000) != 0); // Now in supervisor
}

#[test]
fn test_move_to_ccr() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.sr = 0x2700;

    // MOVE #$1F, CCR
    memory.write_word(0x100, 0x44FC);
    memory.write_word(0x102, 0x001F);

    cpu.step_instruction(&mut memory);

    // Only CCR bits set, supervisor bits unchanged
    assert_eq!(cpu.sr, 0x271F);
}

#[test]
fn test_andi_to_ccr() {
    let (mut cpu, mut memory) = create_test_cpu();
    cpu.sr = 0x271F; // All CCR bits set

    // ANDI #$10, CCR - Clear all except X
    memory.write_word(0x100, 0x023C);
    memory.write_word(0x102, 0x0010);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.sr & 0x00FF, 0x0010);
}

// ============ Property Tests ============

proptest! {
    #[test]
    fn prop_add_sub_inverse(a in 0u16..0xFFFF, b in 0u16..0xFFFF) {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = a as u32;
        cpu.d[1] = b as u32;

        // ADD.W D1, D0
        memory.write_word(0x100, 0xD041);
        cpu.step_instruction(&mut memory);

        let _sum = cpu.d[0] & 0xFFFF;

        // SUB.W D1, D0
        cpu.pc = 0x102;
        memory.write_word(0x102, 0x9041);
        cpu.step_instruction(&mut memory);

        // Should get back original
        prop_assert_eq!(cpu.d[0] & 0xFFFF, a as u32);
    }

    #[test]
    fn prop_neg_neg_identity(a in 0u16..0xFFFF) {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = a as u32;

        // NEG.W D0 twice should restore original
        memory.write_word(0x100, 0x4440);
        memory.write_word(0x102, 0x4440);

        cpu.step_instruction(&mut memory);
        cpu.step_instruction(&mut memory);

        prop_assert_eq!(cpu.d[0] & 0xFFFF, a as u32);
    }

    #[test]
    fn prop_not_not_identity(a in any::<u32>()) {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = a;

        // NOT.L D0 twice should restore original
        memory.write_word(0x100, 0x4680);
        memory.write_word(0x102, 0x4680);

        cpu.step_instruction(&mut memory);
        cpu.step_instruction(&mut memory);

        prop_assert_eq!(cpu.d[0], a);
    }

    #[test]
    fn prop_swap_swap_identity(a in any::<u32>()) {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = a;

        // SWAP D0 twice should restore original
        memory.write_word(0x100, 0x4840);
        memory.write_word(0x102, 0x4840);

        cpu.step_instruction(&mut memory);
        cpu.step_instruction(&mut memory);

        prop_assert_eq!(cpu.d[0], a);
    }

    #[test]
    fn prop_mul_preserves_low_word_with_one(a in 0u16..0xFFFF) {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.d[0] = a as u32;
        cpu.d[1] = 1;

        // MULU D1, D0 (multiply by 1)
        memory.write_word(0x100, 0xC0C1);
        cpu.step_instruction(&mut memory);

        prop_assert_eq!(cpu.d[0], a as u32);
    }
}

// ============ Extended Arithmetic Tests ============

#[test]
fn test_addx_extended() {
    let (mut cpu, mut memory) = create_test_cpu();

    // ADDX.L D1, D0
    // Opcode: 1101 0001 1000 0001 = D181
    memory.write_word(0x100, 0xD181);

    cpu.d[0] = 100;
    cpu.d[1] = 50;
    cpu.set_flag(flags::EXTEND, true); // X set

    cpu.step_instruction(&mut memory);

    // 100 + 50 + 1 = 151
    assert_eq!(cpu.d[0], 151);
    // X should be clear (no carry out)
    assert!(!cpu.get_flag(flags::EXTEND));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_subx_z_flag() {
    let (mut cpu, mut memory) = create_test_cpu();

    // SUBX.L D1, D0
    // Opcode: 1001 0001 1000 0001 = 9181
    memory.write_word(0x100, 0x9181);

    cpu.d[0] = 100;
    cpu.d[1] = 100;
    cpu.set_flag(flags::ZERO, true); // Z starts set
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // 100 - 100 - 0 = 0
    assert_eq!(cpu.d[0], 0);
    // Z should remain set (unchanged if result is 0)
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_negx() {
    let (mut cpu, mut memory) = create_test_cpu();

    // NEGX.B D0
    // Opcode: 0100 0000 0000 0000 = 4000
    memory.write_word(0x100, 0x4000);

    cpu.d[0] = 0x10;
    cpu.set_flag(flags::EXTEND, true);

    cpu.step_instruction(&mut memory);

    // 0 - 0x10 - 1 = -0x11 = 0xEF
    assert_eq!(cpu.d[0] & 0xFF, 0xEF);
    assert!(cpu.get_flag(flags::EXTEND)); // Borrow occurred
}

#[test]
fn test_cmpm() {
    let (mut cpu, mut memory) = create_test_cpu();

    // CMPM.B (A0)+, (A1)+
    // Opcode: 1011 0001 0000 1000 = B308
    memory.write_word(0x100, 0xB308);

    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x2000, 0x10);
    memory.write_byte(0x3000, 0x10);

    cpu.step_instruction(&mut memory);

    // 0x10 - 0x10 = 0
    assert!(cpu.get_flag(flags::ZERO));
    assert_eq!(cpu.a[0], 0x2001);
    assert_eq!(cpu.a[1], 0x3001);
}

#[test]
fn test_roxl() {
    let (mut cpu, mut memory) = create_test_cpu();

    // ROXL.B #1, D0
    // Opcode: 1110 0001 0001 0000 = E310
    memory.write_word(0x100, 0xE310);

    cpu.d[0] = 0x80; // 1000 0000
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    // Shift left, MSB goes to X/C, old X goes to LSB
    // 1000 0000 -> 0000 0000, X=1 (from MSB), C=1
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::CARRY));
}

#[test]
fn test_line_a() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Set up vector 10 (Line 1010 Emulator) = 0x28
    memory.write_long(0x28, 0x5000);

    // Line A instructions: 1010 ...
    memory.write_word(0x100, 0xA000);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x5000);
}

#[test]
fn test_line_f() {
    let (mut cpu, mut memory) = create_test_cpu();

    // Set up vector 11 (Line 1111 Emulator) = 0x2C
    memory.write_long(0x2C, 0x6000);

    // Line F instructions: 1111 ...
    memory.write_word(0x100, 0xF000);

    cpu.step_instruction(&mut memory);

    assert_eq!(cpu.pc, 0x6000);
}

#[test]
fn test_bcd_memory() {
    let (mut cpu, mut memory) = create_test_cpu();

    // ABCD -(A0), -(A1)
    // A0 = 0x2001, A1 = 0x3001
    // Mem[0x2000] = 0x45, Mem[0x3000] = 0x33
    // Result Mem[0x3000] = 0x78
    // Opcode: 1100 001 1 0000 1 000 = 0xC308
    memory.write_word(0x100, 0xC308);
    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x45);
    memory.write_byte(0x3000, 0x33);
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x78);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::EXTEND));

    // SBCD -(A0), -(A1)
    // A0 = 0x2001, A1 = 0x3001
    // Mem[0x2000] = 0x33, Mem[0x3000] = 0x78
    // Result Mem[0x3000] = 0x45
    // Opcode: 1000 001 1 0000 1 000 = 0x8308
    cpu.pc = 0x102;
    memory.write_word(0x102, 0x8308);
    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x33);
    memory.write_byte(0x3000, 0x78);
    cpu.set_flag(flags::ZERO, true);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x45);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);
    assert!(!cpu.get_flag(flags::ZERO));
}

#[test]
fn test_addx_subx_memory() {
    let (mut cpu, mut memory) = create_test_cpu();

    // ADDX.B -(A0), -(A1)
    // A0 = 0x2001, A1 = 0x3001
    // Mem[0x2000] = 0x10, Mem[0x3000] = 0x20
    // Result Mem[0x3000] = 0x30
    // Opcode: 1101 001 1 0000 1 000 = 0xD308
    memory.write_word(0x100, 0xD308);
    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x10);
    memory.write_byte(0x3000, 0x20);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x30);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);

    // SUBX.B -(A0), -(A1)
    // A0 = 0x2001, A1 = 0x3001
    // Mem[0x2000] = 0x10, Mem[0x3000] = 0x30
    // Result Mem[0x3000] = 0x20
    // Opcode: 1001 001 1 0000 1 000 = 0x9308
    cpu.pc = 0x102;
    memory.write_word(0x102, 0x9308);
    cpu.a[0] = 0x2001;
    cpu.a[1] = 0x3001;
    memory.write_byte(0x2000, 0x10);
    memory.write_byte(0x3000, 0x30);
    cpu.set_flag(flags::EXTEND, false);

    cpu.step_instruction(&mut memory);

    assert_eq!(memory.read_byte(0x3000), 0x20);
    assert_eq!(cpu.a[0], 0x2000);
    assert_eq!(cpu.a[1], 0x3000);
}
