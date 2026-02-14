//! Unit tests for Z80 CPU - Part 2: ALU Operations

#![allow(unused_variables, unused_mut)]
use super::*;

use crate::z80::test_utils::TestContext;

fn z80(program: &[u8]) -> (Z80, TestContext) {
    (Z80::new(), TestContext::new(program))
}

// ============ ADD A, r ============
#[test]
fn test_add_a_b_0() {
    let (mut c, mut context) = z80(&[0x80]);
    c.a = 0;
    c.b = 0;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_add_a_b_1() {
    let (mut c, mut context) = z80(&[0x80]);
    c.a = 1;
    c.b = 2;
    c.step(&mut context);
    assert_eq!(c.a, 3);
}
#[test]
fn test_add_a_b_carry() {
    let (mut c, mut context) = z80(&[0x80]);
    c.a = 0xFF;
    c.b = 1;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_add_a_b_half() {
    let (mut c, mut context) = z80(&[0x80]);
    c.a = 0x0F;
    c.b = 1;
    c.step(&mut context);
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn test_add_a_b_ovf() {
    let (mut c, mut context) = z80(&[0x80]);
    c.a = 0x7F;
    c.b = 1;
    c.step(&mut context);
    assert!(c.get_flag(flags::PARITY));
}
#[test]
fn test_add_a_c() {
    let (mut c, mut context) = z80(&[0x81]);
    c.a = 10;
    c.c = 20;
    c.step(&mut context);
    assert_eq!(c.a, 30);
}
#[test]
fn test_add_a_d() {
    let (mut c, mut context) = z80(&[0x82]);
    c.a = 50;
    c.d = 50;
    c.step(&mut context);
    assert_eq!(c.a, 100);
}
#[test]
fn test_add_a_e() {
    let (mut c, mut context) = z80(&[0x83]);
    c.a = 100;
    c.e = 100;
    c.step(&mut context);
    assert_eq!(c.a, 200);
}
#[test]
fn test_add_a_h() {
    let (mut c, mut context) = z80(&[0x84]);
    c.a = 0x80;
    c.h = 0x80;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_add_a_l() {
    let (mut c, mut context) = z80(&[0x85]);
    c.a = 0x40;
    c.l = 0x40;
    c.step(&mut context);
    assert_eq!(c.a, 0x80);
}
#[test]
fn test_add_a_a() {
    let (mut c, mut context) = z80(&[0x87]);
    c.a = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0x84);
}
#[test]
fn test_add_a_a_max() {
    let (mut c, mut context) = z80(&[0x87]);
    c.a = 0x80;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::CARRY));
}

// ============ ADC A, r ============
#[test]
fn test_adc_a_b_nc() {
    let (mut c, mut context) = z80(&[0x88]);
    c.a = 1;
    c.b = 2;
    c.f = 0;
    c.step(&mut context);
    assert_eq!(c.a, 3);
}
#[test]
fn test_adc_a_b_c() {
    let (mut c, mut context) = z80(&[0x88]);
    c.a = 1;
    c.b = 2;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 4);
}
#[test]
fn test_adc_a_b_ff_c() {
    let (mut c, mut context) = z80(&[0x88]);
    c.a = 0xFF;
    c.b = 0;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_adc_a_c_val() {
    let (mut c, mut context) = z80(&[0x89]);
    c.a = 0x10;
    c.c = 0x20;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0x31);
}
#[test]
fn test_adc_a_d_val() {
    let (mut c, mut context) = z80(&[0x8A]);
    c.a = 0x7F;
    c.d = 0;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::PARITY));
}

// ============ SUB r ============
#[test]
fn test_sub_b_0() {
    let (mut c, mut context) = z80(&[0x90]);
    c.a = 5;
    c.b = 5;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_sub_b_1() {
    let (mut c, mut context) = z80(&[0x90]);
    c.a = 10;
    c.b = 3;
    c.step(&mut context);
    assert_eq!(c.a, 7);
}
#[test]
fn test_sub_b_borrow() {
    let (mut c, mut context) = z80(&[0x90]);
    c.a = 0;
    c.b = 1;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_sub_b_n() {
    let (mut c, mut context) = z80(&[0x90]);
    c.a = 5;
    c.b = 3;
    c.step(&mut context);
    assert!(c.get_flag(flags::ADD_SUB));
}
#[test]
fn test_sub_c() {
    let (mut c, mut context) = z80(&[0x91]);
    c.a = 100;
    c.c = 50;
    c.step(&mut context);
    assert_eq!(c.a, 50);
}
#[test]
fn test_sub_d() {
    let (mut c, mut context) = z80(&[0x92]);
    c.a = 0x80;
    c.d = 1;
    c.step(&mut context);
    assert_eq!(c.a, 0x7F);
    assert!(c.get_flag(flags::PARITY));
}
#[test]
fn test_sub_e() {
    let (mut c, mut context) = z80(&[0x93]);
    c.a = 0xFF;
    c.e = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0);
}
#[test]
fn test_sub_h() {
    let (mut c, mut context) = z80(&[0x94]);
    c.a = 0x10;
    c.h = 0x01;
    c.step(&mut context);
    assert_eq!(c.a, 0x0F);
}
#[test]
fn test_sub_l() {
    let (mut c, mut context) = z80(&[0x95]);
    c.a = 0x20;
    c.l = 0x10;
    c.step(&mut context);
    assert_eq!(c.a, 0x10);
}
#[test]
fn test_sub_a() {
    let (mut c, mut context) = z80(&[0x97]);
    c.a = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}

// ============ SBC A, r ============
#[test]
fn test_sbc_b_nc() {
    let (mut c, mut context) = z80(&[0x98]);
    c.a = 10;
    c.b = 3;
    c.f = 0;
    c.step(&mut context);
    assert_eq!(c.a, 7);
}
#[test]
fn test_sbc_b_c() {
    let (mut c, mut context) = z80(&[0x98]);
    c.a = 10;
    c.b = 3;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 6);
}
#[test]
fn test_sbc_b_0_c() {
    let (mut c, mut context) = z80(&[0x98]);
    c.a = 0;
    c.b = 0;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_sbc_c_val() {
    let (mut c, mut context) = z80(&[0x99]);
    c.a = 0x50;
    c.c = 0x20;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0x2F);
}

// ============ AND r ============
#[test]
fn test_and_b_ff() {
    let (mut c, mut context) = z80(&[0xA0]);
    c.a = 0xFF;
    c.b = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_and_b_00() {
    let (mut c, mut context) = z80(&[0xA0]);
    c.a = 0xFF;
    c.b = 0x00;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_and_b_f0() {
    let (mut c, mut context) = z80(&[0xA0]);
    c.a = 0xFF;
    c.b = 0xF0;
    c.step(&mut context);
    assert_eq!(c.a, 0xF0);
}
#[test]
fn test_and_b_0f() {
    let (mut c, mut context) = z80(&[0xA0]);
    c.a = 0xFF;
    c.b = 0x0F;
    c.step(&mut context);
    assert_eq!(c.a, 0x0F);
}
#[test]
fn test_and_c() {
    let (mut c, mut context) = z80(&[0xA1]);
    c.a = 0xAA;
    c.c = 0x55;
    c.step(&mut context);
    assert_eq!(c.a, 0);
}
#[test]
fn test_and_d() {
    let (mut c, mut context) = z80(&[0xA2]);
    c.a = 0xAA;
    c.d = 0xAA;
    c.step(&mut context);
    assert_eq!(c.a, 0xAA);
}
#[test]
fn test_and_e() {
    let (mut c, mut context) = z80(&[0xA3]);
    c.a = 0x12;
    c.e = 0x34;
    c.step(&mut context);
    assert_eq!(c.a, 0x10);
}
#[test]
fn test_and_h_carry() {
    let (mut c, mut context) = z80(&[0xA4]);
    c.a = 0xFF;
    c.h = 0xFF;
    c.step(&mut context);
    assert!(!c.get_flag(flags::CARRY));
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn test_and_a() {
    let (mut c, mut context) = z80(&[0xA7]);
    c.a = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0x42);
}

// ============ XOR r ============
#[test]
fn test_xor_b_self() {
    let (mut c, mut context) = z80(&[0xA8]);
    c.a = 0x42;
    c.b = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_xor_b_ff() {
    let (mut c, mut context) = z80(&[0xA8]);
    c.a = 0x00;
    c.b = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_xor_c() {
    let (mut c, mut context) = z80(&[0xA9]);
    c.a = 0xAA;
    c.c = 0x55;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_xor_d() {
    let (mut c, mut context) = z80(&[0xAA]);
    c.a = 0xFF;
    c.d = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0);
}
#[test]
fn test_xor_e() {
    let (mut c, mut context) = z80(&[0xAB]);
    c.a = 0x0F;
    c.e = 0xF0;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_xor_a() {
    let (mut c, mut context) = z80(&[0xAF]);
    c.a = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_xor_a_clears() {
    let (mut c, mut context) = z80(&[0xAF]);
    c.a = 0x12;
    c.step(&mut context);
    assert_eq!(c.a, 0);
}

// ============ OR r ============
#[test]
fn test_or_b_00() {
    let (mut c, mut context) = z80(&[0xB0]);
    c.a = 0x00;
    c.b = 0x00;
    c.step(&mut context);
    assert_eq!(c.a, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_or_b_ff() {
    let (mut c, mut context) = z80(&[0xB0]);
    c.a = 0x00;
    c.b = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_or_b_f0() {
    let (mut c, mut context) = z80(&[0xB0]);
    c.a = 0x0F;
    c.b = 0xF0;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_or_c() {
    let (mut c, mut context) = z80(&[0xB1]);
    c.a = 0xAA;
    c.c = 0x55;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_or_d() {
    let (mut c, mut context) = z80(&[0xB2]);
    c.a = 0x00;
    c.d = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0x42);
}
#[test]
fn test_or_a() {
    let (mut c, mut context) = z80(&[0xB7]);
    c.a = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0x42);
}
#[test]
fn test_or_a_flags() {
    let (mut c, mut context) = z80(&[0xB7]);
    c.a = 0x42;
    c.step(&mut context);
    assert!(!c.get_flag(flags::CARRY));
    assert!(!c.get_flag(flags::HALF_CARRY));
}

// ============ CP r ============
#[test]
fn test_cp_b_eq() {
    let (mut c, mut context) = z80(&[0xB8]);
    c.a = 5;
    c.b = 5;
    c.step(&mut context);
    assert_eq!(c.a, 5);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_cp_b_lt() {
    let (mut c, mut context) = z80(&[0xB8]);
    c.a = 3;
    c.b = 5;
    c.step(&mut context);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_cp_b_gt() {
    let (mut c, mut context) = z80(&[0xB8]);
    c.a = 5;
    c.b = 3;
    c.step(&mut context);
    assert!(!c.get_flag(flags::CARRY));
    assert!(!c.get_flag(flags::ZERO));
}
#[test]
fn test_cp_c() {
    let (mut c, mut context) = z80(&[0xB9]);
    c.a = 0xFF;
    c.c = 0xFF;
    c.step(&mut context);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_cp_d() {
    let (mut c, mut context) = z80(&[0xBA]);
    c.a = 0;
    c.d = 1;
    c.step(&mut context);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_cp_a() {
    let (mut c, mut context) = z80(&[0xBF]);
    c.a = 0x42;
    c.step(&mut context);
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.a, 0x42);
}

// ============ INC r ============
#[test]
fn test_inc_b_0() {
    let (mut c, mut context) = z80(&[0x04]);
    c.b = 0;
    c.step(&mut context);
    assert_eq!(c.b, 1);
}
#[test]
fn test_inc_b_ff() {
    let (mut c, mut context) = z80(&[0x04]);
    c.b = 0xFF;
    c.step(&mut context);
    assert_eq!(c.b, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_inc_b_7f() {
    let (mut c, mut context) = z80(&[0x04]);
    c.b = 0x7F;
    c.step(&mut context);
    assert_eq!(c.b, 0x80);
    assert!(c.get_flag(flags::PARITY));
}
#[test]
fn test_inc_b_0f() {
    let (mut c, mut context) = z80(&[0x04]);
    c.b = 0x0F;
    c.step(&mut context);
    assert_eq!(c.b, 0x10);
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn test_inc_c() {
    let (mut c, mut context) = z80(&[0x0C]);
    c.c = 0x41;
    c.step(&mut context);
    assert_eq!(c.c, 0x42);
}
#[test]
fn test_inc_d() {
    let (mut c, mut context) = z80(&[0x14]);
    c.d = 0x99;
    c.step(&mut context);
    assert_eq!(c.d, 0x9A);
}
#[test]
fn test_inc_e() {
    let (mut c, mut context) = z80(&[0x1C]);
    c.e = 0xFE;
    c.step(&mut context);
    assert_eq!(c.e, 0xFF);
}
#[test]
fn test_inc_h() {
    let (mut c, mut context) = z80(&[0x24]);
    c.h = 0;
    c.step(&mut context);
    assert_eq!(c.h, 1);
}
#[test]
fn test_inc_l() {
    let (mut c, mut context) = z80(&[0x2C]);
    c.l = 0x7F;
    c.step(&mut context);
    assert_eq!(c.l, 0x80);
}
#[test]
fn test_inc_a() {
    let (mut c, mut context) = z80(&[0x3C]);
    c.a = 0;
    c.step(&mut context);
    assert_eq!(c.a, 1);
}
#[test]
fn test_inc_a_sign() {
    let (mut c, mut context) = z80(&[0x3C]);
    c.a = 0x7F;
    c.step(&mut context);
    assert!(c.get_flag(flags::SIGN));
}

// ============ DEC r ============
#[test]
fn test_dec_b_1() {
    let (mut c, mut context) = z80(&[0x05]);
    c.b = 1;
    c.step(&mut context);
    assert_eq!(c.b, 0);
    assert!(c.get_flag(flags::ZERO));
}
#[test]
fn test_dec_b_0() {
    let (mut c, mut context) = z80(&[0x05]);
    c.b = 0;
    c.step(&mut context);
    assert_eq!(c.b, 0xFF);
}
#[test]
fn test_dec_b_80() {
    let (mut c, mut context) = z80(&[0x05]);
    c.b = 0x80;
    c.step(&mut context);
    assert_eq!(c.b, 0x7F);
    assert!(c.get_flag(flags::PARITY));
}
#[test]
fn test_dec_b_10() {
    let (mut c, mut context) = z80(&[0x05]);
    c.b = 0x10;
    c.step(&mut context);
    assert_eq!(c.b, 0x0F);
    assert!(c.get_flag(flags::HALF_CARRY));
}
#[test]
fn test_dec_c() {
    let (mut c, mut context) = z80(&[0x0D]);
    c.c = 0x42;
    c.step(&mut context);
    assert_eq!(c.c, 0x41);
}
#[test]
fn test_dec_d() {
    let (mut c, mut context) = z80(&[0x15]);
    c.d = 0x01;
    c.step(&mut context);
    assert_eq!(c.d, 0);
}
#[test]
fn test_dec_e() {
    let (mut c, mut context) = z80(&[0x1D]);
    c.e = 0xFF;
    c.step(&mut context);
    assert_eq!(c.e, 0xFE);
}
#[test]
fn test_dec_h() {
    let (mut c, mut context) = z80(&[0x25]);
    c.h = 0x80;
    c.step(&mut context);
    assert_eq!(c.h, 0x7F);
}
#[test]
fn test_dec_l() {
    let (mut c, mut context) = z80(&[0x2D]);
    c.l = 0x01;
    c.step(&mut context);
    assert_eq!(c.l, 0);
}
#[test]
fn test_dec_a() {
    let (mut c, mut context) = z80(&[0x3D]);
    c.a = 0x42;
    c.step(&mut context);
    assert_eq!(c.a, 0x41);
}
#[test]
fn test_dec_a_n() {
    let (mut c, mut context) = z80(&[0x3D]);
    c.a = 1;
    c.step(&mut context);
    assert!(c.get_flag(flags::ADD_SUB));
}

// ============ ADD HL, rr ============
#[test]
fn test_add_hl_bc() {
    let (mut c, mut context) = z80(&[0x09]);
    c.set_hl(0x1000);
    c.set_bc(0x0100);
    c.step(&mut context);
    assert_eq!(c.hl(), 0x1100);
}
#[test]
fn test_add_hl_de() {
    let (mut c, mut context) = z80(&[0x19]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.step(&mut context);
    assert_eq!(c.hl(), 0x3000);
}
#[test]
fn test_add_hl_hl() {
    let (mut c, mut context) = z80(&[0x29]);
    c.set_hl(0x4000);
    c.step(&mut context);
    assert_eq!(c.hl(), 0x8000);
}
#[test]
fn test_add_hl_sp() {
    let (mut c, mut context) = z80(&[0x39]);
    c.set_hl(0x1000);
    c.sp = 0x0500;
    c.step(&mut context);
    assert_eq!(c.hl(), 0x1500);
}
#[test]
fn test_add_hl_bc_carry() {
    let (mut c, mut context) = z80(&[0x09]);
    c.set_hl(0xFFFF);
    c.set_bc(0x0001);
    c.step(&mut context);
    assert_eq!(c.hl(), 0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_add_hl_de_half() {
    let (mut c, mut context) = z80(&[0x19]);
    c.set_hl(0x0FFF);
    c.set_de(0x0001);
    c.step(&mut context);
    assert!(c.get_flag(flags::HALF_CARRY));
}

// ============ INC rr / DEC rr ============
#[test]
fn test_inc_bc() {
    let (mut c, mut context) = z80(&[0x03]);
    c.set_bc(0x00FF);
    c.step(&mut context);
    assert_eq!(c.bc(), 0x0100);
}
#[test]
fn test_inc_de() {
    let (mut c, mut context) = z80(&[0x13]);
    c.set_de(0xFFFE);
    c.step(&mut context);
    assert_eq!(c.de(), 0xFFFF);
}
#[test]
fn test_inc_hl() {
    let (mut c, mut context) = z80(&[0x23]);
    c.set_hl(0xFFFF);
    c.step(&mut context);
    assert_eq!(c.hl(), 0);
}
#[test]
fn test_inc_sp() {
    let (mut c, mut context) = z80(&[0x33]);
    c.sp = 0x7FFF;
    c.step(&mut context);
    assert_eq!(c.sp, 0x8000);
}
#[test]
fn test_dec_bc() {
    let (mut c, mut context) = z80(&[0x0B]);
    c.set_bc(0x0100);
    c.step(&mut context);
    assert_eq!(c.bc(), 0x00FF);
}
#[test]
fn test_dec_de() {
    let (mut c, mut context) = z80(&[0x1B]);
    c.set_de(0x0000);
    c.step(&mut context);
    assert_eq!(c.de(), 0xFFFF);
}
#[test]
fn test_dec_hl() {
    let (mut c, mut context) = z80(&[0x2B]);
    c.set_hl(0x8000);
    c.step(&mut context);
    assert_eq!(c.hl(), 0x7FFF);
}
#[test]
fn test_dec_sp() {
    let (mut c, mut context) = z80(&[0x3B]);
    c.sp = 0x0001;
    c.step(&mut context);
    assert_eq!(c.sp, 0);
}

// ============ ALU A, n (immediate) ============
#[test]
fn test_add_a_n() {
    let (mut c, mut context) = z80(&[0xC6, 0x10]);
    c.a = 0x20;
    c.step(&mut context);
    assert_eq!(c.a, 0x30);
}
#[test]
fn test_adc_a_n() {
    let (mut c, mut context) = z80(&[0xCE, 0x10]);
    c.a = 0x20;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0x31);
}
#[test]
fn test_sub_n() {
    let (mut c, mut context) = z80(&[0xD6, 0x10]);
    c.a = 0x30;
    c.step(&mut context);
    assert_eq!(c.a, 0x20);
}
#[test]
fn test_sbc_a_n() {
    let (mut c, mut context) = z80(&[0xDE, 0x10]);
    c.a = 0x30;
    c.set_flag(flags::CARRY, true);
    c.step(&mut context);
    assert_eq!(c.a, 0x1F);
}
#[test]
fn test_and_n() {
    let (mut c, mut context) = z80(&[0xE6, 0x0F]);
    c.a = 0xFF;
    c.step(&mut context);
    assert_eq!(c.a, 0x0F);
}
#[test]
fn test_xor_n() {
    let (mut c, mut context) = z80(&[0xEE, 0xFF]);
    c.a = 0xAA;
    c.step(&mut context);
    assert_eq!(c.a, 0x55);
}
#[test]
fn test_or_n() {
    let (mut c, mut context) = z80(&[0xF6, 0x0F]);
    c.a = 0xF0;
    c.step(&mut context);
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_cp_n() {
    let (mut c, mut context) = z80(&[0xFE, 0x42]);
    c.a = 0x42;
    c.step(&mut context);
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.a, 0x42);
}