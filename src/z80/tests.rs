#![allow(unused_imports)]
//! Comprehensive unit tests for Z80 CPU

use super::*; use crate::memory::{MemoryInterface, IoInterface};
use crate::memory::Memory;

fn create_z80(program: &[u8]) -> Z80<crate::memory::Memory, crate::z80::test_utils::TestIo> {
    let mut memory = Memory::new(0x10000);
    for (i, &byte) in program.iter().enumerate() {
        memory.data[i] = byte;
    }
    Z80::new(
        memory,
        crate::z80::test_utils::TestIo::default(),
    )
}

// ==================== Register Pair Tests ====================

#[test]
fn test_af_pair() {
    let mut z80 = create_z80(&[]);
    z80.set_af(0x1234);
    assert_eq!(z80.a, 0x12);
    assert_eq!(z80.f, 0x34);
    assert_eq!(z80.af(), 0x1234);
}

#[test]
fn test_bc_pair() {
    let mut z80 = create_z80(&[]);
    z80.set_bc(0xABCD);
    assert_eq!(z80.b, 0xAB);
    assert_eq!(z80.c, 0xCD);
    assert_eq!(z80.bc(), 0xABCD);
}

#[test]
fn test_de_pair() {
    let mut z80 = create_z80(&[]);
    z80.set_de(0x5678);
    assert_eq!(z80.d, 0x56);
    assert_eq!(z80.e, 0x78);
    assert_eq!(z80.de(), 0x5678);
}

#[test]
fn test_hl_pair() {
    let mut z80 = create_z80(&[]);
    z80.set_hl(0xBEEF);
    assert_eq!(z80.h, 0xBE);
    assert_eq!(z80.l, 0xEF);
    assert_eq!(z80.hl(), 0xBEEF);
}

// ==================== NOP Tests ====================

#[test]
fn test_nop() {
    let mut z80 = create_z80(&[0x00]);
    let cycles = z80.step();
    assert_eq!(z80.pc, 1);
    assert_eq!(cycles, 4);
}

#[test]
fn test_nop_no_side_effects() {
    let mut z80 = create_z80(&[0x00]);
    z80.a = 0x42;
    z80.set_bc(0x1234);
    z80.step();
    assert_eq!(z80.a, 0x42);
    assert_eq!(z80.bc(), 0x1234);
}

// ==================== LD rr, nn Tests ====================

#[test]
fn test_ld_bc_nn() {
    let mut z80 = create_z80(&[0x01, 0x34, 0x12]);
    z80.step();
    assert_eq!(z80.bc(), 0x1234);
    assert_eq!(z80.pc, 3);
}

#[test]
fn test_ld_de_nn() {
    let mut z80 = create_z80(&[0x11, 0xCD, 0xAB]);
    z80.step();
    assert_eq!(z80.de(), 0xABCD);
    assert_eq!(z80.pc, 3);
}

#[test]
fn test_ld_hl_nn() {
    let mut z80 = create_z80(&[0x21, 0xEF, 0xBE]);
    z80.step();
    assert_eq!(z80.hl(), 0xBEEF);
    assert_eq!(z80.pc, 3);
}

#[test]
fn test_ld_sp_nn() {
    let mut z80 = create_z80(&[0x31, 0x00, 0x80]);
    z80.step();
    assert_eq!(z80.sp, 0x8000);
    assert_eq!(z80.pc, 3);
}

// ==================== INC/DEC rr Tests ====================

#[test]
fn test_inc_bc() {
    let mut z80 = create_z80(&[0x03]);
    z80.set_bc(0x00FF);
    z80.step();
    assert_eq!(z80.bc(), 0x0100);
}

#[test]
fn test_inc_bc_wrap() {
    let mut z80 = create_z80(&[0x03]);
    z80.set_bc(0xFFFF);
    z80.step();
    assert_eq!(z80.bc(), 0x0000);
}

#[test]
fn test_dec_bc() {
    let mut z80 = create_z80(&[0x0B]);
    z80.set_bc(0x0100);
    z80.step();
    assert_eq!(z80.bc(), 0x00FF);
}

#[test]
fn test_dec_bc_wrap() {
    let mut z80 = create_z80(&[0x0B]);
    z80.set_bc(0x0000);
    z80.step();
    assert_eq!(z80.bc(), 0xFFFF);
}

// ==================== INC/DEC r Tests ====================

#[test]
fn test_inc_b() {
    let mut z80 = create_z80(&[0x04]);
    z80.b = 0x7F;
    z80.step();
    assert_eq!(z80.b, 0x80);
    assert!(z80.get_flag(flags::SIGN));
    assert!(z80.get_flag(flags::PARITY)); // Overflow
}

#[test]
fn test_dec_b() {
    let mut z80 = create_z80(&[0x05]);
    z80.b = 0x80;
    z80.step();
    assert_eq!(z80.b, 0x7F);
    assert!(z80.get_flag(flags::PARITY)); // Overflow
}

#[test]
fn test_inc_a_zero() {
    let mut z80 = create_z80(&[0x3C]);
    z80.a = 0xFF;
    z80.step();
    assert_eq!(z80.a, 0x00);
    assert!(z80.get_flag(flags::ZERO));
}

// ==================== LD r, r' Tests ====================

#[test]
fn test_ld_b_c() {
    let mut z80 = create_z80(&[0x41]);
    z80.c = 0x55;
    z80.step();
    assert_eq!(z80.b, 0x55);
}

#[test]
fn test_ld_a_b() {
    let mut z80 = create_z80(&[0x78]);
    z80.b = 0x42;
    z80.step();
    assert_eq!(z80.a, 0x42);
}

#[test]
fn test_ld_hl_indirect() {
    let mut z80 = create_z80(&[0x36, 0xAB]);
    z80.set_hl(0x0100);
    z80.step();
    assert_eq!(z80.memory.read_byte(0x0100 as u32), 0xAB);
}

// ==================== ALU Tests ====================

#[test]
fn test_add_a_b() {
    let mut z80 = create_z80(&[0x80]);
    z80.a = 0x10;
    z80.b = 0x20;
    z80.step();
    assert_eq!(z80.a, 0x30);
    assert!(!z80.get_flag(flags::ZERO));
    assert!(!z80.get_flag(flags::CARRY));
}

#[test]
fn test_add_a_overflow() {
    let mut z80 = create_z80(&[0x80]);
    z80.a = 0x7F;
    z80.b = 0x01;
    z80.step();
    assert_eq!(z80.a, 0x80);
    assert!(z80.get_flag(flags::PARITY)); // Overflow
    assert!(z80.get_flag(flags::SIGN));
}

#[test]
fn test_sub_a_b() {
    let mut z80 = create_z80(&[0x90]);
    z80.a = 0x30;
    z80.b = 0x10;
    z80.step();
    assert_eq!(z80.a, 0x20);
    assert!(z80.get_flag(flags::ADD_SUB));
}

#[test]
fn test_and_a() {
    let mut z80 = create_z80(&[0xA0]);
    z80.a = 0xF0;
    z80.b = 0x0F;
    z80.step();
    assert_eq!(z80.a, 0x00);
    assert!(z80.get_flag(flags::ZERO));
}

#[test]
fn test_or_a() {
    let mut z80 = create_z80(&[0xB0]);
    z80.a = 0xF0;
    z80.b = 0x0F;
    z80.step();
    assert_eq!(z80.a, 0xFF);
}

#[test]
fn test_xor_a() {
    let mut z80 = create_z80(&[0xA8]);
    z80.a = 0xFF;
    z80.b = 0xFF;
    z80.step();
    assert_eq!(z80.a, 0x00);
    assert!(z80.get_flag(flags::ZERO));
}

#[test]
fn test_cp() {
    let mut z80 = create_z80(&[0xB8]);
    z80.a = 0x10;
    z80.b = 0x10;
    z80.step();
    assert_eq!(z80.a, 0x10); // A unchanged
    assert!(z80.get_flag(flags::ZERO));
}

// ==================== Rotate Tests ====================

#[test]
fn test_rlca() {
    let mut z80 = create_z80(&[0x07]);
    z80.a = 0x85;
    z80.step();
    assert_eq!(z80.a, 0x0B);
    assert!(z80.get_flag(flags::CARRY));
}

#[test]
fn test_rrca() {
    let mut z80 = create_z80(&[0x0F]);
    z80.a = 0x81;
    z80.step();
    assert_eq!(z80.a, 0xC0);
    assert!(z80.get_flag(flags::CARRY));
}

// ==================== Jump/Call Tests ====================

#[test]
fn test_jp_nn() {
    let mut z80 = create_z80(&[0xC3, 0x00, 0x10]);
    z80.step();
    assert_eq!(z80.pc, 0x1000);
}

#[test]
fn test_jr_d() {
    let mut z80 = create_z80(&[0x18, 0x05]);
    z80.step();
    assert_eq!(z80.pc, 7); // 2 + 5
}

#[test]
fn test_jr_d_negative() {
    let mut z80 = create_z80(&[0x00, 0x00, 0x00, 0x00, 0x18, 0xFB]); // JR -5
    z80.pc = 4;
    z80.step();
    assert_eq!(z80.pc, 1); // 6 - 5
}

#[test]
fn test_call_nn() {
    let mut z80 = create_z80(&[0xCD, 0x00, 0x10]);
    z80.sp = 0x2000;
    z80.step();
    assert_eq!(z80.pc, 0x1000);
    assert_eq!(z80.sp, 0x1FFE);
}

#[test]
fn test_ret() {
    let mut z80 = create_z80(&[0xC9]);
    z80.sp = 0x1FFE;
    z80.memory.write_byte(0x1FFE as u32, 0x34);
    z80.memory.write_byte(0x1FFF as u32, 0x12);
    z80.step();
    assert_eq!(z80.pc, 0x1234);
    assert_eq!(z80.sp, 0x2000);
}

// ==================== Push/Pop Tests ====================

#[test]
fn test_push_bc() {
    let mut z80 = create_z80(&[0xC5]);
    z80.sp = 0x2000;
    z80.set_bc(0x1234);
    z80.step();
    assert_eq!(z80.sp, 0x1FFE);
    assert_eq!(z80.memory.read_byte(0x1FFE as u32), 0x34);
    assert_eq!(z80.memory.read_byte(0x1FFF as u32), 0x12);
}

#[test]
fn test_pop_bc() {
    let mut z80 = create_z80(&[0xC1]);
    z80.sp = 0x1FFE;
    z80.memory.write_byte(0x1FFE as u32, 0xCD);
    z80.memory.write_byte(0x1FFF as u32, 0xAB);
    z80.step();
    assert_eq!(z80.bc(), 0xABCD);
    assert_eq!(z80.sp, 0x2000);
}

// ==================== CB Prefix Tests ====================

#[test]
fn test_cb_rlc_b() {
    let mut z80 = create_z80(&[0xCB, 0x00]);
    z80.b = 0x85;
    z80.step();
    assert_eq!(z80.b, 0x0B);
    assert!(z80.get_flag(flags::CARRY));
}

#[test]
fn test_cb_bit_7_a() {
    let mut z80 = create_z80(&[0xCB, 0x7F]);
    z80.a = 0x80;
    z80.step();
    assert!(!z80.get_flag(flags::ZERO));
}

#[test]
fn test_cb_set_3_b() {
    let mut z80 = create_z80(&[0xCB, 0xD8]);
    z80.b = 0x00;
    z80.step();
    assert_eq!(z80.b, 0x08);
}

#[test]
fn test_cb_res_7_a() {
    let mut z80 = create_z80(&[0xCB, 0xBF]);
    z80.a = 0xFF;
    z80.step();
    assert_eq!(z80.a, 0x7F);
}

// ==================== IX/IY Tests ====================

#[test]
fn test_ld_ix_nn() {
    let mut z80 = create_z80(&[0xDD, 0x21, 0x34, 0x12]);
    z80.step();
    assert_eq!(z80.ix, 0x1234);
}

#[test]
fn test_ld_iy_nn() {
    let mut z80 = create_z80(&[0xFD, 0x21, 0xCD, 0xAB]);
    z80.step();
    assert_eq!(z80.iy, 0xABCD);
}

#[test]
fn test_ld_ix_d_n() {
    let mut z80 = create_z80(&[0xDD, 0x36, 0x05, 0x42]);
    z80.ix = 0x1000;
    z80.step();
    assert_eq!(z80.memory.read_byte(0x1005 as u32), 0x42);
}

// ==================== ED Prefix Tests ====================

#[test]
fn test_ed_neg() {
    let mut z80 = create_z80(&[0xED, 0x44]);
    z80.a = 0x01;
    z80.step();
    assert_eq!(z80.a, 0xFF);
}

#[test]
fn test_ed_ldi() {
    let mut z80 = create_z80(&[0xED, 0xA0]);
    z80.set_hl(0x1000);
    z80.set_de(0x2000);
    z80.set_bc(0x0010);
    z80.memory.write_byte(0x1000 as u32, 0x42);
    z80.step();
    assert_eq!(z80.memory.read_byte(0x2000 as u32), 0x42);
    assert_eq!(z80.hl(), 0x1001);
    assert_eq!(z80.de(), 0x2001);
    assert_eq!(z80.bc(), 0x000F);
}

// ==================== HALT Test ====================

#[test]
fn test_halt() {
    let mut z80 = create_z80(&[0x76]);
    z80.step();
    assert!(z80.halted);
}

// ==================== Exchange Tests ====================

#[test]
fn test_ex_af_af_prime() {
    let mut z80 = create_z80(&[0x08]);
    z80.a = 0x12;
    z80.f = 0x34;
    z80.a_prime = 0xAB;
    z80.f_prime = 0xCD;
    z80.step();
    assert_eq!(z80.a, 0xAB);
    assert_eq!(z80.f, 0xCD);
    assert_eq!(z80.a_prime, 0x12);
    assert_eq!(z80.f_prime, 0x34);
}

#[test]
fn test_exx() {
    let mut z80 = create_z80(&[0xD9]);
    z80.set_bc(0x1111);
    z80.set_de(0x2222);
    z80.set_hl(0x3333);
    z80.b_prime = 0xAA;
    z80.c_prime = 0xBB;
    z80.step();
    assert_eq!(z80.bc(), 0xAABB);
    assert_eq!(z80.b_prime, 0x11);
    assert_eq!(z80.c_prime, 0x11);
}

#[test]
fn test_ex_de_hl() {
    let mut z80 = create_z80(&[0xEB]);
    z80.set_de(0x1234);
    z80.set_hl(0xABCD);
    z80.step();
    assert_eq!(z80.de(), 0xABCD);
    assert_eq!(z80.hl(), 0x1234);
}
