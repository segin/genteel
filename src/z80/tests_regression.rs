#![allow(unused_imports)]
//! Z80 Regression Test Suite
//!
//! Known edge cases and common emulator bugs.

use super::*;
use crate::memory::Memory;
use crate::memory::{IoInterface, MemoryInterface};

fn z80(program: &[u8]) -> Z80<Box<crate::memory::Memory>, Box<crate::z80::test_utils::TestIo>> {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    Z80::new(
        Box::new(m),
        Box::new(crate::z80::test_utils::TestIo::default()),
    )
}

// ============ Common emulator bugs ============

// Bug: DAA not handling N flag correctly
#[test]
fn regression_daa_after_sub() {
    let mut c = z80(&[0x90, 0x27]); // SUB B; DAA
    c.a = 0x50;
    c.b = 0x25;
    c.step(); // SUB
    c.step(); // DAA
    assert_eq!(c.a, 0x25);
}

// Bug: DJNZ not decrementing B before the test
#[test]
fn regression_djnz_decrements_first() {
    let mut c = z80(&[0x10, 0x05]);
    c.b = 1;
    c.step();
    assert_eq!(c.b, 0);
    assert_eq!(c.pc, 2); // Not taken
}

// Bug: DJNZ wrapping behavior (decrement then test)
#[test]
fn regression_djnz_wrap() {
    let mut c = z80(&[0x10, 0x05]);
    c.b = 0;
    c.step();
    assert_eq!(c.b, 0xFF);
    assert_eq!(c.pc, 7); // Taken (2 + 5)
}

// Bug: JR displacement is signed
#[test]
fn regression_jr_negative() {
    let mut c = z80(&[0x00, 0x00, 0x18, 0xFC]); // JR -4
    c.pc = 2;
    c.step();
    assert_eq!(c.pc, 0); // 2 + 2 (instruction) + (-4) = 0
}

#[test]
fn regression_jr_positive_overflow() {
    // Test JR at boundary where i16 addition might overflow
    // PC starts at 0x7FFD (32765)
    // 0x7FFD: 18 (JR)
    // 0x7FFE: 01 (+1)
    // After fetch JR: PC=0x7FFE
    // After fetch disp: PC=0x7FFF (32767)
    // Calculation: 32767 + 1 = 32768
    // If done in i16, 32767 + 1 overflows.
    let mut c = z80(&[]);
    c.memory.data[0x7FFD] = 0x18;
    c.memory.data[0x7FFE] = 0x01;
    c.pc = 0x7FFD;
    c.step();
    assert_eq!(c.pc, 0x8000);
}

// Bug: LD (HL), H/L uses new value after HL is modified
#[test]
fn regression_ld_hl_h() {
    let mut c = z80(&[0x74]); // LD (HL), H
    c.set_hl(0x1234);
    let t = c.step();
    assert_eq!(t, 7); // Timing check
    assert_eq!(c.memory.read_byte(0x1234 as u32), 0x12); // H value, not modified
}

#[test]
fn regression_ld_hl_l() {
    let mut c = z80(&[0x75]); // LD (HL), L
    c.set_hl(0x1234);
    let t = c.step();
    assert_eq!(t, 7); // Timing check
    assert_eq!(c.memory.read_byte(0x1234 as u32), 0x34); // L value
}

// Bug: PUSH/POP AF not preserving all flag bits
#[test]
fn regression_push_pop_af_all_bits() {
    let patterns = [0xFF, 0x00, 0x55, 0xAA];
    for &val in &patterns {
        let mut c = z80(&[0xF5, 0xF1]); // PUSH AF; POP AF
        c.sp = 0x8000;
        c.a = val;
        c.f = val;
        c.step(); // PUSH
        c.a = !val; // Corrupt registers to ensure reload works
        c.f = !val;
        c.step(); // POP
        assert_eq!(c.a, val, "Failed to preserve A for pattern 0x{:02X}", val);
        assert_eq!(c.f, val, "Failed to preserve F for pattern 0x{:02X}", val);
    }
}

// Bug: EX (SP), HL not swapping correctly
#[test]
fn regression_ex_sp_hl() {
    let mut c = z80(&[0xE3]);
    c.sp = 0x1000;
    c.set_hl(0x1234);
    c.memory.write_byte(0x1000 as u32, 0xCD);
    c.memory.write_byte(0x1001 as u32, 0xAB);
    c.step();
    assert_eq!(c.hl(), 0xABCD);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x1001 as u32), 0x12);
}

// Bug: INC/DEC not affecting V flag correctly
// Confirmed fixed: implementation correctly sets P/V flag on overflow.
#[test]
fn regression_inc_overflow() {
    let mut c = z80(&[0x3C]); // INC A
    c.a = 0x7F;
    c.step();
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::PARITY)); // Overflow
}

#[test]
fn regression_dec_overflow() {
    let mut c = z80(&[0x3D]); // DEC A
    c.a = 0x80;
    c.step();
    assert_eq!(c.a, 0x7F);
    assert!(c.get_flag(flags::PARITY)); // Overflow
}

// Bug: SCF/CCF H flag behavior
#[test]
fn regression_scf_clears_h() {
    let mut c = z80(&[0x37]); // SCF
    c.set_flag(flags::HALF_CARRY, true);
    c.set_flag(flags::ADD_SUB, true);
    c.set_flag(flags::SIGN, true);
    c.set_flag(flags::ZERO, true);
    c.set_flag(flags::PARITY, true);
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
    assert!(!c.get_flag(flags::ADD_SUB));
    assert!(c.get_flag(flags::CARRY));
    assert!(c.get_flag(flags::SIGN));
    assert!(c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn regression_ccf_copies_c_to_h() {
    let mut c = z80(&[0x3F]); // CCF

    // Case 1: C=1 -> H=1, C=0
    c.set_flag(flags::CARRY, true);
    c.set_flag(flags::HALF_CARRY, false);
    c.set_flag(flags::ADD_SUB, true);
    c.set_flag(flags::SIGN, true);
    c.set_flag(flags::ZERO, true);
    c.set_flag(flags::PARITY, true);
    c.step();
    assert!(!c.get_flag(flags::CARRY)); // Inverted C
    assert!(c.get_flag(flags::HALF_CARRY)); // Previous C copied to H
    assert!(!c.get_flag(flags::ADD_SUB)); // N cleared
    assert!(c.get_flag(flags::SIGN)); // Preserved
    assert!(c.get_flag(flags::ZERO)); // Preserved
    assert!(c.get_flag(flags::PARITY)); // Preserved

    // Reset PC for next step
    c.pc = 0;

    // Case 2: C=0 -> H=0, C=1
    c.set_flag(flags::CARRY, false);
    c.set_flag(flags::HALF_CARRY, true);
    c.set_flag(flags::ADD_SUB, true);
    c.set_flag(flags::SIGN, true);
    c.set_flag(flags::ZERO, true);
    c.set_flag(flags::PARITY, true);
    c.step();
    assert!(c.get_flag(flags::CARRY)); // Inverted C
    assert!(!c.get_flag(flags::HALF_CARRY)); // Previous C copied to H
    assert!(!c.get_flag(flags::ADD_SUB)); // N cleared
    assert!(c.get_flag(flags::SIGN)); // Preserved
    assert!(c.get_flag(flags::ZERO)); // Preserved
    assert!(c.get_flag(flags::PARITY)); // Preserved
}

// Bug: NEG with A=0x80 causes overflow
#[test]
fn regression_neg_80() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x80;
    c.step();
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::PARITY)); // Overflow
    assert!(c.get_flag(flags::CARRY)); // Carry should be set (A!=0)
}

// Regression: NEG with A=0 should clear carry
#[test]
fn regression_neg_00() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x00;
    c.step();
    assert_eq!(c.a, 0x00);
    assert!(!c.get_flag(flags::CARRY));
}

#[test]
fn regression_neg_normal() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
    assert!(!c.get_flag(flags::PARITY)); // No overflow
    assert!(c.get_flag(flags::CARRY)); // Carry set

    // Reuse c for another test
    c.reset();
    c.memory.write_byte(0, 0xED);
    c.memory.write_byte(1, 0x44);
    c.a = 0x7F;
    c.step();
    assert_eq!(c.a, 0x81);
    assert!(!c.get_flag(flags::PARITY)); // No overflow
    assert!(c.get_flag(flags::CARRY)); // Carry set
}

// Bug: LD A, I/R should set P/V from IFF2
#[test]
fn regression_ld_a_i_iff2() {
    let mut c = z80(&[0xED, 0x57]);
    c.i = 0x42;
    c.iff2 = true;
    c.step();
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn regression_ld_a_r_iff2() {
    let mut c = z80(&[0xED, 0x5F]);
    c.r = 0x42;
    c.iff2 = true;
    c.step();
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn regression_ld_a_i_iff2_false() {
    let mut c = z80(&[0xED, 0x57]);
    c.i = 0x42;
    c.iff2 = false;
    c.step();
    assert!(!c.get_flag(flags::PARITY));
}

#[test]
fn regression_ld_a_r_iff2_false() {
    let mut c = z80(&[0xED, 0x5F]);
    c.r = 0x42;
    c.iff2 = false;
    c.step();
    assert!(!c.get_flag(flags::PARITY));
}

// Bug: LDIR/LDDR BC=0 means 64K
#[test]
fn regression_ldir_bc_zero() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0000);
    c.memory.write_byte(0x1000 as u32, 0xAA);
    c.step();
    // BC was 0, now 0xFFFF
    assert_eq!(c.bc(), 0xFFFF);
    assert_eq!(c.memory.read_byte(0x2000 as u32), 0xAA);
    // HL and DE should be incremented
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.de(), 0x2001);
    // PC should loop back to instruction start (0x0000)
    assert_eq!(c.pc, 0x0000);
}

#[test]
fn regression_lddr_bc_zero() {
    let mut c = z80(&[0xED, 0xB8]); // LDDR
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0000);
    c.memory.write_byte(0x1000 as u32, 0xBB);
    c.step();
    // BC was 0, now 0xFFFF
    assert_eq!(c.bc(), 0xFFFF);
    assert_eq!(c.memory.read_byte(0x2000 as u32), 0xBB);
    // HL and DE should be decremented
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.de(), 0x1FFF);
    // PC should loop back to instruction start (0x0000)
    assert_eq!(c.pc, 0x0000);
}

// Bug: ADD HL, SP affects only C and H flags
#[test]
fn regression_add_hl_sp_flags() {
    let mut c = z80(&[0x39]);
    c.set_hl(0x1234);
    c.sp = 0x4321;
    c.set_flag(flags::ZERO, true);
    c.set_flag(flags::SIGN, true);
    c.set_flag(flags::PARITY, true);
    c.step();
    // S, Z, P/V should be preserved
    assert!(c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::SIGN));
    assert!(c.get_flag(flags::PARITY));
}

// Bug: BIT instruction H flag should always be set
#[test]
fn regression_bit_sets_h_flag() {
    // BIT 0, A
    let mut c = z80(&[0xCB, 0x47]);
    c.a = 0x00;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));

    // BIT 7, B
    let mut c = z80(&[0xCB, 0x78]);
    c.b = 0x00;
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));

    // BIT 0, (HL)
    let mut c = z80(&[0xCB, 0x46]);
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));

    // BIT 4, (IX+5) -> DD CB 05 66
    let mut c = z80(&[0xDD, 0xCB, 0x05, 0x66]);
    c.ix = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}

// Bug: RLC/RRC/RL/RR should affect all flags correctly
#[test]
fn regression_rlc_parity() {
    let mut c = z80(&[0xCB, 0x07]); // RLC A
    c.a = 0x00;
    c.step();
    assert!(c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY)); // Even parity (0x00 -> 0x00)
}

#[test]
fn regression_rrc_parity() {
    let mut c = z80(&[0xCB, 0x0F]); // RRC A
    c.a = 0x01;
    c.step();
    // 0x01 -> 0x80 (Carry=1)
    // 0x80 has odd parity? Wait, parity flag is set if EVEN number of bits set?
    // Z80 "Parity/Overflow" flag acts as Parity for logical ops.
    // Parity is set (1) if the number of set bits is even.
    // 0x80 (10000000) has 1 bit set -> Odd parity -> Flag cleared (0).
    assert!(!c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

#[test]
fn regression_rl_parity() {
    let mut c = z80(&[0xCB, 0x17]); // RL A
    c.a = 0x80;
    c.set_flag(flags::CARRY, false);
    c.step();
    // 0x80 << 1 | 0 = 0x00, Carry = 1
    // 0x00 has 0 bits set (even) -> Parity flag set (1)
    assert!(c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn regression_rr_parity() {
    let mut c = z80(&[0xCB, 0x1F]); // RR A
    c.a = 0x01;
    c.set_flag(flags::CARRY, true);
    c.step();
    // 0x01 >> 1 | 0x80 = 0x80, Carry = 1
    // 0x80 has 1 bit set (odd) -> Parity flag cleared (0)
    assert!(!c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

#[test]
fn regression_sla_parity() {
    let mut c = z80(&[0xCB, 0x27]); // SLA A
    c.a = 0xFF;
    c.step();
    // 0xFF << 1 = 0xFE, Carry = 1
    // 0xFE (11111110) has 7 bits set (odd) -> Parity flag cleared (0)
    assert!(!c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

#[test]
fn regression_sra_parity() {
    let mut c = z80(&[0xCB, 0x2F]); // SRA A
    c.a = 0x80;
    c.step();
    // 0x80 (10000000) >> 1 | 0x80 = 0xC0 (11000000)
    // 0xC0 has 2 bits set (even) -> Parity flag set (1)
    assert!(!c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn regression_srl_parity() {
    let mut c = z80(&[0xCB, 0x3F]); // SRL A
    c.a = 0x01;
    c.step();
    // 0x01 >> 1 = 0x00, Carry = 1
    // 0x00 has 0 bits set (even) -> Parity flag set (1)
    assert!(c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY));
}


// Bug: SBC HL, BC with no carry shouldn't borrow
#[test]
fn regression_sbc_hl_no_carry() {
    let mut c = z80(&[0xED, 0x42]);
    c.set_hl(0x1234);
    c.set_bc(0x0100);
    c.f = 0; // No carry
    c.step();
    assert_eq!(c.hl(), 0x1134);
}

// ============ Boundary conditions ============

#[test]
fn regression_sp_wrap_push() {
    let mut c = z80(&[0xC5]); // PUSH BC
    c.sp = 0x0001;
    c.set_bc(0x1234);
    c.step();
    assert_eq!(c.sp, 0xFFFF);
    assert_eq!(c.memory.read_byte(0xFFFF as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x0000 as u32), 0x12);
}

#[test]
fn regression_sp_wrap_pop() {
    let mut c = z80(&[0xC1]); // POP BC at addr 0
    c.sp = 0xFFFE; // Use 0xFFFE so we don't overwrite the instruction
    c.memory.write_byte(0xFFFE as u32, 0xCD);
    c.memory.write_byte(0xFFFF as u32, 0xAB);
    c.step();
    assert_eq!(c.bc(), 0xABCD);
    assert_eq!(c.sp, 0x0000);
}

#[test]
fn regression_pc_wrap() {
    let mut c = z80(&[0x00]); // NOP at 0xFFFF
    c.pc = 0xFFFF;
    c.memory.write_byte(0xFFFF as u32, 0x00);
    c.step();
    assert_eq!(c.pc, 0x0000);
}

// ============ Instruction interaction ============

#[test]
fn regression_ei_di_sequence() {
    // EI followed by DI - should DI take effect immediately?
    let mut c = z80(&[0xFB, 0xF3]); // EI; DI
    c.iff1 = false;
    c.iff2 = false;
    c.step(); // EI
    c.step(); // DI
    assert!(!c.iff1);
    assert!(!c.iff2);
}

#[test]
fn regression_halt_continues() {
    let mut c = z80(&[0x76]);
    c.step();
    assert!(c.halted);
    // HALT should stay at same PC
    let old_pc = c.pc;
    c.step();
    assert_eq!(c.pc, old_pc);
}
