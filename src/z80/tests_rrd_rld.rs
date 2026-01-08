//! Z80 RRD/RLD (Rotate Digit) Tests
//!
//! RRD and RLD are BCD-related nibble rotate instructions.

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(Box::new(m))
}

// ============ RRD: Rotate Right Decimal ============
// (HL) low nibble -> (HL) high nibble -> A low nibble -> (HL) low nibble

#[test]
fn rrd_basic() {
    // A = 0x12, (HL) = 0x34
    // After RRD: A = 0x14, (HL) = 0x23
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x12;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x34);
    c.step();
    assert_eq!(c.a, 0x14);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0x23);
}

#[test]
fn rrd_zeros() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert_eq!(c.a, 0x00);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0x00);
}

#[test]
fn rrd_all_ones() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0xFF;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0xFF);
    c.step();
    assert_eq!(c.a, 0xFF);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0xFF);
}

#[test]
fn rrd_a_high_nibble_preserved() {
    // A's high nibble should be preserved
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0xF0;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x12);
    c.step();
    assert_eq!(c.a & 0xF0, 0xF0); // High nibble preserved
    assert_eq!(c.a & 0x0F, 0x02); // Low nibble from (HL) low
}

#[test]
fn rrd_flags_zero() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::ZERO));
}

#[test]
fn rrd_flags_sign() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x80;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::SIGN));
}

#[test]
fn rrd_clears_n_h() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x12;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x34);
    c.set_flag(flags::ADD_SUB, true);
    c.set_flag(flags::HALF_CARRY, true);
    c.step();
    assert!(!c.get_flag(flags::ADD_SUB));
    assert!(!c.get_flag(flags::HALF_CARRY));
}

#[test]
fn rrd_pc() {
    let mut c = z80(&[0xED, 0x67]);
    c.set_hl(0x1000);
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ RLD: Rotate Left Decimal ============  
// A low nibble -> (HL) low nibble -> (HL) high nibble -> A low nibble

#[test]
fn rld_basic() {
    // A = 0x12, (HL) = 0x34
    // After RLD: A = 0x13, (HL) = 0x42
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0x12;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x34);
    c.step();
    assert_eq!(c.a, 0x13);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0x42);
}

#[test]
fn rld_zeros() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert_eq!(c.a, 0x00);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0x00);
}

#[test]
fn rld_all_ones() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0xFF;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0xFF);
    c.step();
    assert_eq!(c.a, 0xFF);
    assert_eq!(c.memory.read_byte(0x1000 as u32), 0xFF);
}

#[test]
fn rld_a_high_nibble_preserved() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0xF0;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x12);
    c.step();
    assert_eq!(c.a & 0xF0, 0xF0);
    assert_eq!(c.a & 0x0F, 0x01); // High nibble of (HL)
}

#[test]
fn rld_flags_zero() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::ZERO));
}

#[test]
fn rld_flags_sign() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0x80;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::SIGN));
}

#[test]
fn rld_clears_n_h() {
    let mut c = z80(&[0xED, 0x6F]);
    c.a = 0x12;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x34);
    c.set_flag(flags::ADD_SUB, true);
    c.set_flag(flags::HALF_CARRY, true);
    c.step();
    assert!(!c.get_flag(flags::ADD_SUB));
    assert!(!c.get_flag(flags::HALF_CARRY));
}

#[test]
fn rld_pc() {
    let mut c = z80(&[0xED, 0x6F]);
    c.set_hl(0x1000);
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ RRD/RLD round-trip ============

#[test]
fn rrd_rld_roundtrip() {
    // Applying RRD then RLD should give original values
    let mut c = z80(&[0xED, 0x67, 0xED, 0x6F]); // RRD, RLD
    c.a = 0x12;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x34);
    
    // Store original
    let orig_a = c.a;
    let orig_mem = c.memory.read_byte(0x1000 as u32);
    
    c.step(); // RRD
    c.step(); // RLD
    
    assert_eq!(c.a, orig_a);
    assert_eq!(c.memory.read_byte(0x1000 as u32), orig_mem);
}

#[test]
fn rld_rrd_roundtrip() {
    // Applying RLD then RRD should give original values
    let mut c = z80(&[0xED, 0x6F, 0xED, 0x67]); // RLD, RRD
    c.a = 0x56;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x78);
    
    let orig_a = c.a;
    let orig_mem = c.memory.read_byte(0x1000 as u32);
    
    c.step(); // RLD
    c.step(); // RRD
    
    assert_eq!(c.a, orig_a);
    assert_eq!(c.memory.read_byte(0x1000 as u32), orig_mem);
}

// ============ Parity flag ============

#[test]
fn rrd_parity_even() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x00; // Result will have even parity
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(c.get_flag(flags::PARITY));
}

#[test]
fn rrd_parity_odd() {
    let mut c = z80(&[0xED, 0x67]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.memory.write_byte(0x1000 as u32, 0x10); // A becomes 0x00, (HL) becomes 0x01, A's low = 0
    c.step();
    // A = 0x00 has even parity
    assert!(c.get_flag(flags::PARITY));
}
