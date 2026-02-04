//! Exhaustive Z80 ALU Verification
//!
//! "Golden Reference" model for Z80 ALU.
//! Verifies standard and undocumented flags (X/Y).

use super::*;
use crate::memory::Memory;

// fast rng
struct XorShift64 { state: u64 }
impl XorShift64 {
    fn new(seed: u64) -> Self { Self { state: seed } }
    fn next(&mut self) -> u64 {
        let mut x = self.state;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.state = x;
        x
    }
    fn next_u8(&mut self) -> u8 { (self.next() >> 32) as u8 }
    fn next_u16(&mut self) -> u16 { (self.next() >> 48) as u16 }
}

fn z80_setup() -> Z80 {
    let m = Memory::new(0x10000);
    Z80::new(Box::new(m), Box::new(crate::z80::test_utils::TestIo::default()))
}

// ============ Reference Models ============

fn ref_add(val1: u8, val2: u8) -> (u8, u8) {
    let res = val1.wrapping_add(val2);
    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let h = ((val1 & 0x0F) + (val2 & 0x0F)) > 0x0F;
    let v_calc = ((val1 ^ !val2) & (val1 ^ res) & 0x80) != 0;
    let n = false;
    let c = (val1 as u16 + val2 as u16) > 0xFF;
    
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_adc(val1: u8, val2: u8, carry_in: bool) -> (u8, u8) {
    let c_val = if carry_in { 1 } else { 0 };
    let res_wide = val1 as u16 + val2 as u16 + c_val as u16;
    let res = res_wide as u8;
    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let h = ((val1 & 0x0F) + (val2 & 0x0F) + c_val) > 0x0F;
    let v_calc = ((val1 ^ !val2) & (val1 ^ res) & 0x80) != 0;
    let n = false;
    let c = res_wide > 0xFF;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_sub(val1: u8, val2: u8) -> (u8, u8) {
    let res = val1.wrapping_sub(val2);
    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let h = (val1 & 0x0F) < (val2 & 0x0F);
    let v_calc = ((val1 ^ val2) & (val1 ^ res) & 0x80) != 0;
    let n = true;
    let c = val2 > val1;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_sbc(val1: u8, val2: u8, carry_in: bool) -> (u8, u8) {
    let c_val = if carry_in { 1 } else { 0 };
    let res_wide = (val1 as i16) - (val2 as i16) - (c_val as i16);
    let res = res_wide as u8;
    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let h_calc = (val1 as i16 & 0x0F) - (val2 as i16 & 0x0F) - (c_val as i16);
    let h = h_calc < 0;
    let v_calc = ((val1 ^ val2) & (val1 ^ res) & 0x80) != 0;
    let n = true;
    let c = res_wide < 0;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_logic(op: u8, val1: u8, val2: u8) -> (u8, u8) {
    let (res, h) = match op {
        0 => (val1 & val2, true),
        1 => (val1 ^ val2, false),
        2 => (val1 | val2, false),
        _ => unreachable!(),
    };
    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let pv = res.count_ones() % 2 == 0;
    let n = false;
    let c = false;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if pv { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_inc(val: u8, flags_in: u8) -> (u8, u8) {
    let result = val.wrapping_add(1);
    let s = (result & 0x80) != 0;
    let z = result == 0;
    let y = (result & 0x20) != 0;
    let x = (result & 0x08) != 0;
    let h = (val & 0x0F) == 0x0F;
    let pv = val == 0x7F;
    let n = false;
    let c = (flags_in & flags::CARRY) != 0;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if pv { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (result, f)
}

fn ref_dec(val: u8, flags_in: u8) -> (u8, u8) {
    let result = val.wrapping_sub(1);
    let s = (result & 0x80) != 0;
    let z = result == 0;
    let y = (result & 0x20) != 0;
    let x = (result & 0x08) != 0;
    let h = (val & 0x0F) == 0x00;
    let pv = val == 0x80;
    let n = true;
    let c = (flags_in & flags::CARRY) != 0;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if pv { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (result, f)
}

fn ref_add16(val1: u16, val2: u16, flags_in: u8) -> (u16, u8) {
    let res = val1.wrapping_add(val2);
    let c = (val1 as u32 + val2 as u32) > 0xFFFF;
    let h = ((val1 & 0x0FFF) + (val2 & 0x0FFF)) > 0x0FFF;
    let n = false;
    let x = (res & 0x0800) != 0;
    let y = (res & 0x2000) != 0;
    let mut f = flags_in & (flags::SIGN | flags::ZERO | flags::PARITY);
    if c { f |= flags::CARRY; }
    if h { f |= flags::HALF_CARRY; }
    if n { f |= flags::ADD_SUB; }
    if x { f |= flags::X_FLAG; }
    if y { f |= flags::Y_FLAG; }
    (res, f)
}

fn ref_adc16(val1: u16, val2: u16, flags_in: u8) -> (u16, u8) {
    let c_in = if (flags_in & flags::CARRY) != 0 { 1 } else { 0 };
    let res_wide = val1 as u32 + val2 as u32 + c_in;
    let res = res_wide as u16;
    let s = (res & 0x8000) != 0;
    let z = res == 0;
    let x = (res & 0x0800) != 0;
    let y = (res & 0x2000) != 0;
    let h = ((val1 & 0x0FFF) + (val2 & 0x0FFF) + c_in as u16) > 0x0FFF;
    let v_calc = ((val1 ^ !val2) & (val1 ^ res) & 0x8000) != 0;
    let n = false;
    let c = res_wide > 0xFFFF;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_sbc16(val1: u16, val2: u16, flags_in: u8) -> (u16, u8) {
    let c_in = if (flags_in & flags::CARRY) != 0 { 1 } else { 0 };
    let res_wide = (val1 as i32) - (val2 as i32) - (c_in as i32);
    let res = res_wide as u16;
    let s = (res & 0x8000) != 0;
    let z = res == 0;
    let x = (res & 0x0800) != 0;
    let y = (res & 0x2000) != 0;
    let h_calc = (val1 as i32 & 0x0FFF) - (val2 as i32 & 0x0FFF) - (c_in as i32);
    let h = h_calc < 0;
    let v_calc = ((val1 ^ val2) & (val1 ^ res) & 0x8000) != 0;
    let n = true;
    let c = res_wide < 0;
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if v_calc { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if c { f |= flags::CARRY; }
    (res, f)
}

fn ref_shift(op_type: u8, val: u8, flags_in: u8) -> (u8, u8) {
    let old_c = (flags_in & flags::CARRY) != 0;
    let (res, new_c) = match op_type {
        0 => { // RLC
            let c = (val & 0x80) != 0;
            ((val << 1) | (if c { 1 } else { 0 }), c)
        }
        1 => { // RRC
            let c = (val & 0x01) != 0;
            ((val >> 1) | (if c { 0x80 } else { 0 }), c)
        }
        2 => { // RL
            let c = (val & 0x80) != 0;
            ((val << 1) | (if old_c { 1 } else { 0 }), c)
        }
        3 => { // RR
            let c = (val & 0x01) != 0;
            ((val >> 1) | (if old_c { 0x80 } else { 0 }), c)
        }
        4 => { // SLA
            let c = (val & 0x80) != 0;
            (val << 1, c)
        }
        5 => { // SRA
            let c = (val & 0x01) != 0;
            ((val as i8 >> 1) as u8, c)
        }
        7 => { // SRL
            let c = (val & 0x01) != 0;
            (val >> 1, c)
        }
        _ => (val, old_c), // Unhandled
    };

    let s = (res & 0x80) != 0;
    let z = res == 0;
    let y = (res & 0x20) != 0;
    let x = (res & 0x08) != 0;
    let pv = res.count_ones() % 2 == 0; // Parity
    let h = false;
    let n = false;
    
    let mut f = 0u8;
    if s { f |= flags::SIGN; }
    if z { f |= flags::ZERO; }
    if y { f |= flags::Y_FLAG; }
    if h { f |= flags::HALF_CARRY; }
    if x { f |= flags::X_FLAG; }
    if pv { f |= flags::PARITY; }
    if n { f |= flags::ADD_SUB; }
    if new_c { f |= flags::CARRY; }
    (res, f)
}

fn ref_bit(bit: u8, val: u8, flags_in: u8) -> u8 {
    let zero = (val & (1 << bit)) == 0;
    let x = (val & 0x08) != 0;
    let y = (val & 0x20) != 0;
    
    let mut f = flags_in & (flags::SIGN | flags::PARITY | flags::CARRY); // Preserve S, P/V, C
    if zero { f |= flags::ZERO; }
    f |= flags::HALF_CARRY;
    if x { f |= flags::X_FLAG; }
    if y { f |= flags::Y_FLAG; }
    f
}

// ============ TESTS ============

#[test]
fn exhaustive_8bit_arithmetic() {
    let mut rng = XorShift64::new(0x1234567890ABCDEF);
    let mut cpu = z80_setup();
    
    // ADD A, r
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = rng.next_u8();
        cpu.memory.write_byte(0 as u32, 0x80); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_add(a, b);
        assert_eq!(cpu.a, exp_res, "ADD Res iter {}", i);
        assert_eq!(cpu.f, exp_f, "ADD Flags iter {}", i);
    }
    
    // ADC
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        let f_init = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0x88); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_adc(a, b, (f_init & flags::CARRY) != 0);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "ADC Flags iter {}", i);
    }
    
    // SUB
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = rng.next_u8();
        cpu.memory.write_byte(0 as u32, 0x90); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_sub(a, b);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "SUB Flags iter {}", i);
    }
    
    // SBC
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        let f_init = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0x98); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_sbc(a, b, (f_init & flags::CARRY) != 0);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "SBC Flags iter {}", i);
    }
}

#[test]
fn exhaustive_logic() {
    let mut rng = XorShift64::new(0x9876543210FEDCBA);
    let mut cpu = z80_setup();
    // AND
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = rng.next_u8();
        cpu.memory.write_byte(0 as u32, 0xA0); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_logic(0, a, b);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "AND flags iter {}", i);
    }
    // XOR
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = rng.next_u8();
        cpu.memory.write_byte(0 as u32, 0xA8); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_logic(1, a, b);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "XOR flags iter {}", i);
    }
    // OR
    for i in 0..10000 {
        let a = rng.next_u8();
        let b = rng.next_u8();
        cpu.a = a; cpu.b = b; cpu.f = rng.next_u8();
        cpu.memory.write_byte(0 as u32, 0xB0); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_logic(2, a, b);
        assert_eq!(cpu.a, exp_res);
        assert_eq!(cpu.f, exp_f, "OR flags iter {}", i);
    }
}

#[test]
fn exhaustive_inc_dec() {
    let mut rng = XorShift64::new(0xABCDEF1234567890);
    let mut cpu = z80_setup();
    // INC B
    for i in 0..10000 {
        let b = rng.next_u8();
        let f_init = rng.next_u8();
        cpu.b = b; cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0x04); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_inc(b, f_init);
        assert_eq!(cpu.b, exp_res);
        assert_eq!(cpu.f, exp_f, "INC flags iter {}", i);
    }
    // DEC B
    for i in 0..10000 {
        let b = rng.next_u8();
        let f_init = rng.next_u8();
        cpu.b = b; cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0x05); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_dec(b, f_init);
        assert_eq!(cpu.b, exp_res);
        assert_eq!(cpu.f, exp_f, "DEC flags iter {}", i);
    }
}

#[test]
fn exhaustive_16bit_arithmetic() {
    let mut rng = XorShift64::new(0xDEADBEEFCAFEBABE);
    let mut cpu = z80_setup();
    // ADD HL, BC
    for i in 0..10000 {
        let hl = rng.next_u16();
        let bc = rng.next_u16();
        let f_init = rng.next_u8();
        cpu.set_hl(hl); cpu.set_bc(bc); cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0x09); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_add16(hl, bc, f_init);
        assert_eq!(cpu.hl(), exp_res);
        assert_eq!(cpu.f, exp_f, "ADD HL flags iter {}", i);
    }
    // ADC HL, BC
    for i in 0..10000 {
        let hl = rng.next_u16();
        let bc = rng.next_u16();
        let f_init = rng.next_u8();
        cpu.set_hl(hl); cpu.set_bc(bc); cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0xED); cpu.memory.write_byte(1 as u32, 0x4A); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_adc16(hl, bc, f_init);
        assert_eq!(cpu.hl(), exp_res);
        assert_eq!(cpu.f, exp_f, "ADC HL flags iter {}", i);
    }
    // SBC HL, BC
    for i in 0..10000 {
        let hl = rng.next_u16();
        let bc = rng.next_u16();
        let f_init = rng.next_u8();
        cpu.set_hl(hl); cpu.set_bc(bc); cpu.f = f_init;
        cpu.memory.write_byte(0 as u32, 0xED); cpu.memory.write_byte(1 as u32, 0x42); cpu.pc = 0;
        cpu.step();
        let (exp_res, exp_f) = ref_sbc16(hl, bc, f_init);
        assert_eq!(cpu.hl(), exp_res);
        assert_eq!(cpu.f, exp_f, "SBC HL flags iter {}", i);
    }
}

#[test]
fn exhaustive_shifts() {
    let mut rng = XorShift64::new(0x1122334455667788);
    let mut cpu = z80_setup();
    let types = [0, 1, 2, 3, 4, 5, 7];
    let type_names = ["RLC", "RRC", "RL", "RR", "SLA", "SRA", "SRL"];
    for (idx, &t) in types.iter().enumerate() {
        let opcode_byte = (t << 3) | 0x00; // Reg B
        for i in 0..10000 {
            let val = rng.next_u8();
            let f_init = rng.next_u8();
            cpu.b = val; cpu.f = f_init;
            cpu.memory.write_byte(0 as u32, 0xCB); cpu.memory.write_byte(1 as u32, opcode_byte); cpu.pc = 0;
            cpu.step();
            let (exp_res, exp_f) = ref_shift(t, val, f_init);
            assert_eq!(cpu.b, exp_res, "{} Res iter {}", type_names[idx], i);
            assert_eq!(cpu.f, exp_f, "{} Flags iter {}", type_names[idx], i);
        }
    }
}

#[test]
fn exhaustive_bit_register() {
    let mut rng = XorShift64::new(0x9988776655443322);
    let mut cpu = z80_setup();
    let registers = [0, 1, 2, 3, 4, 5, 7];
    let reg_names = ["B", "C", "D", "E", "H", "L", "A"];
    for (r_idx, &r) in registers.iter().enumerate() {
        for b in 0..8 {
            for i in 0..2000 {
                let val = rng.next_u8();
                match r {
                    0 => cpu.b = val, 1 => cpu.c = val, 2 => cpu.d = val, 3 => cpu.e = val,
                    4 => cpu.h = val, 5 => cpu.l = val, 7 => cpu.a = val, _ => {}
                }
                let f_init = rng.next_u8();
                cpu.f = f_init;
                let opcode = 0x40 | (b << 3) | r;
                cpu.memory.write_byte(0 as u32, 0xCB);
                cpu.memory.write_byte(1 as u32, opcode);
                cpu.pc = 0;
                cpu.step();
                let exp_f = ref_bit(b, val, f_init);
                assert_eq!(cpu.f, exp_f, "BIT {}, {} Flags iter {}", b, reg_names[r_idx], i);
            }
        }
    }
}
