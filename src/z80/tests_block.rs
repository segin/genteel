//! Z80 Block Operation Tests
//!
//! Tests for repeated block transfer and search operations:
//! LDIR, LDDR, CPIR, CPDR, INIR, INDR, OTIR, OTDR

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(m)
}

// ============ LDIR (Load, Increment, Repeat) ============

#[test]
fn ldir_single_byte() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0001);
    c.memory.data[0x1000] = 0x42;
    c.step();
    assert_eq!(c.memory.data[0x2000], 0x42);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.de(), 0x2001);
    assert_eq!(c.bc(), 0x0000);
    assert_eq!(c.pc, 2); // Done, moved past
}

#[test]
fn ldir_multi_byte() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0003);
    c.memory.data[0x1000] = 0x11;
    c.memory.data[0x1001] = 0x22;
    c.memory.data[0x1002] = 0x33;
    
    // First iteration
    c.step();
    assert_eq!(c.memory.data[0x2000], 0x11);
    assert_eq!(c.bc(), 0x0002);
    assert_eq!(c.pc, 0); // Repeat
    
    // Second iteration
    c.step();
    assert_eq!(c.memory.data[0x2001], 0x22);
    assert_eq!(c.bc(), 0x0001);
    
    // Third iteration (last)
    c.step();
    assert_eq!(c.memory.data[0x2002], 0x33);
    assert_eq!(c.bc(), 0x0000);
    assert_eq!(c.pc, 2); // Done
}

#[test]
fn ldir_256_bytes() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0100);
    
    // Fill source
    for i in 0..256 {
        c.memory.data[0x1000 + i] = i as u8;
    }
    
    // Execute all 256 iterations
    for _ in 0..256 {
        c.step();
    }
    
    assert_eq!(c.bc(), 0x0000);
    for i in 0..256 {
        assert_eq!(c.memory.data[0x2000 + i], i as u8);
    }
}

#[test]
fn ldir_pv_flag() {
    // P/V is reset when BC becomes 0
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0001);
    c.step();
    assert!(!c.get_flag(flags::PARITY));
}

// ============ LDDR (Load, Decrement, Repeat) ============

#[test]
fn lddr_single_byte() {
    let mut c = z80(&[0xED, 0xB8]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0001);
    c.memory.data[0x1000] = 0x55;
    c.step();
    assert_eq!(c.memory.data[0x2000], 0x55);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.de(), 0x1FFF);
    assert_eq!(c.bc(), 0x0000);
}

#[test]
fn lddr_multi_byte() {
    let mut c = z80(&[0xED, 0xB8]);
    c.set_hl(0x1002);
    c.set_de(0x2002);
    c.set_bc(0x0003);
    c.memory.data[0x1000] = 0x11;
    c.memory.data[0x1001] = 0x22;
    c.memory.data[0x1002] = 0x33;
    
    for _ in 0..3 {
        c.step();
    }
    
    assert_eq!(c.memory.data[0x2000], 0x11);
    assert_eq!(c.memory.data[0x2001], 0x22);
    assert_eq!(c.memory.data[0x2002], 0x33);
}

// ============ CPIR (Compare, Increment, Repeat) ============

#[test]
fn cpir_found_first() {
    let mut c = z80(&[0xED, 0xB1]);
    c.a = 0x42;
    c.set_hl(0x1000);
    c.set_bc(0x0010);
    c.memory.data[0x1000] = 0x42;
    c.step();
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.bc(), 0x000F);
    assert_eq!(c.pc, 2); // Done - found
}

#[test]
fn cpir_found_third() {
    let mut c = z80(&[0xED, 0xB1]);
    c.a = 0x42;
    c.set_hl(0x1000);
    c.set_bc(0x0010);
    c.memory.data[0x1000] = 0x00;
    c.memory.data[0x1001] = 0x00;
    c.memory.data[0x1002] = 0x42;
    
    c.step(); // Not found, continue
    assert!(!c.get_flag(flags::ZERO));
    assert_eq!(c.pc, 0);
    
    c.step();
    assert_eq!(c.pc, 0);
    
    c.step(); // Found!
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.pc, 2);
}

#[test]
fn cpir_not_found() {
    let mut c = z80(&[0xED, 0xB1]);
    c.a = 0xFF;
    c.set_hl(0x1000);
    c.set_bc(0x0003);
    c.memory.data[0x1000] = 0x00;
    c.memory.data[0x1001] = 0x00;
    c.memory.data[0x1002] = 0x00;
    
    for _ in 0..3 {
        c.step();
    }
    
    assert!(!c.get_flag(flags::ZERO));
    assert_eq!(c.bc(), 0x0000);
    assert!(!c.get_flag(flags::PARITY)); // BC = 0
}

// ============ CPDR (Compare, Decrement, Repeat) ============

#[test]
fn cpdr_found() {
    let mut c = z80(&[0xED, 0xB9]);
    c.a = 0x42;
    c.set_hl(0x1002);
    c.set_bc(0x0003);
    c.memory.data[0x1000] = 0x42;
    c.memory.data[0x1001] = 0x00;
    c.memory.data[0x1002] = 0x00;
    
    c.step(); // 0x1002 != 0x42
    c.step(); // 0x1001 != 0x42
    c.step(); // 0x1000 == 0x42!
    
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.hl(), 0x0FFF);
}

// ============ BC=0 edge case ============

#[test]
fn ldir_bc_0_initial() {
    // When BC starts at 0, it wraps to 0xFFFF
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0000);
    c.memory.data[0x1000] = 0xAA;
    
    c.step();
    
    // BC wraps from 0 to 0xFFFF
    assert_eq!(c.bc(), 0xFFFF);
    assert_eq!(c.memory.data[0x2000], 0xAA);
}

// ============ Overlapping regions ============

#[test]
fn ldir_overlapping_forward() {
    // Copy from 0x1000-0x1002 to 0x1001-0x1003 (overlap)
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x1001);
    c.set_bc(0x0003);
    c.memory.data[0x1000] = 0x11;
    c.memory.data[0x1001] = 0x22;
    c.memory.data[0x1002] = 0x33;
    
    for _ in 0..3 {
        c.step();
    }
    
    // Result: 11 11 11 11 (propagated)
    assert_eq!(c.memory.data[0x1000], 0x11);
    assert_eq!(c.memory.data[0x1001], 0x11);
    assert_eq!(c.memory.data[0x1002], 0x11);
    assert_eq!(c.memory.data[0x1003], 0x11);
}

// ============ Memory wrap ============

#[test]
fn ldir_wrap_around() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0xFFFE);
    c.set_de(0x0100);
    c.set_bc(0x0003);
    c.memory.data[0xFFFE] = 0xAA;
    c.memory.data[0xFFFF] = 0xBB;
    c.memory.data[0x0000] = 0xCC;
    
    for _ in 0..3 {
        c.step();
    }
    
    assert_eq!(c.memory.data[0x0100], 0xAA);
    assert_eq!(c.memory.data[0x0101], 0xBB);
    assert_eq!(c.memory.data[0x0102], 0xCC);
    assert_eq!(c.hl(), 0x0001); // Wrapped
}

// ============ N flag preserved ============

#[test]
fn ldir_n_flag() {
    let mut c = z80(&[0xED, 0xB0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0001);
    c.step();
    assert!(!c.get_flag(flags::ADD_SUB)); // N is reset
}

#[test]
fn cpir_n_flag() {
    let mut c = z80(&[0xED, 0xB1]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.set_bc(0x0001);
    c.step();
    assert!(c.get_flag(flags::ADD_SUB)); // N is set (compare is subtraction)
}

// ============ H flag in CPI/CPD ============

#[test]
fn cpi_half_carry() {
    let mut c = z80(&[0xED, 0xA1]);
    c.a = 0x10;
    c.set_hl(0x1000);
    c.set_bc(0x0001);
    c.memory.data[0x1000] = 0x01; // 0x10 - 0x01 = 0x0F, no half borrow
    c.step();
    assert!(!c.get_flag(flags::HALF_CARRY));
}

#[test]
fn cpi_half_borrow() {
    let mut c = z80(&[0xED, 0xA1]);
    c.a = 0x00;
    c.set_hl(0x1000);
    c.set_bc(0x0001);
    c.memory.data[0x1000] = 0x01; // 0x00 - 0x01 = borrow from bit 4
    c.step();
    assert!(c.get_flag(flags::HALF_CARRY));
}
