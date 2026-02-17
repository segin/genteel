#![allow(unused_imports)]
//! Unit tests for Z80 CPU - Part 1: Load Instructions

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

// ============ LD r, n (8-bit immediate) ============
#[test]
fn test_ld_b_n() {
    let mut c = z80(&[0x06, 0x42]);
    c.step();
    assert_eq!(c.b, 0x42);
}
#[test]
fn test_ld_c_n() {
    let mut c = z80(&[0x0E, 0x42]);
    c.step();
    assert_eq!(c.c, 0x42);
}
#[test]
fn test_ld_d_n() {
    let mut c = z80(&[0x16, 0x42]);
    c.step();
    assert_eq!(c.d, 0x42);
}
#[test]
fn test_ld_e_n() {
    let mut c = z80(&[0x1E, 0x42]);
    c.step();
    assert_eq!(c.e, 0x42);
}
#[test]
fn test_ld_h_n() {
    let mut c = z80(&[0x26, 0x42]);
    c.step();
    assert_eq!(c.h, 0x42);
}
#[test]
fn test_ld_l_n() {
    let mut c = z80(&[0x2E, 0x42]);
    c.step();
    assert_eq!(c.l, 0x42);
}
#[test]
fn test_ld_a_n() {
    let mut c = z80(&[0x3E, 0x42]);
    c.step();
    assert_eq!(c.a, 0x42);
}
#[test]
fn test_ld_b_n_00() {
    let mut c = z80(&[0x06, 0x00]);
    c.step();
    assert_eq!(c.b, 0x00);
}
#[test]
fn test_ld_b_n_ff() {
    let mut c = z80(&[0x06, 0xFF]);
    c.step();
    assert_eq!(c.b, 0xFF);
}
#[test]
fn test_ld_c_n_00() {
    let mut c = z80(&[0x0E, 0x00]);
    c.step();
    assert_eq!(c.c, 0x00);
}
#[test]
fn test_ld_c_n_ff() {
    let mut c = z80(&[0x0E, 0xFF]);
    c.step();
    assert_eq!(c.c, 0xFF);
}
#[test]
fn test_ld_d_n_00() {
    let mut c = z80(&[0x16, 0x00]);
    c.step();
    assert_eq!(c.d, 0x00);
}
#[test]
fn test_ld_d_n_ff() {
    let mut c = z80(&[0x16, 0xFF]);
    c.step();
    assert_eq!(c.d, 0xFF);
}
#[test]
fn test_ld_e_n_00() {
    let mut c = z80(&[0x1E, 0x00]);
    c.step();
    assert_eq!(c.e, 0x00);
}
#[test]
fn test_ld_e_n_ff() {
    let mut c = z80(&[0x1E, 0xFF]);
    c.step();
    assert_eq!(c.e, 0xFF);
}
#[test]
fn test_ld_h_n_00() {
    let mut c = z80(&[0x26, 0x00]);
    c.step();
    assert_eq!(c.h, 0x00);
}
#[test]
fn test_ld_h_n_ff() {
    let mut c = z80(&[0x26, 0xFF]);
    c.step();
    assert_eq!(c.h, 0xFF);
}
#[test]
fn test_ld_l_n_00() {
    let mut c = z80(&[0x2E, 0x00]);
    c.step();
    assert_eq!(c.l, 0x00);
}
#[test]
fn test_ld_l_n_ff() {
    let mut c = z80(&[0x2E, 0xFF]);
    c.step();
    assert_eq!(c.l, 0xFF);
}
#[test]
fn test_ld_a_n_00() {
    let mut c = z80(&[0x3E, 0x00]);
    c.step();
    assert_eq!(c.a, 0x00);
}
#[test]
fn test_ld_a_n_ff() {
    let mut c = z80(&[0x3E, 0xFF]);
    c.step();
    assert_eq!(c.a, 0xFF);
}

// ============ LD rr, nn (16-bit immediate) ============
#[test]
fn test_ld_bc_0000() {
    let mut c = z80(&[0x01, 0x00, 0x00]);
    c.step();
    assert_eq!(c.bc(), 0x0000);
}
#[test]
fn test_ld_bc_ffff() {
    let mut c = z80(&[0x01, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.bc(), 0xFFFF);
}
#[test]
fn test_ld_bc_1234() {
    let mut c = z80(&[0x01, 0x34, 0x12]);
    c.step();
    assert_eq!(c.bc(), 0x1234);
}
#[test]
fn test_ld_bc_abcd() {
    let mut c = z80(&[0x01, 0xCD, 0xAB]);
    c.step();
    assert_eq!(c.bc(), 0xABCD);
}
#[test]
fn test_ld_de_0000() {
    let mut c = z80(&[0x11, 0x00, 0x00]);
    c.step();
    assert_eq!(c.de(), 0x0000);
}
#[test]
fn test_ld_de_ffff() {
    let mut c = z80(&[0x11, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.de(), 0xFFFF);
}
#[test]
fn test_ld_de_1234() {
    let mut c = z80(&[0x11, 0x34, 0x12]);
    c.step();
    assert_eq!(c.de(), 0x1234);
}
#[test]
fn test_ld_de_5678() {
    let mut c = z80(&[0x11, 0x78, 0x56]);
    c.step();
    assert_eq!(c.de(), 0x5678);
}
#[test]
fn test_ld_hl_0000() {
    let mut c = z80(&[0x21, 0x00, 0x00]);
    c.step();
    assert_eq!(c.hl(), 0x0000);
}
#[test]
fn test_ld_hl_ffff() {
    let mut c = z80(&[0x21, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.hl(), 0xFFFF);
}
#[test]
fn test_ld_hl_1234() {
    let mut c = z80(&[0x21, 0x34, 0x12]);
    c.step();
    assert_eq!(c.hl(), 0x1234);
}
#[test]
fn test_ld_hl_beef() {
    let mut c = z80(&[0x21, 0xEF, 0xBE]);
    c.step();
    assert_eq!(c.hl(), 0xBEEF);
}
#[test]
fn test_ld_sp_0000() {
    let mut c = z80(&[0x31, 0x00, 0x00]);
    c.step();
    assert_eq!(c.sp, 0x0000);
}
#[test]
fn test_ld_sp_ffff() {
    let mut c = z80(&[0x31, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.sp, 0xFFFF);
}
#[test]
fn test_ld_sp_8000() {
    let mut c = z80(&[0x31, 0x00, 0x80]);
    c.step();
    assert_eq!(c.sp, 0x8000);
}

// ============ LD r, r' (register to register) ============
#[test]
fn test_ld_b_b() {
    let mut c = z80(&[0x40]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.b, 0x11);
}
#[test]
fn test_ld_b_c() {
    let mut c = z80(&[0x41]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.b, 0x22);
}
#[test]
fn test_ld_b_d() {
    let mut c = z80(&[0x42]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.b, 0x33);
}
#[test]
fn test_ld_b_e() {
    let mut c = z80(&[0x43]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.b, 0x44);
}
#[test]
fn test_ld_b_h() {
    let mut c = z80(&[0x44]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.b, 0x55);
}
#[test]
fn test_ld_b_l() {
    let mut c = z80(&[0x45]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.b, 0x66);
}
#[test]
fn test_ld_b_a() {
    let mut c = z80(&[0x47]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.b, 0x77);
}
#[test]
fn test_ld_c_b() {
    let mut c = z80(&[0x48]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.c, 0x11);
}
#[test]
fn test_ld_c_c() {
    let mut c = z80(&[0x49]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.c, 0x22);
}
#[test]
fn test_ld_c_d() {
    let mut c = z80(&[0x4A]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.c, 0x33);
}
#[test]
fn test_ld_c_e() {
    let mut c = z80(&[0x4B]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.c, 0x44);
}
#[test]
fn test_ld_c_h() {
    let mut c = z80(&[0x4C]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.c, 0x55);
}
#[test]
fn test_ld_c_l() {
    let mut c = z80(&[0x4D]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.c, 0x66);
}
#[test]
fn test_ld_c_a() {
    let mut c = z80(&[0x4F]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.c, 0x77);
}
#[test]
fn test_ld_d_b() {
    let mut c = z80(&[0x50]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.d, 0x11);
}
#[test]
fn test_ld_d_c() {
    let mut c = z80(&[0x51]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.d, 0x22);
}
#[test]
fn test_ld_d_d() {
    let mut c = z80(&[0x52]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.d, 0x33);
}
#[test]
fn test_ld_d_e() {
    let mut c = z80(&[0x53]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.d, 0x44);
}
#[test]
fn test_ld_d_h() {
    let mut c = z80(&[0x54]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.d, 0x55);
}
#[test]
fn test_ld_d_l() {
    let mut c = z80(&[0x55]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.d, 0x66);
}
#[test]
fn test_ld_d_a() {
    let mut c = z80(&[0x57]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.d, 0x77);
}
#[test]
fn test_ld_e_b() {
    let mut c = z80(&[0x58]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.e, 0x11);
}
#[test]
fn test_ld_e_c() {
    let mut c = z80(&[0x59]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.e, 0x22);
}
#[test]
fn test_ld_e_d() {
    let mut c = z80(&[0x5A]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.e, 0x33);
}
#[test]
fn test_ld_e_e() {
    let mut c = z80(&[0x5B]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.e, 0x44);
}
#[test]
fn test_ld_e_h() {
    let mut c = z80(&[0x5C]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.e, 0x55);
}
#[test]
fn test_ld_e_l() {
    let mut c = z80(&[0x5D]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.e, 0x66);
}
#[test]
fn test_ld_e_a() {
    let mut c = z80(&[0x5F]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.e, 0x77);
}
#[test]
fn test_ld_h_b() {
    let mut c = z80(&[0x60]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.h, 0x11);
}
#[test]
fn test_ld_h_c() {
    let mut c = z80(&[0x61]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.h, 0x22);
}
#[test]
fn test_ld_h_d() {
    let mut c = z80(&[0x62]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.h, 0x33);
}
#[test]
fn test_ld_h_e() {
    let mut c = z80(&[0x63]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.h, 0x44);
}
#[test]
fn test_ld_h_h() {
    let mut c = z80(&[0x64]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.h, 0x55);
}
#[test]
fn test_ld_h_l() {
    let mut c = z80(&[0x65]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.h, 0x66);
}
#[test]
fn test_ld_h_a() {
    let mut c = z80(&[0x67]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.h, 0x77);
}
#[test]
fn test_ld_l_b() {
    let mut c = z80(&[0x68]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.l, 0x11);
}
#[test]
fn test_ld_l_c() {
    let mut c = z80(&[0x69]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.l, 0x22);
}
#[test]
fn test_ld_l_d() {
    let mut c = z80(&[0x6A]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.l, 0x33);
}
#[test]
fn test_ld_l_e() {
    let mut c = z80(&[0x6B]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.l, 0x44);
}
#[test]
fn test_ld_l_h() {
    let mut c = z80(&[0x6C]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.l, 0x55);
}
#[test]
fn test_ld_l_l() {
    let mut c = z80(&[0x6D]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.l, 0x66);
}
#[test]
fn test_ld_l_a() {
    let mut c = z80(&[0x6F]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.l, 0x77);
}
#[test]
fn test_ld_a_b() {
    let mut c = z80(&[0x78]);
    c.b = 0x11;
    c.step();
    assert_eq!(c.a, 0x11);
}
#[test]
fn test_ld_a_c() {
    let mut c = z80(&[0x79]);
    c.c = 0x22;
    c.step();
    assert_eq!(c.a, 0x22);
}
#[test]
fn test_ld_a_d() {
    let mut c = z80(&[0x7A]);
    c.d = 0x33;
    c.step();
    assert_eq!(c.a, 0x33);
}
#[test]
fn test_ld_a_e() {
    let mut c = z80(&[0x7B]);
    c.e = 0x44;
    c.step();
    assert_eq!(c.a, 0x44);
}
#[test]
fn test_ld_a_h() {
    let mut c = z80(&[0x7C]);
    c.h = 0x55;
    c.step();
    assert_eq!(c.a, 0x55);
}
#[test]
fn test_ld_a_l() {
    let mut c = z80(&[0x7D]);
    c.l = 0x66;
    c.step();
    assert_eq!(c.a, 0x66);
}
#[test]
fn test_ld_a_a() {
    let mut c = z80(&[0x7F]);
    c.a = 0x77;
    c.step();
    assert_eq!(c.a, 0x77);
}

// ============ LD r, (HL) ============
#[test]
fn test_ld_b_hl_ind() {
    let mut c = z80(&[0x46]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0xAB);
    c.step();
    assert_eq!(c.b, 0xAB);
}
#[test]
fn test_ld_c_hl_ind() {
    let mut c = z80(&[0x4E]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0xCD);
    c.step();
    assert_eq!(c.c, 0xCD);
}
#[test]
fn test_ld_d_hl_ind() {
    let mut c = z80(&[0x56]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0xEF);
    c.step();
    assert_eq!(c.d, 0xEF);
}
#[test]
fn test_ld_e_hl_ind() {
    let mut c = z80(&[0x5E]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0x12);
    c.step();
    assert_eq!(c.e, 0x12);
}
#[test]
fn test_ld_h_hl_ind() {
    let mut c = z80(&[0x66]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0x34);
    c.step();
    assert_eq!(c.h, 0x34);
}
#[test]
fn test_ld_l_hl_ind() {
    let mut c = z80(&[0x6E]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0x56);
    c.step();
    assert_eq!(c.l, 0x56);
}
#[test]
fn test_ld_a_hl_ind() {
    let mut c = z80(&[0x7E]);
    c.set_hl(0x100);
    c.memory.write_byte(0x100 as u32, 0x78);
    c.step();
    assert_eq!(c.a, 0x78);
}

// ============ LD (HL), r ============
#[test]
fn test_ld_hl_b() {
    let mut c = z80(&[0x70]);
    c.set_hl(0x200);
    c.b = 0x11;
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x11);
}
#[test]
fn test_ld_hl_c() {
    let mut c = z80(&[0x71]);
    c.set_hl(0x200);
    c.c = 0x22;
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x22);
}
#[test]
fn test_ld_hl_d() {
    let mut c = z80(&[0x72]);
    c.set_hl(0x200);
    c.d = 0x33;
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x33);
}
#[test]
fn test_ld_hl_e() {
    let mut c = z80(&[0x73]);
    c.set_hl(0x200);
    c.e = 0x44;
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x44);
}
#[test]
fn test_ld_hl_h() {
    let mut c = z80(&[0x74]);
    c.set_hl(0x200);
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x02);
}
#[test]
fn test_ld_hl_l() {
    let mut c = z80(&[0x75]);
    c.set_hl(0x200);
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x00);
}
#[test]
fn test_ld_hl_a() {
    let mut c = z80(&[0x77]);
    c.set_hl(0x200);
    c.a = 0x77;
    c.step();
    assert_eq!(c.memory.read_byte(0x200 as u32), 0x77);
}

// ============ LD (BC), A and LD A, (BC) ============
#[test]
fn test_ld_bc_a() {
    let mut c = z80(&[0x02]);
    c.set_bc(0x300);
    c.a = 0xAA;
    c.step();
    assert_eq!(c.memory.read_byte(0x300 as u32), 0xAA);
}
#[test]
fn test_ld_a_bc() {
    let mut c = z80(&[0x0A]);
    c.set_bc(0x300);
    c.memory.write_byte(0x300 as u32, 0xBB);
    c.step();
    assert_eq!(c.a, 0xBB);
}

// ============ LD (DE), A and LD A, (DE) ============
#[test]
fn test_ld_de_a() {
    let mut c = z80(&[0x12]);
    c.set_de(0x400);
    c.a = 0xCC;
    c.step();
    assert_eq!(c.memory.read_byte(0x400 as u32), 0xCC);
}
#[test]
fn test_ld_a_de() {
    let mut c = z80(&[0x1A]);
    c.set_de(0x400);
    c.memory.write_byte(0x400 as u32, 0xDD);
    c.step();
    assert_eq!(c.a, 0xDD);
}

// ============ LD (nn), A and LD A, (nn) ============
#[test]
fn test_ld_nn_a() {
    let mut c = z80(&[0x32, 0x00, 0x50]);
    c.a = 0xEE;
    c.step();
    assert_eq!(c.memory.read_byte(0x5000 as u32), 0xEE);
}
#[test]
fn test_ld_a_nn() {
    let mut c = z80(&[0x3A, 0x00, 0x50]);
    c.memory.write_byte(0x5000 as u32, 0xFF);
    c.step();
    assert_eq!(c.a, 0xFF);
}

// ============ LD (nn), HL and LD HL, (nn) ============
#[test]
fn test_ld_nn_hl() {
    let mut c = z80(&[0x22, 0x00, 0x60]);
    c.set_hl(0x1234);
    c.step();
    assert_eq!(c.memory.read_byte(0x6000 as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x6001 as u32), 0x12);
}
#[test]
fn test_ld_hl_nn_ind() {
    let mut c = z80(&[0x2A, 0x00, 0x60]);
    c.memory.write_byte(0x6000 as u32, 0x78);
    c.memory.write_byte(0x6001 as u32, 0x56);
    c.step();
    assert_eq!(c.hl(), 0x5678);
}
