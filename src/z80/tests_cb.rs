//! Unit tests for Z80 CPU - Part 3: CB Prefix (Bit Operations)

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() { m.data[i] = b; }
    Z80::new(Box::new(m))
}

// ============ RLC r ============
#[test] fn test_rlc_b_00() { let mut c = z80(&[0xCB, 0x00]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_b_01() { let mut c = z80(&[0xCB, 0x00]); c.b = 0x01; c.step(); assert_eq!(c.b, 0x02); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_b_80() { let mut c = z80(&[0xCB, 0x00]); c.b = 0x80; c.step(); assert_eq!(c.b, 0x01); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_b_ff() { let mut c = z80(&[0xCB, 0x00]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0xFF); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_b_85() { let mut c = z80(&[0xCB, 0x00]); c.b = 0x85; c.step(); assert_eq!(c.b, 0x0B); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_c() { let mut c = z80(&[0xCB, 0x01]); c.c = 0x40; c.step(); assert_eq!(c.c, 0x80); }
#[test] fn test_rlc_d() { let mut c = z80(&[0xCB, 0x02]); c.d = 0x81; c.step(); assert_eq!(c.d, 0x03); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_e() { let mut c = z80(&[0xCB, 0x03]); c.e = 0x55; c.step(); assert_eq!(c.e, 0xAA); }
#[test] fn test_rlc_h() { let mut c = z80(&[0xCB, 0x04]); c.h = 0xAA; c.step(); assert_eq!(c.h, 0x55); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rlc_l() { let mut c = z80(&[0xCB, 0x05]); c.l = 0x01; c.step(); assert_eq!(c.l, 0x02); }
#[test] fn test_rlc_a() { let mut c = z80(&[0xCB, 0x07]); c.a = 0x80; c.step(); assert_eq!(c.a, 0x01); assert!(c.get_flag(flags::CARRY)); }

// ============ RRC r ============
#[test] fn test_rrc_b_00() { let mut c = z80(&[0xCB, 0x08]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rrc_b_01() { let mut c = z80(&[0xCB, 0x08]); c.b = 0x01; c.step(); assert_eq!(c.b, 0x80); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rrc_b_80() { let mut c = z80(&[0xCB, 0x08]); c.b = 0x80; c.step(); assert_eq!(c.b, 0x40); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rrc_b_81() { let mut c = z80(&[0xCB, 0x08]); c.b = 0x81; c.step(); assert_eq!(c.b, 0xC0); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rrc_c() { let mut c = z80(&[0xCB, 0x09]); c.c = 0x02; c.step(); assert_eq!(c.c, 0x01); }
#[test] fn test_rrc_a() { let mut c = z80(&[0xCB, 0x0F]); c.a = 0x01; c.step(); assert_eq!(c.a, 0x80); assert!(c.get_flag(flags::CARRY)); }

// ============ RL r ============
#[test] fn test_rl_b_nc() { let mut c = z80(&[0xCB, 0x10]); c.b = 0x80; c.f = 0; c.step(); assert_eq!(c.b, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rl_b_c() { let mut c = z80(&[0xCB, 0x10]); c.b = 0x80; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.b, 0x01); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rl_b_00_c() { let mut c = z80(&[0xCB, 0x10]); c.b = 0x00; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.b, 0x01); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rl_c() { let mut c = z80(&[0xCB, 0x11]); c.c = 0x40; c.f = 0; c.step(); assert_eq!(c.c, 0x80); }
#[test] fn test_rl_a() { let mut c = z80(&[0xCB, 0x17]); c.a = 0x7F; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xFF); }

// ============ RR r ============
#[test] fn test_rr_b_nc() { let mut c = z80(&[0xCB, 0x18]); c.b = 0x01; c.f = 0; c.step(); assert_eq!(c.b, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rr_b_c() { let mut c = z80(&[0xCB, 0x18]); c.b = 0x01; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.b, 0x80); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_rr_b_00_c() { let mut c = z80(&[0xCB, 0x18]); c.b = 0x00; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.b, 0x80); assert!(!c.get_flag(flags::CARRY)); }
#[test] fn test_rr_a() { let mut c = z80(&[0xCB, 0x1F]); c.a = 0xFE; c.set_flag(flags::CARRY, true); c.step(); assert_eq!(c.a, 0xFF); }

// ============ SLA r ============
#[test] fn test_sla_b_00() { let mut c = z80(&[0xCB, 0x20]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); }
#[test] fn test_sla_b_01() { let mut c = z80(&[0xCB, 0x20]); c.b = 0x01; c.step(); assert_eq!(c.b, 0x02); }
#[test] fn test_sla_b_80() { let mut c = z80(&[0xCB, 0x20]); c.b = 0x80; c.step(); assert_eq!(c.b, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_sla_b_ff() { let mut c = z80(&[0xCB, 0x20]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0xFE); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_sla_c() { let mut c = z80(&[0xCB, 0x21]); c.c = 0x40; c.step(); assert_eq!(c.c, 0x80); }
#[test] fn test_sla_a() { let mut c = z80(&[0xCB, 0x27]); c.a = 0x55; c.step(); assert_eq!(c.a, 0xAA); }

// ============ SRA r ============
#[test] fn test_sra_b_00() { let mut c = z80(&[0xCB, 0x28]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); }
#[test] fn test_sra_b_01() { let mut c = z80(&[0xCB, 0x28]); c.b = 0x01; c.step(); assert_eq!(c.b, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_sra_b_80() { let mut c = z80(&[0xCB, 0x28]); c.b = 0x80; c.step(); assert_eq!(c.b, 0xC0); } // Sign preserved
#[test] fn test_sra_b_81() { let mut c = z80(&[0xCB, 0x28]); c.b = 0x81; c.step(); assert_eq!(c.b, 0xC0); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_sra_b_ff() { let mut c = z80(&[0xCB, 0x28]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0xFF); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_sra_a() { let mut c = z80(&[0xCB, 0x2F]); c.a = 0xAA; c.step(); assert_eq!(c.a, 0xD5); }

// ============ SRL r ============
#[test] fn test_srl_b_00() { let mut c = z80(&[0xCB, 0x38]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); }
#[test] fn test_srl_b_01() { let mut c = z80(&[0xCB, 0x38]); c.b = 0x01; c.step(); assert_eq!(c.b, 0x00); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_srl_b_80() { let mut c = z80(&[0xCB, 0x38]); c.b = 0x80; c.step(); assert_eq!(c.b, 0x40); }
#[test] fn test_srl_b_81() { let mut c = z80(&[0xCB, 0x38]); c.b = 0x81; c.step(); assert_eq!(c.b, 0x40); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_srl_b_ff() { let mut c = z80(&[0xCB, 0x38]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0x7F); assert!(c.get_flag(flags::CARRY)); }
#[test] fn test_srl_a() { let mut c = z80(&[0xCB, 0x3F]); c.a = 0xAA; c.step(); assert_eq!(c.a, 0x55); }

// ============ BIT b, r ============
// BIT 0
#[test] fn test_bit_0_b_0() { let mut c = z80(&[0xCB, 0x40]); c.b = 0x00; c.step(); assert!(c.get_flag(flags::ZERO)); }
#[test] fn test_bit_0_b_1() { let mut c = z80(&[0xCB, 0x40]); c.b = 0x01; c.step(); assert!(!c.get_flag(flags::ZERO)); }
#[test] fn test_bit_0_b_fe() { let mut c = z80(&[0xCB, 0x40]); c.b = 0xFE; c.step(); assert!(c.get_flag(flags::ZERO)); }
// BIT 1
#[test] fn test_bit_1_c_0() { let mut c = z80(&[0xCB, 0x49]); c.c = 0x00; c.step(); assert!(c.get_flag(flags::ZERO)); }
#[test] fn test_bit_1_c_2() { let mut c = z80(&[0xCB, 0x49]); c.c = 0x02; c.step(); assert!(!c.get_flag(flags::ZERO)); }
// BIT 7
#[test] fn test_bit_7_a_0() { let mut c = z80(&[0xCB, 0x7F]); c.a = 0x00; c.step(); assert!(c.get_flag(flags::ZERO)); }
#[test] fn test_bit_7_a_80() { let mut c = z80(&[0xCB, 0x7F]); c.a = 0x80; c.step(); assert!(!c.get_flag(flags::ZERO)); }
#[test] fn test_bit_7_a_7f() { let mut c = z80(&[0xCB, 0x7F]); c.a = 0x7F; c.step(); assert!(c.get_flag(flags::ZERO)); }
// Various bits
#[test] fn test_bit_3_d() { let mut c = z80(&[0xCB, 0x5A]); c.d = 0x08; c.step(); assert!(!c.get_flag(flags::ZERO)); }
#[test] fn test_bit_4_e() { let mut c = z80(&[0xCB, 0x63]); c.e = 0x10; c.step(); assert!(!c.get_flag(flags::ZERO)); }
#[test] fn test_bit_5_h() { let mut c = z80(&[0xCB, 0x6C]); c.h = 0x20; c.step(); assert!(!c.get_flag(flags::ZERO)); }
#[test] fn test_bit_6_l() { let mut c = z80(&[0xCB, 0x75]); c.l = 0x40; c.step(); assert!(!c.get_flag(flags::ZERO)); }

// ============ RES b, r ============
#[test] fn test_res_0_b() { let mut c = z80(&[0xCB, 0x80]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0xFE); }
#[test] fn test_res_1_c() { let mut c = z80(&[0xCB, 0x89]); c.c = 0xFF; c.step(); assert_eq!(c.c, 0xFD); }
#[test] fn test_res_2_d() { let mut c = z80(&[0xCB, 0x92]); c.d = 0xFF; c.step(); assert_eq!(c.d, 0xFB); }
#[test] fn test_res_3_e() { let mut c = z80(&[0xCB, 0x9B]); c.e = 0xFF; c.step(); assert_eq!(c.e, 0xF7); }
#[test] fn test_res_4_h() { let mut c = z80(&[0xCB, 0xA4]); c.h = 0xFF; c.step(); assert_eq!(c.h, 0xEF); }
#[test] fn test_res_5_l() { let mut c = z80(&[0xCB, 0xAD]); c.l = 0xFF; c.step(); assert_eq!(c.l, 0xDF); }
#[test] fn test_res_6_a() { let mut c = z80(&[0xCB, 0xB7]); c.a = 0xFF; c.step(); assert_eq!(c.a, 0xBF); }
#[test] fn test_res_7_b() { let mut c = z80(&[0xCB, 0xB8]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0x7F); }
#[test] fn test_res_0_already() { let mut c = z80(&[0xCB, 0x80]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x00); }

// ============ SET b, r ============
#[test] fn test_set_0_b() { let mut c = z80(&[0xCB, 0xC0]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x01); }
#[test] fn test_set_1_c() { let mut c = z80(&[0xCB, 0xC9]); c.c = 0x00; c.step(); assert_eq!(c.c, 0x02); }
#[test] fn test_set_2_d() { let mut c = z80(&[0xCB, 0xD2]); c.d = 0x00; c.step(); assert_eq!(c.d, 0x04); }
#[test] fn test_set_3_e() { let mut c = z80(&[0xCB, 0xDB]); c.e = 0x00; c.step(); assert_eq!(c.e, 0x08); }
#[test] fn test_set_4_h() { let mut c = z80(&[0xCB, 0xE4]); c.h = 0x00; c.step(); assert_eq!(c.h, 0x10); }
#[test] fn test_set_5_l() { let mut c = z80(&[0xCB, 0xED]); c.l = 0x00; c.step(); assert_eq!(c.l, 0x20); }
#[test] fn test_set_6_a() { let mut c = z80(&[0xCB, 0xF7]); c.a = 0x00; c.step(); assert_eq!(c.a, 0x40); }
#[test] fn test_set_7_b() { let mut c = z80(&[0xCB, 0xF8]); c.b = 0x00; c.step(); assert_eq!(c.b, 0x80); }
#[test] fn test_set_7_already() { let mut c = z80(&[0xCB, 0xF8]); c.b = 0xFF; c.step(); assert_eq!(c.b, 0xFF); }
