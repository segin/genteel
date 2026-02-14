#![allow(unused_imports)]
//! Z80 Regression Test Suite
//!
//! Known edge cases and common emulator bugs.

use super::*; use crate::memory::{MemoryInterface, IoInterface};
use crate::memory::Memory;

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

// Bug: JR displacement is signed
#[test]
fn regression_jr_negative() {
    let mut c = z80(&[0x00, 0x00, 0x18, 0xFC]); // JR -4
    c.pc = 2;
    c.step();
    assert_eq!(c.pc, 0); // 2 + 2 (instruction) + (-4) = 0
}

// Bug: LD (HL), H/L uses new value after HL is modified
#[test]
fn regression_ld_hl_h() {
    let mut c = z80(&[0x74]); // LD (HL), H
    c.set_hl(0x1234);
    c.step();
    assert_eq!(c.memory.read_byte(0x1234 as u32), 0x12); // H value, not modified
}

// Bug: PUSH/POP AF not preserving all flag bits
#[test]
fn regression_push_pop_af_all_bits() {
    let mut c = z80(&[0xF5, 0xF1]); // PUSH AF; POP AF
    c.sp = 0x8000;
    c.a = 0xFF;
    c.f = 0xFF; // All flag bits set
    c.step(); // PUSH
    c.a = 0x00;
    c.f = 0x00;
    c.step(); // POP
    assert_eq!(c.a, 0xFF);
    assert_eq!(c.f, 0xFF);
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
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

#[test]
fn regression_ccf_copies_c_to_h() {
    let mut c = z80(&[0x3F]); // CCF
    c.set_flag(flags::CARRY, true);
    c.set_flag(flags::HALF_CARRY, false);
    c.step();
    assert!(!c.get_flag(flags::CARRY));
    assert!(c.get_flag(flags::HALF_CARRY)); // Previous C copied to H
}

// Bug: NEG with A=0x80 causes overflow
#[test]
fn regression_neg_80() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x80;
    c.step();
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::PARITY)); // Overflow
}

// Bug: NEG with A=0 clears carry
#[test]
fn regression_neg_00() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x00;
    c.step();
    assert_eq!(c.a, 0x00);
    assert!(!c.get_flag(flags::CARRY));
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

// Bug: BIT instruction H flag is always set
#[test]
fn regression_bit_h_flag() {
    let mut c = z80(&[0xCB, 0x47]); // BIT 0, A
    c.a = 0x00;
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
    assert!(c.get_flag(flags::PARITY)); // Even parity
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
