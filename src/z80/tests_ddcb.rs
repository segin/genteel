#![allow(unused_imports)]
//! Z80 DD CB / FD CB (Indexed Bit Operations) Tests
//!
//! Tests for indexed bit operations at (IX+d) and (IY+d).
//! Includes undocumented behavior where result is also stored to a register.

use super::*;
use crate::z80::test_utils::create_z80;

// ============ DD CB: Rotate/Shift at (IX+d) ============

#[test]
fn ddcb_rlc_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x06]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x80);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_rrc_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x0E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x01);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x80);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_rl_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x16]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x80);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_rr_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x1E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x01);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x80);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_sla_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x26]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x80);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x00);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_sra_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x2E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x81);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0xC0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn ddcb_sll_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x36]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
}
#[test]
fn ddcb_srl_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x3E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x81);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x40);
    assert!(c.get_flag(flags::CARRY));
}

// ============ DD CB: BIT at (IX+d) ============

#[test]
fn ddcb_bit_0_ix_set() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x46]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x01);
    c.step();
    assert!(!c.get_flag(flags::ZERO));
}
#[test]
fn ddcb_bit_0_ix_clear() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x46]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn ddcb_bit_7_ix_set() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x7E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x80);
    c.step();
    assert!(!c.get_flag(flags::ZERO));
}
#[test]
fn ddcb_bit_7_ix_clear() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x7E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x7F);
    c.step();
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn ddcb_bit_3_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x5E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x08);
    c.step();
    assert!(!c.get_flag(flags::ZERO));
}

// ============ DD CB: RES at (IX+d) ============

#[test]
fn ddcb_res_0_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x86]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0xFF);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0xFE);
}
#[test]
fn ddcb_res_7_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xBE]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0xFF);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x7F);
}
#[test]
fn ddcb_res_3_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x9E]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0xFF);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0xF7);
}
#[test]
fn ddcb_res_all() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x86]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x01);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x00);
}

// ============ DD CB: SET at (IX+d) ============

#[test]
fn ddcb_set_0_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xC6]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
}
#[test]
fn ddcb_set_7_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xFE]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x80);
}
#[test]
fn ddcb_set_3_ix() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xDE]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x08);
}
#[test]
fn ddcb_set_all() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xFE]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x7F);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0xFF);
}

// ============ Negative displacement ============

#[test]
fn ddcb_neg_d() {
    let mut c = create_z80(&[0xDD, 0xCB, 0xFB, 0x06]);
    c.ix = 0x1010;
    c.memory.write_byte(0x100B as u32, 0x80);
    c.step();
    assert_eq!(c.memory.read_byte(0x100B as u32), 0x01);
}
#[test]
fn ddcb_neg_d_set() {
    let mut c = create_z80(&[0xDD, 0xCB, 0xFB, 0xC6]);
    c.ix = 0x1010;
    c.memory.write_byte(0x100B as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x100B as u32), 0x01);
}

// ============ FD CB: Same operations with IY ============

#[test]
fn fdcb_rlc_iy() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0x06]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0x80);
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0x01);
}
#[test]
fn fdcb_rrc_iy() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0x0E]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0x01);
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0x80);
}
#[test]
fn fdcb_bit_7_iy() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0x7E]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0x80);
    c.step();
    assert!(!c.get_flag(flags::ZERO));
}
#[test]
fn fdcb_res_0_iy() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0x86]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0xFF);
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0xFE);
}
#[test]
fn fdcb_set_7_iy() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0xFE]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0x80);
}

// ============ Undocumented: Store result to register ============
// Using opcode bits other than 6 stores result to that register too

#[test]
fn ddcb_rlc_store_b() {
    // DD CB d 00 = RLC (IX+d), B
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x00]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x80);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
    assert_eq!(c.b, 0x01); // Also stored to B!
}

#[test]
fn ddcb_rlc_store_a() {
    // DD CB d 07 = RLC (IX+d), A
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x07]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x40);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x80);
    assert_eq!(c.a, 0x80);
}

#[test]
fn ddcb_set_store_c() {
    // DD CB d C1 = SET 0, (IX+d), C
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0xC1]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0x00);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x01);
    assert_eq!(c.c, 0x01);
}

#[test]
fn ddcb_res_store_d() {
    // DD CB d 82 = RES 0, (IX+d), D
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x82]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1005 as u32, 0xFF);
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0xFE);
    assert_eq!(c.d, 0xFE);
}

// ============ PC advancement ============

#[test]
fn ddcb_pc() {
    let mut c = create_z80(&[0xDD, 0xCB, 0x05, 0x06]);
    c.ix = 0x1000;
    c.step();
    assert_eq!(c.pc, 4);
}
#[test]
fn fdcb_pc() {
    let mut c = create_z80(&[0xFD, 0xCB, 0x05, 0x06]);
    c.iy = 0x2000;
    c.step();
    assert_eq!(c.pc, 4);
}
