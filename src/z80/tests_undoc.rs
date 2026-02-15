#![allow(unused_imports)]
//! Z80 Undocumented Instruction Tests
//!
//! Tests for undocumented but commonly used Z80 instructions:
//! - SLL (Shift Left Logical with bit 0 set to 1)
//! - ED mirror opcodes (multiple encodings for same instruction)
//! - IXH/IXL/IYH/IYL access via DD/FD prefixes

use super::*;
use crate::memory::Memory;
use crate::memory::{IoInterface, MemoryInterface};

fn z80(program: &[u8]) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    Z80::new(m, crate::z80::test_utils::TestIo::default())
}

// ============ SLL (CB 30-37) - Undocumented shift ============
// SLL shifts left and sets bit 0 to 1 (not 0 like SLA)

#[test]
fn sll_b_00() {
    let mut c = z80(&[0xCB, 0x30]);
    c.b = 0x00;
    c.step();
    assert_eq!(c.b, 0x01);
    assert!(!c.get_flag(flags::CARRY));
}
#[test]
fn sll_b_01() {
    let mut c = z80(&[0xCB, 0x30]);
    c.b = 0x01;
    c.step();
    assert_eq!(c.b, 0x03);
}
#[test]
fn sll_b_80() {
    let mut c = z80(&[0xCB, 0x30]);
    c.b = 0x80;
    c.step();
    assert_eq!(c.b, 0x01);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn sll_b_ff() {
    let mut c = z80(&[0xCB, 0x30]);
    c.b = 0xFF;
    c.step();
    assert_eq!(c.b, 0xFF);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn sll_c() {
    let mut c = z80(&[0xCB, 0x31]);
    c.c = 0x55;
    c.step();
    assert_eq!(c.c, 0xAB);
}
#[test]
fn sll_d() {
    let mut c = z80(&[0xCB, 0x32]);
    c.d = 0xAA;
    c.step();
    assert_eq!(c.d, 0x55);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn sll_e() {
    let mut c = z80(&[0xCB, 0x33]);
    c.e = 0x40;
    c.step();
    assert_eq!(c.e, 0x81);
}
#[test]
fn sll_h() {
    let mut c = z80(&[0xCB, 0x34]);
    c.h = 0x00;
    c.step();
    assert_eq!(c.h, 0x01);
}
#[test]
fn sll_l() {
    let mut c = z80(&[0xCB, 0x35]);
    c.l = 0x7F;
    c.step();
    assert_eq!(c.l, 0xFF);
}
#[test]
fn sll_hl() {
    let mut c = z80(&[0xCB, 0x36]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x100 as u32), 0x01);
}
#[test]
fn sll_a() {
    let mut c = z80(&[0xCB, 0x37]);
    c.a = 0x42;
    c.step();
    assert_eq!(c.a, 0x85);
}

// ============ ED mirror opcodes ============
// NEG has multiple encodings: 44, 4C, 54, 5C, 64, 6C, 74, 7C

#[test]
fn neg_44() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_4c() {
    let mut c = z80(&[0xED, 0x4C]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_54() {
    let mut c = z80(&[0xED, 0x54]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_5c() {
    let mut c = z80(&[0xED, 0x5C]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_64() {
    let mut c = z80(&[0xED, 0x64]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_6c() {
    let mut c = z80(&[0xED, 0x6C]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_74() {
    let mut c = z80(&[0xED, 0x74]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn neg_7c() {
    let mut c = z80(&[0xED, 0x7C]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
}

// RETN mirrors: 45, 55, 65, 75
#[test]
fn retn_45() {
    let mut c = z80(&[0xED, 0x45]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x20);
    c.iff2 = true;
    c.step();
    assert_eq!(c.pc, 0x2000);
    assert!(c.iff1);
}
#[test]
fn retn_55() {
    let mut c = z80(&[0xED, 0x55]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x20);
    c.iff2 = true;
    c.step();
    assert_eq!(c.pc, 0x2000);
}
#[test]
fn retn_65() {
    let mut c = z80(&[0xED, 0x65]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x20);
    c.step();
    assert_eq!(c.pc, 0x2000);
}
#[test]
fn retn_75() {
    let mut c = z80(&[0xED, 0x75]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x20);
    c.step();
    assert_eq!(c.pc, 0x2000);
}

// RETI mirrors: 4D, 5D, 6D, 7D
#[test]
fn reti_4d() {
    let mut c = z80(&[0xED, 0x4D]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x30);
    c.step();
    assert_eq!(c.pc, 0x3000);
}
#[test]
fn reti_5d() {
    let mut c = z80(&[0xED, 0x5D]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x30);
    c.step();
    assert_eq!(c.pc, 0x3000);
}
#[test]
fn reti_6d() {
    let mut c = z80(&[0xED, 0x6D]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x30);
    c.step();
    assert_eq!(c.pc, 0x3000);
}
#[test]
fn reti_7d() {
    let mut c = z80(&[0xED, 0x7D]);
    c.sp = 0x100;
    c.memory.write_byte(0x100 as u32, 0x00);
    c.memory.write_byte(0x101 as u32, 0x30);
    c.step();
    assert_eq!(c.pc, 0x3000);
}

// IM mirrors
#[test]
fn im0_46() {
    let mut c = z80(&[0xED, 0x46]);
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im0_4e() {
    let mut c = z80(&[0xED, 0x4E]);
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im0_66() {
    let mut c = z80(&[0xED, 0x66]);
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im0_6e() {
    let mut c = z80(&[0xED, 0x6E]);
    c.step();
    assert_eq!(c.im, 0);
}
#[test]
fn im1_56() {
    let mut c = z80(&[0xED, 0x56]);
    c.step();
    assert_eq!(c.im, 1);
}
#[test]
fn im1_76() {
    let mut c = z80(&[0xED, 0x76]);
    c.step();
    assert_eq!(c.im, 1);
}
#[test]
fn im2_5e() {
    let mut c = z80(&[0xED, 0x5E]);
    c.step();
    assert_eq!(c.im, 2);
}
#[test]
fn im2_7e() {
    let mut c = z80(&[0xED, 0x7E]);
    c.step();
    assert_eq!(c.im, 2);
}

// ============ IN F, (C) - ED 70 ============
// Reads from port but only sets flags, discards value

#[test]
fn in_f_c() {
    let mut c = z80(&[0xED, 0x70]);
    c.set_bc(0x1234);
    c.step();
    // Should set flags based on input but not store to register
    assert_eq!(c.pc, 2);
}

// ============ OUT (C), 0 - ED 71 ============
// Outputs 0 to port

#[test]
fn out_c_0() {
    let mut c = z80(&[0xED, 0x71]);
    c.set_bc(0x1234);
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ Undocumented IXH/IXL Access (DD prefix) ============

#[test]
fn dd_ld_ixh_b() {
    let mut c = z80(&[0xDD, 0x60]);
    c.b = 0x55;
    c.step();
    assert_eq!(c.ixh(), 0x55);
    assert_eq!(c.ixl(), 0x00);
}
#[test]
fn dd_ld_a_ixl() {
    let mut c = z80(&[0xDD, 0x7D]);
    c.set_ixl(0xAA);
    c.step();
    assert_eq!(c.a, 0xAA);
}
#[test]
fn dd_add_a_ixh() {
    let mut c = z80(&[0xDD, 0x84]);
    c.a = 0x10;
    c.set_ixh(0x20);
    c.step();
    assert_eq!(c.a, 0x30);
}
#[test]
fn dd_inc_ixh() {
    let mut c = z80(&[0xDD, 0x24]);
    c.set_ixh(0x10);
    c.step();
    assert_eq!(c.ixh(), 0x11);
}
#[test]
fn dd_dec_ixl() {
    let mut c = z80(&[0xDD, 0x2D]);
    c.set_ixl(0x10);
    c.step();
    assert_eq!(c.ixl(), 0x0F);
}
#[test]
fn dd_ld_ixh_n() {
    let mut c = z80(&[0xDD, 0x26, 0x55]);
    c.step();
    assert_eq!(c.ixh(), 0x55);
}

// ============ Undocumented IYH/IYL Access (FD prefix) ============

#[test]
fn fd_ld_iyh_c() {
    let mut c = z80(&[0xFD, 0x61]);
    c.c = 0x55;
    c.step();
    assert_eq!(c.iyh(), 0x55);
}
#[test]
fn fd_sub_iyl() {
    let mut c = z80(&[0xFD, 0x95]);
    c.a = 0x10;
    c.set_iyl(0x05);
    c.step();
    assert_eq!(c.a, 0x0B);
}
