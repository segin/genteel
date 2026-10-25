//! Unit tests for Z80 CPU - ED Prefix Block Instructions
//! Tests for LDIR, LDDR, CPIR, CPDR, INIR, INDR, OTIR, OTDR

use super::*;
use crate::z80::test_utils::create_z80;

// ============ LDIR ============
#[test]
fn test_ldir_basic() {
    let mut c = create_z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0002);
    c.memory.write_byte(0x1000, 0xAA);
    c.memory.write_byte(0x1001, 0xBB);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0); // PC jumps back to repeat
    assert_eq!(c.memptr, 1); // PC + 1 (0 + 1)
    assert_eq!(c.memory.read_byte(0x2000), 0xAA);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.de(), 0x2001);
    assert_eq!(c.bc(), 0x0001);
    assert!(!c.get_flag(flags::HALF_CARRY));
    assert!(!c.get_flag(flags::ADD_SUB));
    assert!(c.get_flag(flags::PARITY)); // BC != 0

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2); // PC moves past instruction
    assert_eq!(c.memory.read_byte(0x2001), 0xBB);
    assert_eq!(c.hl(), 0x1002);
    assert_eq!(c.de(), 0x2002);
    assert_eq!(c.bc(), 0x0000);
    assert!(!c.get_flag(flags::PARITY)); // BC == 0
}

// ============ LDDR ============
#[test]
fn test_lddr_basic() {
    let mut c = create_z80(&[0xED, 0xB8]);
    c.set_hl(0x1001);
    c.set_de(0x2001);
    c.set_bc(0x0002);
    c.memory.write_byte(0x1001, 0xAA);
    c.memory.write_byte(0x1000, 0xBB);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.memptr, 1);
    assert_eq!(c.memory.read_byte(0x2001), 0xAA);
    assert_eq!(c.hl(), 0x1000);
    assert_eq!(c.de(), 0x2000);
    assert_eq!(c.bc(), 0x0001);
    assert!(c.get_flag(flags::PARITY));

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.memory.read_byte(0x2000), 0xBB);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.de(), 0x1FFF);
    assert_eq!(c.bc(), 0x0000);
    assert!(!c.get_flag(flags::PARITY));
}

// ============ CPIR ============
#[test]
fn test_cpir_match() {
    let mut c = create_z80(&[0xED, 0xB1]);
    c.a = 0x55;
    c.set_hl(0x1000);
    c.set_bc(0x0003);
    c.memory.write_byte(0x1000, 0x00);
    c.memory.write_byte(0x1001, 0x55);
    c.memory.write_byte(0x1002, 0x00);

    // First step (no match)
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.memptr, 1);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.bc(), 0x0002);
    assert!(!c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::PARITY));
    assert!(c.get_flag(flags::ADD_SUB));

    // Second step (match)
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    // memptr increments by 1
    assert_eq!(c.memptr, 2);
    assert_eq!(c.hl(), 0x1002);
    assert_eq!(c.bc(), 0x0001);
    assert!(c.get_flag(flags::ZERO)); // Match found!
    assert!(c.get_flag(flags::PARITY)); // BC != 0
}

#[test]
fn test_cpir_no_match() {
    let mut c = create_z80(&[0xED, 0xB1]);
    c.a = 0x55;
    c.set_hl(0x1000);
    c.set_bc(0x0002);
    c.memory.write_byte(0x1000, 0x00);
    c.memory.write_byte(0x1001, 0x11);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.bc(), 0);
    assert!(!c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

// ============ CPDR ============
#[test]
fn test_cpdr_match() {
    let mut c = create_z80(&[0xED, 0xB9]);
    c.a = 0x77;
    c.set_hl(0x1001);
    c.set_bc(0x0002);
    c.memory.write_byte(0x1001, 0x00);
    c.memory.write_byte(0x1000, 0x77);

    // First step (no match)
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.memptr, 1);
    assert_eq!(c.hl(), 0x1000);
    assert_eq!(c.bc(), 0x0001);

    // Second step (match)
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    // CPD decrements MEMPTR (PC+1 = 1 after the repeat) to 0
    assert_eq!(c.memptr, 0);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.bc(), 0x0000);
    assert!(c.get_flag(flags::ZERO));
    assert!(!c.get_flag(flags::PARITY));
}

// ============ INIR ============
#[test]
fn test_inir() {
    let mut c = create_z80(&[0xED, 0xB2]);
    c.set_hl(0x1000);
    c.set_bc(0x0280); // B=2, C=0x80
                      // Port 0x80 read
                      // Mock IO returns 0xFF

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.b, 0x01);
    assert_eq!(c.memory.read_byte(0x1000), 0xFF);
    assert!(!c.get_flag(flags::ZERO));
    assert!(c.get_flag(flags::ADD_SUB));

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.hl(), 0x1002);
    assert_eq!(c.b, 0x00);
    assert_eq!(c.memory.read_byte(0x1001), 0xFF);
    assert!(c.get_flag(flags::ZERO));
}

// ============ INDR ============
#[test]
fn test_indr() {
    let mut c = create_z80(&[0xED, 0xBA]);
    c.set_hl(0x1001);
    c.set_bc(0x0280);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.hl(), 0x1000);
    assert_eq!(c.b, 0x01);
    assert_eq!(c.memory.read_byte(0x1001), 0xFF);

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.b, 0x00);
    assert_eq!(c.memory.read_byte(0x1000), 0xFF);
    assert!(c.get_flag(flags::ZERO));
}

// ============ OTIR ============
#[test]
fn test_otir() {
    let mut c = create_z80(&[0xED, 0xB3]);
    c.set_hl(0x1000);
    c.set_bc(0x0280); // B=2, C=0x80
    c.memory.write_byte(0x1000, 0x11);
    c.memory.write_byte(0x1001, 0x22);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.b, 0x01);
    assert!(!c.get_flag(flags::ZERO));
    // N is bit 7 of the byte written (0x11, 0x22)
    assert!(!c.get_flag(flags::ADD_SUB));

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.hl(), 0x1002);
    assert_eq!(c.b, 0x00);
    assert!(c.get_flag(flags::ZERO));
}

// ============ OTDR ============
#[test]
fn test_otdr() {
    let mut c = create_z80(&[0xED, 0xBB]);
    c.set_hl(0x1001);
    c.set_bc(0x0280); // B=2, C=0x80
    c.memory.write_byte(0x1001, 0x11);
    c.memory.write_byte(0x1000, 0x22);

    // First step
    let t1 = c.step();
    assert_eq!(t1, 21);
    assert_eq!(c.pc, 0);
    assert_eq!(c.hl(), 0x1000);
    assert_eq!(c.b, 0x01);

    // Second step
    let t2 = c.step();
    assert_eq!(t2, 16);
    assert_eq!(c.pc, 2);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.b, 0x00);
    assert!(c.get_flag(flags::ZERO));
}
