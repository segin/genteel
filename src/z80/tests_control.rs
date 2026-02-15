#![allow(unused_imports)]
//! Unit tests for Z80 CPU - Part 4: Control Flow

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

// ============ JP nn ============
#[test]
fn test_jp_0000() {
    let mut c = z80(&[0xC3, 0x00, 0x00]);
    c.step();
    assert_eq!(c.pc, 0x0000);
}
#[test]
fn test_jp_1234() {
    let mut c = z80(&[0xC3, 0x34, 0x12]);
    c.step();
    assert_eq!(c.pc, 0x1234);
}
#[test]
fn test_jp_ffff() {
    let mut c = z80(&[0xC3, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.pc, 0xFFFF);
}

// ============ JP cc, nn ============
#[test]
fn test_jp_nz_taken() {
    let mut c = z80(&[0xC2, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_nz_not_taken() {
    let mut c = z80(&[0xC2, 0x00, 0x10]);
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 3);
}
#[test]
fn test_jp_z_taken() {
    let mut c = z80(&[0xCA, 0x00, 0x10]);
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_z_not_taken() {
    let mut c = z80(&[0xCA, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 3);
}
#[test]
fn test_jp_nc_taken() {
    let mut c = z80(&[0xD2, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_nc_not_taken() {
    let mut c = z80(&[0xD2, 0x00, 0x10]);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 3);
}
#[test]
fn test_jp_c_taken() {
    let mut c = z80(&[0xDA, 0x00, 0x10]);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_c_not_taken() {
    let mut c = z80(&[0xDA, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 3);
}
#[test]
fn test_jp_po_taken() {
    let mut c = z80(&[0xE2, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_pe_taken() {
    let mut c = z80(&[0xEA, 0x00, 0x10]);
    c.set_flag(flags::PARITY, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_p_taken() {
    let mut c = z80(&[0xF2, 0x00, 0x10]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_jp_m_taken() {
    let mut c = z80(&[0xFA, 0x00, 0x10]);
    c.set_flag(flags::SIGN, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}

// ============ JR d ============
#[test]
fn test_jr_0() {
    let mut c = z80(&[0x18, 0x00]);
    c.step();
    assert_eq!(c.pc, 2);
}
#[test]
fn test_jr_pos() {
    let mut c = z80(&[0x18, 0x05]);
    c.step();
    assert_eq!(c.pc, 7);
}
#[test]
fn test_jr_neg() {
    let mut c = z80(&[0x00, 0x00, 0x00, 0x18, 0xFC]);
    c.pc = 3;
    c.step();
    assert_eq!(c.pc, 1);
}
#[test]
fn test_jr_7f() {
    let mut c = z80(&[0x18, 0x7F]);
    c.step();
    assert_eq!(c.pc, 129);
}

// ============ JR cc, d ============
#[test]
fn test_jr_nz_taken() {
    let mut c = z80(&[0x20, 0x05]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 7);
}
#[test]
fn test_jr_nz_not_taken() {
    let mut c = z80(&[0x20, 0x05]);
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 2);
}
#[test]
fn test_jr_z_taken() {
    let mut c = z80(&[0x28, 0x05]);
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 7);
}
#[test]
fn test_jr_z_not_taken() {
    let mut c = z80(&[0x28, 0x05]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 2);
}
#[test]
fn test_jr_nc_taken() {
    let mut c = z80(&[0x30, 0x05]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 7);
}
#[test]
fn test_jr_nc_not_taken() {
    let mut c = z80(&[0x30, 0x05]);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 2);
}
#[test]
fn test_jr_c_taken() {
    let mut c = z80(&[0x38, 0x05]);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 7);
}
#[test]
fn test_jr_c_not_taken() {
    let mut c = z80(&[0x38, 0x05]);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ DJNZ d ============
#[test]
fn test_djnz_taken() {
    let mut c = z80(&[0x10, 0x05]);
    c.b = 5;
    c.step();
    assert_eq!(c.b, 4);
    assert_eq!(c.pc, 7);
}
#[test]
fn test_djnz_not_taken() {
    let mut c = z80(&[0x10, 0x05]);
    c.b = 1;
    c.step();
    assert_eq!(c.b, 0);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_djnz_loop() {
    let mut c = z80(&[0x10, 0xFE]);
    c.b = 3;
    c.step();
    assert_eq!(c.pc, 0);
    c.step();
    assert_eq!(c.pc, 0);
    c.step();
    assert_eq!(c.pc, 2);
}

// ============ CALL nn ============
#[test]
fn test_call() {
    let mut c = z80(&[0xCD, 0x00, 0x10]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x1000);
    assert_eq!(c.sp, 0x1FFE);
}
#[test]
fn test_call_stack() {
    let mut c = z80(&[0xCD, 0x00, 0x10]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x00);
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x03);
}

// ============ CALL cc, nn ============
#[test]
fn test_call_nz_taken() {
    let mut c = z80(&[0xC4, 0x00, 0x10]);
    c.sp = 0x2000;
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_call_nz_not_taken() {
    let mut c = z80(&[0xC4, 0x00, 0x10]);
    c.sp = 0x2000;
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 3);
    assert_eq!(c.sp, 0x2000);
}
#[test]
fn test_call_z_taken() {
    let mut c = z80(&[0xCC, 0x00, 0x10]);
    c.sp = 0x2000;
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_call_nc_taken() {
    let mut c = z80(&[0xD4, 0x00, 0x10]);
    c.sp = 0x2000;
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_call_c_taken() {
    let mut c = z80(&[0xDC, 0x00, 0x10]);
    c.sp = 0x2000;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}

// ============ RET ============
#[test]
fn test_ret() {
    let mut c = z80(&[0xC9]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x34);
    c.memory.write_byte(0x1FFF as u32, 0x12);
    c.step();
    assert_eq!(c.pc, 0x1234);
    assert_eq!(c.sp, 0x2000);
}

// ============ RET cc ============
#[test]
fn test_ret_nz_taken() {
    let mut c = z80(&[0xC0]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x10);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_ret_nz_not_taken() {
    let mut c = z80(&[0xC0]);
    c.sp = 0x1FFE;
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 1);
    assert_eq!(c.sp, 0x1FFE);
}
#[test]
fn test_ret_z_taken() {
    let mut c = z80(&[0xC8]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x10);
    c.set_flag(flags::ZERO, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_ret_nc_taken() {
    let mut c = z80(&[0xD0]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x10);
    c.f = 0;
    c.step();
    assert_eq!(c.pc, 0x1000);
}
#[test]
fn test_ret_c_taken() {
    let mut c = z80(&[0xD8]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x10);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.pc, 0x1000);
}

// ============ RST n ============
#[test]
fn test_rst_00() {
    let mut c = z80(&[0xC7]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0000);
}
#[test]
fn test_rst_08() {
    let mut c = z80(&[0xCF]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0008);
}
#[test]
fn test_rst_10() {
    let mut c = z80(&[0xD7]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0010);
}
#[test]
fn test_rst_18() {
    let mut c = z80(&[0xDF]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0018);
}
#[test]
fn test_rst_20() {
    let mut c = z80(&[0xE7]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0020);
}
#[test]
fn test_rst_28() {
    let mut c = z80(&[0xEF]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0028);
}
#[test]
fn test_rst_30() {
    let mut c = z80(&[0xF7]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0030);
}
#[test]
fn test_rst_38() {
    let mut c = z80(&[0xFF]);
    c.sp = 0x2000;
    c.step();
    assert_eq!(c.pc, 0x0038);
}

// ============ JP (HL) / JP (IX) / JP (IY) ============
#[test]
fn test_jp_hl() {
    let mut c = z80(&[0xE9]);
    c.set_hl(0x1234);
    c.step();
    assert_eq!(c.pc, 0x1234);
}
#[test]
fn test_jp_ix() {
    let mut c = z80(&[0xDD, 0xE9]);
    c.ix = 0x5678;
    c.step();
    assert_eq!(c.pc, 0x5678);
}
#[test]
fn test_jp_iy() {
    let mut c = z80(&[0xFD, 0xE9]);
    c.iy = 0x9ABC;
    c.step();
    assert_eq!(c.pc, 0x9ABC);
}

// ============ PUSH/POP ============
#[test]
fn test_push_bc() {
    let mut c = z80(&[0xC5]);
    c.sp = 0x2000;
    c.set_bc(0x1234);
    c.step();
    assert_eq!(c.sp, 0x1FFE);
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x12);
}
#[test]
fn test_push_de() {
    let mut c = z80(&[0xD5]);
    c.sp = 0x2000;
    c.set_de(0xABCD);
    c.step();
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0xCD);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0xAB);
}
#[test]
fn test_push_hl() {
    let mut c = z80(&[0xE5]);
    c.sp = 0x2000;
    c.set_hl(0x5678);
    c.step();
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x78);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x56);
}
#[test]
fn test_push_af() {
    let mut c = z80(&[0xF5]);
    c.sp = 0x2000;
    c.set_af(0x9ABC);
    c.step();
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0xBC);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x9A);
}
#[test]
fn test_pop_bc() {
    let mut c = z80(&[0xC1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x34);
    c.memory.write_byte(0x1FFF as u32, 0x12);
    c.step();
    assert_eq!(c.bc(), 0x1234);
    assert_eq!(c.sp, 0x2000);
}
#[test]
fn test_pop_de() {
    let mut c = z80(&[0xD1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0xCD);
    c.memory.write_byte(0x1FFF as u32, 0xAB);
    c.step();
    assert_eq!(c.de(), 0xABCD);
}
#[test]
fn test_pop_hl() {
    let mut c = z80(&[0xE1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x78);
    c.memory.write_byte(0x1FFF as u32, 0x56);
    c.step();
    assert_eq!(c.hl(), 0x5678);
}
#[test]
fn test_pop_af() {
    let mut c = z80(&[0xF1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0xBC);
    c.memory.write_byte(0x1FFF as u32, 0x9A);
    c.step();
    assert_eq!(c.af(), 0x9ABC);
}

// ============ HALT, DI, EI ============
#[test]
fn test_halt() {
    let mut c = z80(&[0x76]);
    c.step();
    assert!(c.halted);
}
#[test]
fn test_di() {
    let mut c = z80(&[0xF3]);
    c.iff1 = true;
    c.iff2 = true;
    c.step();
    assert!(!c.iff1);
    assert!(!c.iff2);
}
#[test]
fn test_ei() {
    let mut c = z80(&[0xFB]);
    c.iff1 = false;
    c.iff2 = false;
    c.step();
    assert!(c.iff1);
    assert!(c.iff2);
}
