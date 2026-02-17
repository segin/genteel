#![allow(unused_imports)]
//! Z80 Interrupt Handling Tests
//!
//! Tests for interrupt-related instructions and behavior:
//! - DI/EI and IFF1/IFF2
//! - IM 0/1/2 mode selection
//! - RETN/RETI behavior

use super::*;
use crate::memory::Memory;
use crate::memory::{IoInterface, MemoryInterface};

fn z80(program: &[u8]) -> crate::z80::test_utils::TestZ80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    let cpu = Z80::new();
    crate::z80::test_utils::TestZ80::new(cpu, m, crate::z80::test_utils::TestIo::default())
}

// ============ DI (Disable Interrupts) ============

#[test]
fn di_clears_iff1() {
    let mut c = z80(&[0xF3]);
    c.iff1 = true;
    c.step();
    assert!(!c.iff1);
}
#[test]
fn di_clears_iff2() {
    let mut c = z80(&[0xF3]);
    c.iff2 = true;
    c.step();
    assert!(!c.iff2);
}
#[test]
fn di_both() {
    let mut c = z80(&[0xF3]);
    c.iff1 = true;
    c.iff2 = true;
    c.step();
    assert!(!c.iff1);
    assert!(!c.iff2);
}
#[test]
fn di_already_clear() {
    let mut c = z80(&[0xF3]);
    c.iff1 = false;
    c.iff2 = false;
    c.step();
    assert!(!c.iff1);
    assert!(!c.iff2);
}
#[test]
fn di_pc() {
    let mut c = z80(&[0xF3]);
    c.step();
    assert_eq!(c.pc, 1);
}

// ============ EI (Enable Interrupts) ============

#[test]
fn ei_sets_iff1() {
    let mut c = z80(&[0xFB]);
    c.iff1 = false;
    c.step();
    assert!(c.iff1);
}
#[test]
fn ei_sets_iff2() {
    let mut c = z80(&[0xFB]);
    c.iff2 = false;
    c.step();
    assert!(c.iff2);
}
#[test]
fn ei_both() {
    let mut c = z80(&[0xFB]);
    c.iff1 = false;
    c.iff2 = false;
    c.step();
    assert!(c.iff1);
    assert!(c.iff2);
}
#[test]
fn ei_already_set() {
    let mut c = z80(&[0xFB]);
    c.iff1 = true;
    c.iff2 = true;
    c.step();
    assert!(c.iff1);
    assert!(c.iff2);
}
#[test]
fn ei_pc() {
    let mut c = z80(&[0xFB]);
    c.step();
    assert_eq!(c.pc, 1);
}

// ============ DI/EI sequence ============

#[test]
fn di_ei_sequence() {
    let mut c = z80(&[0xF3, 0xFB]); // DI; EI
    c.iff1 = true;
    c.iff2 = true;
    c.step(); // DI
    assert!(!c.iff1);
    assert!(!c.iff2);
    c.step(); // EI
    assert!(c.iff1);
    assert!(c.iff2);
}

#[test]
fn ei_di_sequence() {
    let mut c = z80(&[0xFB, 0xF3]); // EI; DI
    c.iff1 = false;
    c.iff2 = false;
    c.step(); // EI
    assert!(c.iff1);
    assert!(c.iff2);
    c.step(); // DI
    assert!(!c.iff1);
    assert!(!c.iff2);
}

// ============ IM (Interrupt Mode) ============

#[test]
fn im0_from_1() {
    let mut c = z80(&[0xED, 0x46]);
    c.im = 1;
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im0_from_2() {
    let mut c = z80(&[0xED, 0x46]);
    c.im = 2;
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im1_from_0() {
    let mut c = z80(&[0xED, 0x56]);
    c.im = 0;
    c.step();
    assert_eq!(c.im, 1);
}
#[test]
fn im1_from_2() {
    let mut c = z80(&[0xED, 0x56]);
    c.im = 2;
    c.step();
    assert_eq!(c.im, 1);
}
#[test]
fn im2_from_0() {
    let mut c = z80(&[0xED, 0x5E]);
    c.im = 0;
    c.step();
    assert_eq!(c.im, 2);
}
#[test]
fn im2_from_1() {
    let mut c = z80(&[0xED, 0x5E]);
    c.im = 1;
    c.step();
    assert_eq!(c.im, 2);
}
#[test]
fn im_pc() {
    let mut c = z80(&[0xED, 0x46]);
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ RETN (Return from NMI) ============

#[test]
fn retn_returns() {
    let mut c = z80(&[0xED, 0x45]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x34);
    c.memory.write_byte(0x1FFF as u32, 0x12);
    c.step();
    assert_eq!(c.pc, 0x1234);
    assert_eq!(c.sp, 0x2000);
}

#[test]
fn retn_restores_iff1_from_iff2_true() {
    let mut c = z80(&[0xED, 0x45]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x00);
    c.iff1 = false;
    c.iff2 = true;
    c.step();
    assert!(c.iff1);
    assert!(c.iff2);
}

#[test]
fn retn_restores_iff1_from_iff2_false() {
    let mut c = z80(&[0xED, 0x45]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x00);
    c.iff1 = true;
    c.iff2 = false;
    c.step();
    assert!(!c.iff1);
    assert!(!c.iff2);
}

// ============ RETI (Return from Interrupt) ============

#[test]
fn reti_returns() {
    let mut c = z80(&[0xED, 0x4D]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x78);
    c.memory.write_byte(0x1FFF as u32, 0x56);
    c.step();
    assert_eq!(c.pc, 0x5678);
    assert_eq!(c.sp, 0x2000);
}

#[test]
fn reti_does_not_modify_iff() {
    // RETI doesn't modify IFF flags (unlike RETN)
    let mut c = z80(&[0xED, 0x4D]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x00);
    c.iff1 = false;
    c.iff2 = true;
    c.step();
    // IFF flags should be unchanged
    assert!(!c.iff1);
    assert!(c.iff2);
}

// ============ LD A, I and LD A, R with IFF2 ============

#[test]
fn ld_a_i_sets_parity_from_iff2_true() {
    let mut c = z80(&[0xED, 0x57]);
    c.i = 0x42;
    c.iff2 = true;
    c.step();
    assert_eq!(c.a, 0x42);
    assert!(c.get_flag(flags::PARITY)); // P/V = IFF2
}

#[test]
fn ld_a_i_sets_parity_from_iff2_false() {
    let mut c = z80(&[0xED, 0x57]);
    c.i = 0x42;
    c.iff2 = false;
    c.step();
    assert_eq!(c.a, 0x42);
    assert!(!c.get_flag(flags::PARITY)); // P/V = IFF2
}

#[test]
fn ld_a_r_sets_parity_from_iff2_true() {
    let mut c = z80(&[0xED, 0x5F]);
    c.r = 0x00; // Will become 0x01 after fetch
    c.iff2 = true;
    c.step();
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn ld_a_r_sets_parity_from_iff2_false() {
    let mut c = z80(&[0xED, 0x5F]);
    c.r = 0x00;
    c.iff2 = false;
    c.step();
    assert!(!c.get_flag(flags::PARITY));
}

// ============ HALT behavior ============

#[test]
fn halt_sets_flag() {
    let mut c = z80(&[0x76]);
    assert!(!c.halted);
    c.step();
    assert!(c.halted);
}

#[test]
fn halt_stays_halted() {
    let mut c = z80(&[0x76]);
    c.step();
    assert!(c.halted);
    let cycles = c.step();
    assert!(c.halted);
    assert_eq!(cycles, 4); // HALT uses 4 T-states per NOP
}

#[test]
fn halt_pc_stops() {
    let mut c = z80(&[0x76]);
    c.step();
    assert_eq!(c.pc, 1);
    c.step();
    assert_eq!(c.pc, 1); // PC doesn't advance during HALT
}
