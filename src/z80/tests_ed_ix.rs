//! Unit tests for Z80 CPU - Part 5: ED Prefix and IX/IY

use super::*;
use crate::memory::Memory;

fn z80(program: &[u8]) -> Z80 {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    Z80::new(
        Box::new(m),
        Box::new(crate::z80::test_utils::TestIo::default()),
    )
}

// ============ ED: NEG ============
#[test]
fn test_neg_00() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x00;
    c.step();
    assert_eq!(c.a, 0x00);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_neg_01() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x01;
    c.step();
    assert_eq!(c.a, 0xFF);
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 2);
}
#[test]
fn test_neg_80() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0x80;
    c.step();
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::PARITY));
    assert_eq!(c.pc, 2);
}
#[test]
fn test_neg_ff() {
    let mut c = z80(&[0xED, 0x44]);
    c.a = 0xFF;
    c.step();
    assert_eq!(c.a, 0x01);
    assert_eq!(c.pc, 2);
}

// ============ ED: LD I,A / LD A,I / LD R,A / LD A,R ============
#[test]
fn test_ld_i_a() {
    let mut c = z80(&[0xED, 0x47]);
    c.a = 0x42;
    c.step();
    assert_eq!(c.i, 0x42);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ld_r_a() {
    let mut c = z80(&[0xED, 0x4F]);
    c.a = 0x55;
    c.step();
    assert_eq!(c.r, 0x55);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ld_a_i() {
    let mut c = z80(&[0xED, 0x57]);
    c.i = 0x77;
    c.step();
    assert_eq!(c.a, 0x77);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ld_a_r() {
    let mut c = z80(&[0xED, 0x5F]);
    c.r = 0x98;
    c.step();
    assert_eq!(c.a, 0x9A);
    assert_eq!(c.pc, 2);
} // R increments during fetch

// ============ ED: IM 0/1/2 ============
#[test]
fn test_im_0() {
    let mut c = z80(&[0xED, 0x46]);
    c.step();
    assert_eq!(c.im, 0);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_im_1() {
    let mut c = z80(&[0xED, 0x56]);
    c.step();
    assert_eq!(c.im, 1);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_im_2() {
    let mut c = z80(&[0xED, 0x5E]);
    c.step();
    assert_eq!(c.im, 2);
    assert_eq!(c.pc, 2);
}

// ============ ED: RETN / RETI ============
#[test]
fn test_retn() {
    let mut c = z80(&[0xED, 0x45]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x10);
    c.iff2 = true;
    c.step();
    assert_eq!(c.pc, 0x1000);
    assert!(c.iff1);
}
#[test]
fn test_reti() {
    let mut c = z80(&[0xED, 0x4D]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0x00);
    c.memory.write_byte(0x1FFF as u32, 0x20);
    c.step();
    assert_eq!(c.pc, 0x2000);
}

// ============ ED: ADC HL,rr / SBC HL,rr ============
#[test]
fn test_adc_hl_bc() {
    let mut c = z80(&[0xED, 0x4A]);
    c.set_hl(0x1000);
    c.set_bc(0x0100);
    c.f = 0;
    c.step();
    assert_eq!(c.hl(), 0x1100);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_adc_hl_bc_c() {
    let mut c = z80(&[0xED, 0x4A]);
    c.set_hl(0x1000);
    c.set_bc(0x0100);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.hl(), 0x1101);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_adc_hl_de() {
    let mut c = z80(&[0xED, 0x5A]);
    c.set_hl(0x8000);
    c.set_de(0x8000);
    c.f = 0;
    c.step();
    assert_eq!(c.hl(), 0);
    assert!(c.get_flag(flags::CARRY));
}
#[test]
fn test_sbc_hl_bc() {
    let mut c = z80(&[0xED, 0x42]);
    c.set_hl(0x1000);
    c.set_bc(0x0100);
    c.f = 0;
    c.step();
    assert_eq!(c.hl(), 0x0F00);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_sbc_hl_bc_c() {
    let mut c = z80(&[0xED, 0x42]);
    c.set_hl(0x1000);
    c.set_bc(0x0100);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.hl(), 0x0EFF);
}
#[test]
fn test_sbc_hl_de() {
    let mut c = z80(&[0xED, 0x52]);
    c.set_hl(0x0100);
    c.set_de(0x0200);
    c.f = 0;
    c.step();
    assert!(c.get_flag(flags::CARRY));
}

// ============ ED: LD (nn),rr / LD rr,(nn) ============
#[test]
fn test_ed_ld_nn_bc() {
    let mut c = z80(&[0xED, 0x43, 0x00, 0x50]);
    c.set_bc(0x1234);
    c.step();
    assert_eq!(c.memory.read_byte(0x5000 as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x5001 as u32), 0x12);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ed_ld_bc_nn() {
    let mut c = z80(&[0xED, 0x4B, 0x00, 0x50]);
    c.memory.write_byte(0x5000 as u32, 0xCD);
    c.memory.write_byte(0x5001 as u32, 0xAB);
    c.step();
    assert_eq!(c.bc(), 0xABCD);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ed_ld_nn_de() {
    let mut c = z80(&[0xED, 0x53, 0x00, 0x60]);
    c.set_de(0x5678);
    c.step();
    assert_eq!(c.memory.read_byte(0x6000 as u32), 0x78);
    assert_eq!(c.memory.read_byte(0x6001 as u32), 0x56);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ed_ld_de_nn() {
    let mut c = z80(&[0xED, 0x5B, 0x00, 0x60]);
    c.memory.write_byte(0x6000 as u32, 0xEF);
    c.memory.write_byte(0x6001 as u32, 0xBE);
    c.step();
    assert_eq!(c.de(), 0xBEEF);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ed_ld_nn_sp() {
    let mut c = z80(&[0xED, 0x73, 0x00, 0x70]);
    c.sp = 0x9ABC;
    c.step();
    assert_eq!(c.memory.read_byte(0x7000 as u32), 0xBC);
    assert_eq!(c.memory.read_byte(0x7001 as u32), 0x9A);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ed_ld_sp_nn() {
    let mut c = z80(&[0xED, 0x7B, 0x00, 0x70]);
    c.memory.write_byte(0x7000 as u32, 0xDE);
    c.memory.write_byte(0x7001 as u32, 0xF0);
    c.step();
    assert_eq!(c.sp, 0xF0DE);
    assert_eq!(c.pc, 4);
}

// ============ ED: LDI/LDD ============
#[test]
fn test_ldi() {
    let mut c = z80(&[0xED, 0xA0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0010);
    c.memory.write_byte(0x1000 as u32, 0x42);
    c.step();
    assert_eq!(c.memory.read_byte(0x2000 as u32), 0x42);
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.de(), 0x2001);
    assert_eq!(c.bc(), 0x000F);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ldd() {
    let mut c = z80(&[0xED, 0xA8]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0010);
    c.memory.write_byte(0x1000 as u32, 0x55);
    c.step();
    assert_eq!(c.memory.read_byte(0x2000 as u32), 0x55);
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.de(), 0x1FFF);
    assert_eq!(c.bc(), 0x000F);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ldi_bc_1() {
    let mut c = z80(&[0xED, 0xA0]);
    c.set_hl(0x1000);
    c.set_de(0x2000);
    c.set_bc(0x0001);
    c.memory.write_byte(0x1000 as u32, 0xAA);
    c.step();
    assert_eq!(c.bc(), 0x0000);
    assert!(!c.get_flag(flags::PARITY));
}

// ============ ED: CPI/CPD ============
#[test]
fn test_cpi_match() {
    let mut c = z80(&[0xED, 0xA1]);
    c.a = 0x42;
    c.set_hl(0x1000);
    c.set_bc(0x0010);
    c.memory.write_byte(0x1000 as u32, 0x42);
    c.step();
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.hl(), 0x1001);
    assert_eq!(c.bc(), 0x000F);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_cpi_no_match() {
    let mut c = z80(&[0xED, 0xA1]);
    c.a = 0x42;
    c.set_hl(0x1000);
    c.set_bc(0x0010);
    c.memory.write_byte(0x1000 as u32, 0x00);
    c.step();
    assert!(!c.get_flag(flags::ZERO));
}
#[test]
fn test_cpd() {
    let mut c = z80(&[0xED, 0xA9]);
    c.a = 0x55;
    c.set_hl(0x1000);
    c.set_bc(0x0010);
    c.memory.write_byte(0x1000 as u32, 0x55);
    c.step();
    assert!(c.get_flag(flags::ZERO));
    assert_eq!(c.hl(), 0x0FFF);
    assert_eq!(c.pc, 2);
}

// ============ IX instructions ============
#[test]
fn test_ld_ix_nn() {
    let mut c = z80(&[0xDD, 0x21, 0x34, 0x12]);
    c.step();
    assert_eq!(c.ix, 0x1234);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ld_ix_nn_ffff() {
    let mut c = z80(&[0xDD, 0x21, 0xFF, 0xFF]);
    c.step();
    assert_eq!(c.ix, 0xFFFF);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_inc_ix() {
    let mut c = z80(&[0xDD, 0x23]);
    c.ix = 0x00FF;
    c.step();
    assert_eq!(c.ix, 0x0100);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_dec_ix() {
    let mut c = z80(&[0xDD, 0x2B]);
    c.ix = 0x0100;
    c.step();
    assert_eq!(c.ix, 0x00FF);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_inc_ix_wrap() {
    let mut c = z80(&[0xDD, 0x23]);
    c.ix = 0xFFFF;
    c.step();
    assert_eq!(c.ix, 0x0000);
}
#[test]
fn test_dec_ix_wrap() {
    let mut c = z80(&[0xDD, 0x2B]);
    c.ix = 0x0000;
    c.step();
    assert_eq!(c.ix, 0xFFFF);
}
#[test]
fn test_ld_nn_ix() {
    let mut c = z80(&[0xDD, 0x22, 0x00, 0x50]);
    c.ix = 0xABCD;
    c.step();
    assert_eq!(c.memory.read_byte(0x5000 as u32), 0xCD);
    assert_eq!(c.memory.read_byte(0x5001 as u32), 0xAB);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ld_ix_nn_ind() {
    let mut c = z80(&[0xDD, 0x2A, 0x00, 0x50]);
    c.memory.write_byte(0x5000 as u32, 0x78);
    c.memory.write_byte(0x5001 as u32, 0x56);
    c.step();
    assert_eq!(c.ix, 0x5678);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_push_ix() {
    let mut c = z80(&[0xDD, 0xE5]);
    c.sp = 0x2000;
    c.ix = 0x1234;
    c.step();
    assert_eq!(c.sp, 0x1FFE);
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x12);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_pop_ix() {
    let mut c = z80(&[0xDD, 0xE1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0xCD);
    c.memory.write_byte(0x1FFF as u32, 0xAB);
    c.step();
    assert_eq!(c.ix, 0xABCD);
    assert_eq!(c.sp, 0x2000);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_ld_sp_ix() {
    let mut c = z80(&[0xDD, 0xF9]);
    c.ix = 0x8000;
    c.step();
    assert_eq!(c.sp, 0x8000);
    assert_eq!(c.pc, 2);
}

// ============ IX+d indexed addressing ============
#[test]
fn test_ld_ix_d_n() {
    let mut c = z80(&[0xDD, 0x36, 0x05, 0x42]);
    c.ix = 0x1000;
    c.step();
    assert_eq!(c.memory.read_byte(0x1005 as u32), 0x42);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ld_ix_d_n_neg() {
    let mut c = z80(&[0xDD, 0x36, 0xFB, 0x99]);
    c.ix = 0x1000;
    c.step();
    assert_eq!(c.memory.read_byte(0x0FFB as u32), 0x99);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ld_b_ix_d() {
    let mut c = z80(&[0xDD, 0x46, 0x10]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1010 as u32, 0xAB);
    c.step();
    assert_eq!(c.b, 0xAB);
    assert_eq!(c.pc, 3);
}
#[test]
fn test_ld_c_ix_d() {
    let mut c = z80(&[0xDD, 0x4E, 0x20]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1020 as u32, 0xCD);
    c.step();
    assert_eq!(c.c, 0xCD);
    assert_eq!(c.pc, 3);
}
#[test]
fn test_ld_ix_d_b() {
    let mut c = z80(&[0xDD, 0x70, 0x05]);
    c.ix = 0x2000;
    c.b = 0x11;
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0x11);
    assert_eq!(c.pc, 3);
}
#[test]
fn test_inc_ix_d() {
    let mut c = z80(&[0xDD, 0x34, 0x10]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1010 as u32, 0x41);
    c.step();
    assert_eq!(c.memory.read_byte(0x1010 as u32), 0x42);
    assert_eq!(c.pc, 3);
}
#[test]
fn test_dec_ix_d() {
    let mut c = z80(&[0xDD, 0x35, 0x10]);
    c.ix = 0x1000;
    c.memory.write_byte(0x1010 as u32, 0x42);
    c.step();
    assert_eq!(c.memory.read_byte(0x1010 as u32), 0x41);
    assert_eq!(c.pc, 3);
}

// ============ IY instructions ============
#[test]
fn test_ld_iy_nn() {
    let mut c = z80(&[0xFD, 0x21, 0xCD, 0xAB]);
    c.step();
    assert_eq!(c.iy, 0xABCD);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_inc_iy() {
    let mut c = z80(&[0xFD, 0x23]);
    c.iy = 0x1234;
    c.step();
    assert_eq!(c.iy, 0x1235);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_dec_iy() {
    let mut c = z80(&[0xFD, 0x2B]);
    c.iy = 0x1234;
    c.step();
    assert_eq!(c.iy, 0x1233);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_push_iy() {
    let mut c = z80(&[0xFD, 0xE5]);
    c.sp = 0x2000;
    c.iy = 0x5678;
    c.step();
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x78);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x56);
    assert_eq!(c.pc, 2);
}
#[test]
fn test_pop_iy() {
    let mut c = z80(&[0xFD, 0xE1]);
    c.sp = 0x1FFE;
    c.memory.write_byte(0x1FFE as u32, 0xEF);
    c.memory.write_byte(0x1FFF as u32, 0xBE);
    c.step();
    assert_eq!(c.iy, 0xBEEF);
    assert_eq!(c.pc, 2);
}

// ============ IY+d indexed addressing ============
#[test]
fn test_ld_iy_d_n() {
    let mut c = z80(&[0xFD, 0x36, 0x05, 0x77]);
    c.iy = 0x2000;
    c.step();
    assert_eq!(c.memory.read_byte(0x2005 as u32), 0x77);
    assert_eq!(c.pc, 4);
}
#[test]
fn test_ld_a_iy_d() {
    let mut c = z80(&[0xFD, 0x7E, 0x10]);
    c.iy = 0x2000;
    c.memory.write_byte(0x2010 as u32, 0xEE);
    c.step();
    assert_eq!(c.a, 0xEE);
    assert_eq!(c.pc, 3);
}
#[test]
fn test_ld_iy_d_a() {
    let mut c = z80(&[0xFD, 0x77, 0x05]);
    c.iy = 0x3000;
    c.a = 0x22;
    c.step();
    assert_eq!(c.memory.read_byte(0x3005 as u32), 0x22);
    assert_eq!(c.pc, 3);
}

// ============ Exchange instructions ============
#[test]
fn test_ex_af_af() {
    let mut c = z80(&[0x08]);
    c.a = 0x12;
    c.f = 0x34;
    c.a_prime = 0xAB;
    c.f_prime = 0xCD;
    c.step();
    assert_eq!(c.a, 0xAB);
    assert_eq!(c.f, 0xCD);
    assert_eq!(c.a_prime, 0x12);
    assert_eq!(c.f_prime, 0x34);
    assert_eq!(c.pc, 1);
}
#[test]
fn test_exx() {
    let mut c = z80(&[0xD9]);
    c.set_bc(0x1111);
    c.set_de(0x2222);
    c.set_hl(0x3333);
    c.b_prime = 0xAA;
    c.c_prime = 0xBB;
    c.d_prime = 0xCC;
    c.e_prime = 0xDD;
    c.h_prime = 0xEE;
    c.l_prime = 0xFF;
    c.step();
    assert_eq!(c.bc(), 0xAABB);
    assert_eq!(c.de(), 0xCCDD);
    assert_eq!(c.hl(), 0xEEFF);
    assert_eq!(c.pc, 1);
}
#[test]
fn test_ex_de_hl() {
    let mut c = z80(&[0xEB]);
    c.set_de(0x1234);
    c.set_hl(0xABCD);
    c.step();
    assert_eq!(c.de(), 0xABCD);
    assert_eq!(c.hl(), 0x1234);
    assert_eq!(c.pc, 1);
}
#[test]
fn test_ex_sp_hl() {
    let mut c = z80(&[0xE3]);
    c.sp = 0x1FFE;
    c.set_hl(0x1234);
    c.memory.write_byte(0x1FFE as u32, 0xCD);
    c.memory.write_byte(0x1FFF as u32, 0xAB);
    c.step();
    assert_eq!(c.hl(), 0xABCD);
    assert_eq!(c.memory.read_byte(0x1FFE as u32), 0x34);
    assert_eq!(c.memory.read_byte(0x1FFF as u32), 0x12);
    assert_eq!(c.pc, 1);
}

// ============ Misc instructions ============
#[test]
fn test_cpl() {
    let mut c = z80(&[0x2F]);
    c.a = 0x55;
    c.step();
    assert_eq!(c.a, 0xAA);
    assert_eq!(c.pc, 1);
}
#[test]
fn test_cpl_00() {
    let mut c = z80(&[0x2F]);
    c.a = 0x00;
    c.step();
    assert_eq!(c.a, 0xFF);
}
#[test]
fn test_cpl_ff() {
    let mut c = z80(&[0x2F]);
    c.a = 0xFF;
    c.step();
    assert_eq!(c.a, 0x00);
}
#[test]
fn test_scf() {
    let mut c = z80(&[0x37]);
    c.f = 0;
    c.step();
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}

// ============ Undocumented HALT (DD 76, FD 76) ============
#[test]
fn test_dd_76_halt() {
    let mut c = z80(&[0xDD, 0x76]);
    let t = c.step();
    assert!(c.halted, "CPU should be halted");
    assert_eq!(c.pc, 2, "PC should be incremented by 2");
    assert_eq!(t, 8, "T-states should be 8");
}

#[test]
fn test_fd_76_halt() {
    let mut c = z80(&[0xFD, 0x76]);
    let t = c.step();
    assert!(c.halted, "CPU should be halted");
    assert_eq!(c.pc, 2, "PC should be incremented by 2");
    assert_eq!(t, 8, "T-states should be 8");
}
#[test]
fn test_ccf_set() {
    let mut c = z80(&[0x3F]);
    c.set_flag(flags::CARRY, true);
    c.step();
    assert!(!c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}
#[test]
fn test_ccf_clear() {
    let mut c = z80(&[0x3F]);
    c.f = 0;
    c.step();
    assert!(c.get_flag(flags::CARRY));
}

// ============ Rotate A (non-CB) ============
#[test]
fn test_rlca_pc() {
    let mut c = z80(&[0x07]);
    c.a = 0x85;
    c.step();
    assert_eq!(c.a, 0x0B);
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}
#[test]
fn test_rrca_pc() {
    let mut c = z80(&[0x0F]);
    c.a = 0x81;
    c.step();
    assert_eq!(c.a, 0xC0);
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}
#[test]
fn test_rla_pc() {
    let mut c = z80(&[0x17]);
    c.a = 0x80;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.a, 0x01);
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}
#[test]
fn test_rra_pc() {
    let mut c = z80(&[0x1F]);
    c.a = 0x01;
    c.set_flag(flags::CARRY, true);
    c.step();
    assert_eq!(c.a, 0x80);
    assert!(c.get_flag(flags::CARRY));
    assert_eq!(c.pc, 1);
}
