//! M68k ALU Tests
//!
//! Exhaustive tests for M68k arithmetic and logical unit operations.
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
// ADD Tests
// ============================================================================

#[test]
fn test_add_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD200]); // ADD.B D0, D1
    cpu.d[0] = 0x55;
    cpu.d[1] = 0x33;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x88);
}

#[test]
fn test_add_w_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD240]); // ADD.W D0, D1
    cpu.d[0] = 0x1234;
    cpu.d[1] = 0x4321;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFFFF, 0x5555);
}

#[test]
fn test_add_l_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD280]); // ADD.L D0, D1
    cpu.d[0] = 0x12345678;
    cpu.d[1] = 0x11111111;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0x23456789);
}

#[test]
fn test_add_carry_byte() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD200]); // ADD.B D0, D1
    cpu.d[0] = 0xFF;
    cpu.d[1] = 0x01;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_add_overflow_byte() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD200]); // ADD.B D0, D1
    cpu.d[0] = 0x7F;
    cpu.d[1] = 0x01; // 127 + 1 = overflow
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x80);
    assert!(cpu.get_flag(flags::OVERFLOW));
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_add_negative_byte() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD200]); // ADD.B D0, D1
    cpu.d[0] = 0x80;
    cpu.d[1] = 0x00;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_add_zero_byte() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD200]); // ADD.B D0, D1
    cpu.d[0] = 0x00;
    cpu.d[1] = 0x00;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
}

// ============================================================================
// ADDI Tests
// ============================================================================

#[test]
fn test_addi_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0600, 0x0042]); // ADDI.B #$42, D0
    cpu.d[0] = 0x10;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x52);
}

#[test]
fn test_addi_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0640, 0x1234]); // ADDI.W #$1234, D0
    cpu.d[0] = 0x1000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x2234);
}

#[test]
fn test_addi_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0680, 0x0001, 0x0000]); // ADDI.L #$10000, D0
    cpu.d[0] = 0x20000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0x30000);
}

// ============================================================================
// ADDQ Tests
// ============================================================================

#[test]
fn test_addq_1_to_8() {
    let (mut cpu, mut memory) = create_cpu();
    for data in 1..=8u8 {
        let opcode = 0x5040 | ((data as u16 % 8) << 9); // ADDQ.W #data, D0
        write_op(&mut memory, &[opcode]);
        cpu.invalidate_cache();
        cpu.d[0] = 100;
        cpu.pc = 0x1000;
        cpu.invalidate_cache();
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0] & 0xFFFF, 100 + data as u32, "ADDQ #{}", data);
    }
}

#[test]
fn test_addq_to_address_reg() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x5248]); // ADDQ.W #1, A0
    cpu.a[0] = 0x1000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x1001);
}

// ============================================================================
// ADDA Tests
// ============================================================================

#[test]
fn test_adda_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD0C0]); // ADDA.W D0, A0
    cpu.d[0] = 0x1000;
    cpu.a[0] = 0x2000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x3000);
}

#[test]
fn test_adda_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD1C0]); // ADDA.L D0, A0
    cpu.d[0] = 0x12345678;
    cpu.a[0] = 0x11111111;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x23456789);
}

#[test]
fn test_adda_sign_extend() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD0C0]); // ADDA.W D0, A0
    cpu.d[0] = 0xFFFF; // -1 as word, sign extends to 0xFFFFFFFF
    cpu.a[0] = 0x1000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x0FFF); // 0x1000 + (-1) = 0x0FFF
}

// ============================================================================
// ADDX Tests
// ============================================================================

#[test]
fn test_addx_register() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD101]); // ADDX.B D1, D0
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x20;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x31); // 0x10 + 0x20 + 1
}

#[test]
fn test_addx_carry_propagation() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xD101]); // ADDX.B D1, D0
    cpu.d[0] = 0xFF;
    cpu.d[1] = 0x00;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
}

// ============================================================================
// SUB Tests
// ============================================================================

#[test]
fn test_sub_b_d0_d1() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x9200]); // SUB.B D0, D1
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x55;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x45);
}

#[test]
fn test_sub_borrow_byte() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x9200]); // SUB.B D0, D1
    cpu.d[0] = 0x01;
    cpu.d[1] = 0x00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0xFF);
    assert!(cpu.get_flag(flags::CARRY));
    assert!(cpu.get_flag(flags::EXTEND));
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_sub_overflow() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x9200]); // SUB.B D0, D1
    cpu.d[0] = 0x01;
    cpu.d[1] = 0x80; // -128 - 1 = overflow
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x7F);
    assert!(cpu.get_flag(flags::OVERFLOW));
}

// ============================================================================
// SUBI Tests
// ============================================================================

#[test]
fn test_subi_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0400, 0x0010]); // SUBI.B #$10, D0
    cpu.d[0] = 0x50;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x40);
}

#[test]
fn test_subi_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0440, 0x1000]); // SUBI.W #$1000, D0
    cpu.d[0] = 0x5000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFFFF, 0x4000);
}

// ============================================================================
// SUBQ Tests
// ============================================================================

#[test]
fn test_subq_1_to_8() {
    let (mut cpu, mut memory) = create_cpu();
    for data in 1..=8u8 {
        let opcode = 0x5140 | ((data as u16 % 8) << 9); // SUBQ.W #data, D0
        write_op(&mut memory, &[opcode]);
        cpu.invalidate_cache();
        cpu.d[0] = 100;
        cpu.pc = 0x1000;
        cpu.invalidate_cache();
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0] & 0xFFFF, 100 - data as u32, "SUBQ #{}", data);
    }
}

#[test]
fn test_subq_to_address_reg() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x5348]); // SUBQ.W #1, A0
    cpu.a[0] = 0x1000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.a[0], 0x0FFF);
}

// ============================================================================
// SUBX Tests
// ============================================================================

#[test]
fn test_subx_register() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x9101]); // SUBX.B D1, D0
    cpu.d[0] = 0x30;
    cpu.d[1] = 0x10;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x1F); // 0x30 - 0x10 - 1
}

// ============================================================================
// NEG Tests
// ============================================================================

#[test]
fn test_neg_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4400]); // NEG.B D0
    cpu.d[0] = 0x01;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
    assert!(cpu.get_flag(flags::NEGATIVE));
}

#[test]
fn test_neg_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4400]); // NEG.B D0
    cpu.d[0] = 0x00;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_neg_0x80() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4400]); // NEG.B D0
    cpu.d[0] = 0x80;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x80); // -(-128) = -128 (overflow)
    assert!(cpu.get_flag(flags::OVERFLOW));
}

// ============================================================================
// NEGX Tests
// ============================================================================

#[test]
fn test_negx_with_extend() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4000]); // NEGX.B D0
    cpu.d[0] = 0x00;
    cpu.set_flag(flags::EXTEND, true);
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF); // 0 - 0 - 1 = -1
}

// ============================================================================
// CMP Tests
// ============================================================================

#[test]
fn test_cmp_equal() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xB200]); // CMP.B D0, D1
    cpu.d[0] = 0x42;
    cpu.d[1] = 0x42;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_cmp_greater() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xB200]); // CMP.B D0, D1
    cpu.d[0] = 0x10;
    cpu.d[1] = 0x42;
    cpu.step_instruction(&mut memory);
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::NEGATIVE)); // 0x42 - 0x10 = positive
}

#[test]
fn test_cmp_less() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xB200]); // CMP.B D0, D1
    cpu.d[0] = 0x50;
    cpu.d[1] = 0x10;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::CARRY)); // Borrow occurred
    assert!(cpu.get_flag(flags::NEGATIVE));
}

// ============================================================================
// CMPI Tests
// ============================================================================

#[test]
fn test_cmpi_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0C00, 0x0042]); // CMPI.B #$42, D0
    cpu.d[0] = 0x42;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_cmpi_w() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0C40, 0x1234]); // CMPI.W #$1234, D0
    cpu.d[0] = 0x1234;
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
}

// ============================================================================
// CMPM Tests
// ============================================================================

#[test]

fn test_cmpm_b() {
    let (mut cpu, mut memory) = create_cpu();
    // CMPM.B (A1)+, (A0)+
    // Opcode: 1011 (B) 000 (Rx=A0) 1 00 (size) 001 (mode) 001 (Ry=A1) -> 0xB109
    write_op(&mut memory, &[0xB109]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x2000, 0x42);
    memory.write_byte(0x3000, 0x42);
    cpu.step_instruction(&mut memory);
    assert!(cpu.get_flag(flags::ZERO));
    assert_eq!(cpu.a[0], 0x2001);
    assert_eq!(cpu.a[1], 0x3001);
}

// ============================================================================
// AND Tests
// ============================================================================

#[test]
fn test_and_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC200]); // AND.B D0, D1
    cpu.d[0] = 0xF0;
    cpu.d[1] = 0x0F;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_and_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC280]); // AND.L D0, D1
    cpu.d[0] = 0xFF00FF00;
    cpu.d[1] = 0xFFFFFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0xFF00FF00);
}

// ============================================================================
// ANDI Tests
// ============================================================================

#[test]
fn test_andi_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0200, 0x000F]); // ANDI.B #$0F, D0
    cpu.d[0] = 0xFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x0F);
}

// ============================================================================
// OR Tests
// ============================================================================

#[test]
fn test_or_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x8200]); // OR.B D0, D1
    cpu.d[0] = 0xF0;
    cpu.d[1] = 0x0F;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1] & 0xFF, 0xFF);
}

#[test]
fn test_or_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x8280]); // OR.L D0, D1
    cpu.d[0] = 0xFF00FF00;
    cpu.d[1] = 0x00FF00FF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[1], 0xFFFFFFFF);
}

// ============================================================================
// ORI Tests
// ============================================================================

#[test]
fn test_ori_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0000, 0x00F0]); // ORI.B #$F0, D0
    cpu.d[0] = 0x0F;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xFF);
}

// ============================================================================
// EOR Tests
// ============================================================================

#[test]
fn test_eor_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xB300]); // EOR.B D1, D0
    cpu.d[0] = 0xFF;
    cpu.d[1] = 0xFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x00);
    assert!(cpu.get_flag(flags::ZERO));
}

#[test]
fn test_eor_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xB380]); // EOR.L D1, D0
    cpu.d[0] = 0xAAAAAAAA;
    cpu.d[1] = 0x55555555;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFFFFFF);
}

// ============================================================================
// EORI Tests
// ============================================================================

#[test]
fn test_eori_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x0A00, 0x00FF]); // EORI.B #$FF, D0
    cpu.d[0] = 0xAA;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0x55);
}

// ============================================================================
// NOT Tests
// ============================================================================

#[test]
fn test_not_b() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4600]); // NOT.B D0
    cpu.d[0] = 0x55;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] & 0xFF, 0xAA);
}

#[test]
fn test_not_l() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x4680]); // NOT.L D0
    cpu.d[0] = 0x00000000;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFFFFFF);
}

// ============================================================================
// MUL Tests
// ============================================================================

#[test]
fn test_mulu_basic() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC0C1]); // MULU.W D1, D0
    cpu.d[0] = 100;
    cpu.d[1] = 200;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 20000);
}

#[test]
fn test_mulu_max() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC0C1]); // MULU.W D1, D0
    cpu.d[0] = 0xFFFF;
    cpu.d[1] = 0xFFFF;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 0xFFFE0001);
}

#[test]
fn test_muls_pos_pos() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC1C1]); // MULS.W D1, D0
    cpu.d[0] = 100;
    cpu.d[1] = 200;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0], 20000);
}

#[test]
fn test_muls_neg_pos() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC1C1]); // MULS.W D1, D0
    cpu.d[0] = 0xFFFE; // -2
    cpu.d[1] = 100;
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] as i32, -200);
}

#[test]
fn test_muls_neg_neg() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0xC1C1]); // MULS.W D1, D0
    cpu.d[0] = 0xFFFE; // -2
    cpu.d[1] = 0xFFF6; // -10
    cpu.step_instruction(&mut memory);
    assert_eq!(cpu.d[0] as i32, 20);
}

// ============================================================================
// DIV Tests
// ============================================================================

#[test]
fn test_divu_basic() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x80C1]); // DIVU.W D1, D0
    cpu.d[0] = 20000;
    cpu.d[1] = 100;
    cpu.step_instruction(&mut memory);
    let quotient = cpu.d[0] & 0xFFFF;
    let remainder = (cpu.d[0] >> 16) & 0xFFFF;
    assert_eq!(quotient, 200);
    assert_eq!(remainder, 0);
}

#[test]
fn test_divu_remainder() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x80C1]); // DIVU.W D1, D0
    cpu.d[0] = 12345;
    cpu.d[1] = 100;
    cpu.step_instruction(&mut memory);
    let quotient = cpu.d[0] & 0xFFFF;
    let remainder = (cpu.d[0] >> 16) & 0xFFFF;
    assert_eq!(quotient, 123);
    assert_eq!(remainder, 45);
}

#[test]
fn test_divs_neg_pos() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = (-200i32) as u32;
    cpu.d[1] = 10;
    cpu.step_instruction(&mut memory);
    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    assert_eq!(quotient, -20);
}

#[test]
fn test_divu_by_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x80C1]); // DIVU.W D1, D0
    cpu.d[0] = 100;
    cpu.d[1] = 0;
    // Set up divide by zero vector
    memory.write_long(0x14, 0x2000);
    cpu.step_instruction(&mut memory);
    // Should trap
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_divs_basic() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
                                      // -10 / 3
                                      // Quotient: -3 (0xFFFD)
                                      // Remainder: -1 (0xFFFF)
    cpu.d[0] = (-10i32) as u32;
    cpu.d[1] = 3;
    cpu.step_instruction(&mut memory);

    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    let remainder = ((cpu.d[0] >> 16) & 0xFFFF) as i16;

    assert_eq!(quotient, -3);
    assert_eq!(remainder, -1);

    // Flags for quotient -3: N=1, Z=0
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_divs_by_zero() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = 100;
    cpu.d[1] = 0;
    // Set up divide by zero vector
    memory.write_long(0x14, 0x2000);
    cpu.step_instruction(&mut memory);
    // Should trap
    assert_eq!(cpu.pc, 0x2000);
}

#[test]
fn test_divs_overflow() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
                                      // 0x80000000 (-2147483648) / -1 = 2147483648
                                      // 2147483648 cannot fit in 16-bit signed quotient (max 32767)
    cpu.d[0] = 0x80000000;
    cpu.d[1] = 0xFFFFFFFF; // -1
    cpu.step_instruction(&mut memory);

    // Check overflow flag
    assert!(cpu.get_flag(flags::OVERFLOW));

    // Destination register should be unchanged
    assert_eq!(cpu.d[0], 0x80000000);
}

#[test]
fn test_divs_pos_neg() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = 100;
    cpu.d[1] = (-10i16) as u16 as u32; // -10
    cpu.step_instruction(&mut memory);

    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    let remainder = (cpu.d[0] >> 16) as i16;

    assert_eq!(quotient, -10);
    assert_eq!(remainder, 0);
    assert!(cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_divs_neg_neg() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = (-100i32) as u32;
    cpu.d[1] = (-10i16) as u16 as u32; // -10
    cpu.step_instruction(&mut memory);

    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    let remainder = (cpu.d[0] >> 16) as i16;

    assert_eq!(quotient, 10);
    assert_eq!(remainder, 0);
    assert!(!cpu.get_flag(flags::NEGATIVE));
    assert!(!cpu.get_flag(flags::ZERO));
    assert!(!cpu.get_flag(flags::OVERFLOW));
    assert!(!cpu.get_flag(flags::CARRY));
}

#[test]
fn test_divs_remainder() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = 105;
    cpu.d[1] = 10;
    cpu.step_instruction(&mut memory);

    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    let remainder = (cpu.d[0] >> 16) as i16;

    assert_eq!(quotient, 10);
    assert_eq!(remainder, 5);
}

#[test]
fn test_divs_neg_remainder() {
    let (mut cpu, mut memory) = create_cpu();
    write_op(&mut memory, &[0x81C1]); // DIVS.W D1, D0
    cpu.d[0] = (-105i32) as u32;
    cpu.d[1] = 10;
    cpu.step_instruction(&mut memory);

    let quotient = (cpu.d[0] & 0xFFFF) as i16;
    let remainder = (cpu.d[0] >> 16) as i16;

    assert_eq!(quotient, -10);
    assert_eq!(remainder, -5);
}

#[test]
fn test_addx_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // ADDX.B -(A1), -(A0)
    // Opcode: 1101 (ADDX prefix) 000 (Rx=A0) 1 00 (Size=B) 001 (Mode=Memory) 001 (Ry=A1) -> 0xD109
    write_op(&mut memory, &[0xD109]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x1FFF, 0x10);
    memory.write_byte(0x2FFF, 0x20);
    cpu.set_flag(flags::EXTEND, true);

    cpu.step_instruction(&mut memory);

    // Result should be written to 0x1FFF (pre-decremented A0)
    // 0x10 + 0x20 + 1 = 0x31
    assert_eq!(memory.read_byte(0x1FFF), 0x31);
    assert_eq!(cpu.a[0], 0x1FFF);
    assert_eq!(cpu.a[1], 0x2FFF);
}

#[test]
fn test_subx_memory() {
    let (mut cpu, mut memory) = create_cpu();
    // SUBX.B -(A1), -(A0)
    // Opcode: 1001 (SUBX prefix) 000 (Rx=A0) 1 00 (Size=B) 001 (Mode=Memory) 001 (Ry=A1) -> 0x9109
    write_op(&mut memory, &[0x9109]);
    cpu.a[0] = 0x2000;
    cpu.a[1] = 0x3000;
    memory.write_byte(0x1FFF, 0x30); // Dest
    memory.write_byte(0x2FFF, 0x10); // Src
    cpu.set_flag(flags::EXTEND, true);

    cpu.step_instruction(&mut memory);

    // Result: 0x30 - 0x10 - 1 = 0x1F
    assert_eq!(memory.read_byte(0x1FFF), 0x1F);
    assert_eq!(cpu.a[0], 0x1FFF);
    assert_eq!(cpu.a[1], 0x2FFF);
}
