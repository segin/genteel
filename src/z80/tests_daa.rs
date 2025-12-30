//! Z80 DAA (Decimal Adjust Accumulator) Tests
//!
//! DAA is one of the most complex Z80 instructions to implement correctly.
//! It adjusts A to valid BCD after ADD or SUB operations.

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(m)
}

// ============ DAA after ADD (N=0) - no carries ============

#[test] fn daa_00_add() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.step(); assert_eq!(c.a, 0x00); }
#[test] fn daa_09_add() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.step(); assert_eq!(c.a, 0x09); }
#[test] fn daa_0a_add() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_0f_add() { let mut c = z80(&[0x27]); c.a = 0x0F; c.f = 0; c.step(); assert_eq!(c.a, 0x15); }
#[test] fn daa_10_add() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_19_add() { let mut c = z80(&[0x27]); c.a = 0x19; c.f = 0; c.step(); assert_eq!(c.a, 0x19); }
#[test] fn daa_1a_add() { let mut c = z80(&[0x27]); c.a = 0x1A; c.f = 0; c.step(); assert_eq!(c.a, 0x20); }
#[test] fn daa_90_add() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.step(); assert_eq!(c.a, 0x90); }
#[test] fn daa_99_add() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.step(); assert_eq!(c.a, 0x99); }
#[test] fn daa_9a_add() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_a0_add() { let mut c = z80(&[0x27]); c.a = 0xA0; c.f = 0; c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_ff_add() { let mut c = z80(&[0x27]); c.a = 0xFF; c.f = 0; c.step(); assert_eq!(c.a, 0x65); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with Half-Carry ============

#[test] fn daa_00_add_h() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x06); }
#[test] fn daa_09_add_h() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x0F); }
#[test] fn daa_0a_add_h() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_90_add_h() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x96); }
#[test] fn daa_9a_add_h() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with Carry ============

#[test] fn daa_00_add_c() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x60); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_09_add_c() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x69); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_0a_add_c() { let mut c = z80(&[0x27]); c.a = 0x0A; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x70); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_90_add_c() { let mut c = z80(&[0x27]); c.a = 0x90; c.f = 0; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xF0); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after ADD with H+C ============

#[test] fn daa_00_add_hc() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x66); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_99_add_hc() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xFF); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after SUB (N=1) ============

#[test] fn daa_00_sub() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x00); }
#[test] fn daa_09_sub() { let mut c = z80(&[0x27]); c.a = 0x09; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x09); }
#[test] fn daa_10_sub() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x10); }
#[test] fn daa_99_sub() { let mut c = z80(&[0x27]); c.a = 0x99; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.step(); assert_eq!(c.a, 0x99); }

// ============ DAA after SUB with Half-Carry ============

#[test] fn daa_00_sub_h() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0xFA); }
#[test] fn daa_10_sub_h() { let mut c = z80(&[0x27]); c.a = 0x10; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0x0A); }
#[test] fn daa_ff_sub_h() { let mut c = z80(&[0x27]); c.a = 0xFF; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.step(); assert_eq!(c.a, 0xF9); assert!(!c.get_flag(flags::CARRY)); }

// ============ DAA after SUB with Carry ============

#[test] fn daa_00_sub_c() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xA0); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_60_sub_c() { let mut c = z80(&[0x27]); c.a = 0x60; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA after SUB with H+C ============

#[test] fn daa_00_sub_hc() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x9A); assert!(c.get_flag(flags::CARRY)); }
#[test] fn daa_66_sub_hc() { let mut c = z80(&[0x27]); c.a = 0x66; c.f = 0; c.set_flag(flags::ADD_SUB, true); c.set_flag(flags::HALF_CARRY, true); c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0x00); assert!(c.get_flag(flags::CARRY)); }

// ============ DAA flag behavior ============

#[test] fn daa_zero_flag() { let mut c = z80(&[0x27]); c.a = 0x9A; c.f = 0; c.step(); assert!(c.get_flag(flags::ZERO)); }
#[test] fn daa_sign_flag() { let mut c = z80(&[0x27]); c.a = 0x80; c.f = 0; c.step(); assert!(c.get_flag(flags::SIGN)); }
#[test] fn daa_parity_flag() { let mut c = z80(&[0x27]); c.a = 0x00; c.f = 0; c.step(); assert!(c.get_flag(flags::PARITY)); } // 0 has even parity

// ============ Real BCD operations: 99 + 1 = 100 (carry) ============

#[test] fn daa_bcd_99_plus_1() {
    // Simulate 99 + 01 in BCD
    let mut c = z80(&[0x80, 0x27]); // ADD A, B; DAA
    c.a = 0x99;
    c.b = 0x01;
    c.step(); // ADD
    c.step(); // DAA
    assert_eq!(c.a, 0x00);
    assert!(c.get_flag(flags::CARRY)); // Overflow to 100
}

#[test] fn daa_bcd_45_plus_37() {
    // 45 + 37 = 82 in BCD
    let mut c = z80(&[0x80, 0x27]);
    c.a = 0x45;
    c.b = 0x37;
    c.step();
    c.step();
    assert_eq!(c.a, 0x82);
    assert!(!c.get_flag(flags::CARRY));
}

#[test] fn daa_bcd_50_minus_25() {
    // 50 - 25 = 25 in BCD
    let mut c = z80(&[0x90, 0x27]); // SUB B; DAA
    c.a = 0x50;
    c.b = 0x25;
    c.step();
    c.step();
    assert_eq!(c.a, 0x25);
}

#[test] fn daa_bcd_25_minus_50() {
    // 25 - 50 = -25, represented as 75 with borrow in BCD
    let mut c = z80(&[0x90, 0x27]);
    c.a = 0x25;
    c.b = 0x50;
    c.step();
    c.step();
    assert_eq!(c.a, 0x75);
    assert!(c.get_flag(flags::CARRY)); // Borrow
}

#[test] fn daa_pc() { let mut c = z80(&[0x27]); c.a = 0; c.step(); assert_eq!(c.pc, 1); }
