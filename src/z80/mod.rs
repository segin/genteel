//! Z80 CPU Implementation for Genesis Sound Co-processor
//!
//! The Z80 is used as a sound co-processor in the Sega Genesis, running at 3.58 MHz.
//! It has access to 8KB of dedicated sound RAM and controls the YM2612 and SN76489.

use crate::memory::Memory;

/// Z80 Flag bits in the F register
pub mod flags {
    pub const CARRY: u8 = 0b0000_0001;      // C - Carry flag
    pub const ADD_SUB: u8 = 0b0000_0010;    // N - Add/Subtract flag
    pub const PARITY: u8 = 0b0000_0100;     // P/V - Parity/Overflow flag
    pub const HALF_CARRY: u8 = 0b0001_0000; // H - Half-carry flag
    pub const ZERO: u8 = 0b0100_0000;       // Z - Zero flag
    pub const SIGN: u8 = 0b1000_0000;       // S - Sign flag
}

/// Z80 CPU state
#[derive(Debug, Clone)]
pub struct Z80 {
    // Main registers
    pub a: u8,
    pub f: u8,
    pub b: u8,
    pub c: u8,
    pub d: u8,
    pub e: u8,
    pub h: u8,
    pub l: u8,

    // Alternate registers
    pub a_prime: u8,
    pub f_prime: u8,
    pub b_prime: u8,
    pub c_prime: u8,
    pub d_prime: u8,
    pub e_prime: u8,
    pub h_prime: u8,
    pub l_prime: u8,

    // Index registers
    pub ix: u16,
    pub iy: u16,

    // Stack Pointer and Program Counter
    pub sp: u16,
    pub pc: u16,

    // Interrupt Vector and Memory Refresh
    pub i: u8,
    pub r: u8,

    // Interrupt flip-flops
    pub iff1: bool,
    pub iff2: bool,
    
    // Interrupt mode (0, 1, or 2)
    pub im: u8,

    // Halted state
    pub halted: bool,

    // Memory
    pub memory: Memory,

    // Cycle counter for timing
    pub cycles: u64,
}

impl Z80 {
    pub fn new(memory: Memory) -> Self {
        Self {
            a: 0xFF, f: 0xFF, b: 0, c: 0, d: 0, e: 0, h: 0, l: 0,
            a_prime: 0, f_prime: 0, b_prime: 0, c_prime: 0, d_prime: 0, e_prime: 0, h_prime: 0, l_prime: 0,
            ix: 0, iy: 0,
            sp: 0xFFFF, pc: 0,
            i: 0, r: 0,
            iff1: false, iff2: false,
            im: 0,
            halted: false,
            memory,
            cycles: 0,
        }
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.a = 0xFF;
        self.f = 0xFF;
        self.pc = 0;
        self.sp = 0xFFFF;
        self.i = 0;
        self.r = 0;
        self.iff1 = false;
        self.iff2 = false;
        self.im = 0;
        self.halted = false;
    }

    // ========== Register pair getters/setters ==========
    
    pub fn af(&self) -> u16 {
        (self.a as u16) << 8 | self.f as u16
    }

    pub fn set_af(&mut self, val: u16) {
        self.a = (val >> 8) as u8;
        self.f = val as u8;
    }

    pub fn bc(&self) -> u16 {
        (self.b as u16) << 8 | self.c as u16
    }

    pub fn set_bc(&mut self, val: u16) {
        self.b = (val >> 8) as u8;
        self.c = val as u8;
    }

    pub fn de(&self) -> u16 {
        (self.d as u16) << 8 | self.e as u16
    }

    pub fn set_de(&mut self, val: u16) {
        self.d = (val >> 8) as u8;
        self.e = val as u8;
    }

    pub fn hl(&self) -> u16 {
        (self.h as u16) << 8 | self.l as u16
    }

    pub fn set_hl(&mut self, val: u16) {
        self.h = (val >> 8) as u8;
        self.l = val as u8;
    }

    // ========== Flag helpers ==========

    pub fn get_flag(&self, flag: u8) -> bool {
        (self.f & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u8, value: bool) {
        if value {
            self.f |= flag;
        } else {
            self.f &= !flag;
        }
    }

    fn set_sz_flags(&mut self, value: u8) {
        self.set_flag(flags::ZERO, value == 0);
        self.set_flag(flags::SIGN, (value & 0x80) != 0);
    }

    fn set_parity_flag(&mut self, value: u8) {
        let parity = value.count_ones() % 2 == 0;
        self.set_flag(flags::PARITY, parity);
    }

    // ========== Memory access helpers ==========

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory.read_byte(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);
        byte
    }

    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    fn read_byte(&self, addr: u16) -> u8 {
        self.memory.read_byte(addr as u32)
    }

    fn write_byte(&mut self, addr: u16, value: u8) {
        self.memory.write_byte(addr as u32, value);
    }

    fn read_word(&self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, value as u8);
        self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
    }

    fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, value);
    }

    fn pop(&mut self) -> u16 {
        let value = self.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // ========== ALU operations ==========

    fn add_a(&mut self, value: u8, with_carry: bool) {
        let carry = if with_carry && self.get_flag(flags::CARRY) { 1u16 } else { 0 };
        let a = self.a as u16;
        let v = value as u16;
        let result = a + v + carry;
        
        let half_carry = ((a & 0x0F) + (v & 0x0F) + carry) > 0x0F;
        let overflow = ((a ^ result) & (v ^ result) & 0x80) != 0;
        
        self.a = result as u8;
        self.set_flag(flags::CARRY, result > 0xFF);
        self.set_flag(flags::HALF_CARRY, half_carry);
        self.set_flag(flags::PARITY, overflow);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(self.a);
    }

    fn sub_a(&mut self, value: u8, with_carry: bool, store: bool) {
        let carry = if with_carry && self.get_flag(flags::CARRY) { 1u16 } else { 0 };
        let a = self.a as u16;
        let v = value as u16;
        let result = a.wrapping_sub(v).wrapping_sub(carry);
        
        let half_carry = (a & 0x0F) < (v & 0x0F) + carry;
        let overflow = ((a ^ v) & (a ^ result) & 0x80) != 0;
        
        self.set_flag(flags::CARRY, result > 0xFF);
        self.set_flag(flags::HALF_CARRY, half_carry);
        self.set_flag(flags::PARITY, overflow);
        self.set_flag(flags::ADD_SUB, true);
        
        if store {
            self.a = result as u8;
        }
        self.set_sz_flags(result as u8);
    }

    fn and_a(&mut self, value: u8) {
        self.a &= value;
        self.set_flag(flags::CARRY, false);
        self.set_flag(flags::HALF_CARRY, true);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(self.a);
        self.set_parity_flag(self.a);
    }

    fn or_a(&mut self, value: u8) {
        self.a |= value;
        self.set_flag(flags::CARRY, false);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(self.a);
        self.set_parity_flag(self.a);
    }

    fn xor_a(&mut self, value: u8) {
        self.a ^= value;
        self.set_flag(flags::CARRY, false);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(self.a);
        self.set_parity_flag(self.a);
    }

    fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);
        self.set_flag(flags::HALF_CARRY, (value & 0x0F) == 0x0F);
        self.set_flag(flags::PARITY, value == 0x7F);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(result);
        result
    }

    fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);
        self.set_flag(flags::HALF_CARRY, (value & 0x0F) == 0x00);
        self.set_flag(flags::PARITY, value == 0x80);
        self.set_flag(flags::ADD_SUB, true);
        self.set_sz_flags(result);
        result
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.hl() as u32;
        let v = value as u32;
        let result = hl + v;
        
        self.set_flag(flags::CARRY, result > 0xFFFF);
        self.set_flag(flags::HALF_CARRY, ((hl & 0x0FFF) + (v & 0x0FFF)) > 0x0FFF);
        self.set_flag(flags::ADD_SUB, false);
        
        self.set_hl(result as u16);
    }

    // ========== Rotate/Shift operations ==========

    fn rlca(&mut self) {
        let carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if carry { 1 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
    }

    fn rrca(&mut self) {
        let carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if carry { 0x80 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
    }

    fn rla(&mut self) {
        let old_carry = self.get_flag(flags::CARRY);
        let new_carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if old_carry { 1 } else { 0 };
        self.set_flag(flags::CARRY, new_carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
    }

    fn rra(&mut self) {
        let old_carry = self.get_flag(flags::CARRY);
        let new_carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if old_carry { 0x80 } else { 0 };
        self.set_flag(flags::CARRY, new_carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
    }

    // ========== Helper to get/set register by index ==========

    fn get_reg(&self, index: u8) -> u8 {
        match index {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => self.h,
            5 => self.l,
            6 => self.read_byte(self.hl()),
            7 => self.a,
            _ => 0,
        }
    }

    fn set_reg(&mut self, index: u8, value: u8) {
        match index {
            0 => self.b = value,
            1 => self.c = value,
            2 => self.d = value,
            3 => self.e = value,
            4 => self.h = value,
            5 => self.l = value,
            6 => {
                let addr = self.hl();
                self.write_byte(addr, value);
            }
            7 => self.a = value,
            _ => {}
        }
    }

    fn get_rp(&self, index: u8) -> u16 {
        match index {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            3 => self.sp,
            _ => 0,
        }
    }

    fn set_rp(&mut self, index: u8, value: u16) {
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.sp = value,
            _ => {}
        }
    }

    fn get_rp2(&self, index: u8) -> u16 {
        match index {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            3 => self.af(),
            _ => 0,
        }
    }

    fn set_rp2(&mut self, index: u8, value: u16) {
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.set_af(value),
            _ => {}
        }
    }

    fn check_condition(&self, cc: u8) -> bool {
        match cc {
            0 => !self.get_flag(flags::ZERO),      // NZ
            1 => self.get_flag(flags::ZERO),       // Z
            2 => !self.get_flag(flags::CARRY),     // NC
            3 => self.get_flag(flags::CARRY),      // C
            4 => !self.get_flag(flags::PARITY),    // PO
            5 => self.get_flag(flags::PARITY),     // PE
            6 => !self.get_flag(flags::SIGN),      // P
            7 => self.get_flag(flags::SIGN),       // M
            _ => false,
        }
    }

    // ========== Main execution ==========

    /// Execute one instruction, returns number of T-states used
    pub fn step(&mut self) -> u8 {
        if self.halted {
            return 4; // HALT uses 4 T-states per cycle
        }

        // Increment R register (lower 7 bits only)
        self.r = (self.r & 0x80) | ((self.r.wrapping_add(1)) & 0x7F);

        let opcode = self.fetch_byte();
        
        // Decode using the standard Z80 opcode structure
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = (y >> 1) & 0x03;
        let q = y & 0x01;

        match x {
            0 => self.execute_x0(opcode, y, z, p, q),
            1 => self.execute_x1(y, z),
            2 => self.execute_x2(y, z),
            3 => self.execute_x3(opcode, y, z, p, q),
            _ => 4,
        }
    }

    fn execute_x0(&mut self, _opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8 {
        match z {
            0 => match y {
                0 => 4, // NOP
                1 => { // EX AF, AF'
                    std::mem::swap(&mut self.a, &mut self.a_prime);
                    std::mem::swap(&mut self.f, &mut self.f_prime);
                    4
                }
                2 => { // DJNZ d
                    let d = self.fetch_byte() as i8;
                    self.b = self.b.wrapping_sub(1);
                    if self.b != 0 {
                        self.pc = (self.pc as i16 + d as i16) as u16;
                        13
                    } else {
                        8
                    }
                }
                3 => { // JR d
                    let d = self.fetch_byte() as i8;
                    self.pc = (self.pc as i16 + d as i16) as u16;
                    12
                }
                4..=7 => { // JR cc, d
                    let d = self.fetch_byte() as i8;
                    if self.check_condition(y - 4) {
                        self.pc = (self.pc as i16 + d as i16) as u16;
                        12
                    } else {
                        7
                    }
                }
                _ => 4,
            },
            1 => if q == 0 {
                // LD rp, nn
                let nn = self.fetch_word();
                self.set_rp(p, nn);
                10
            } else {
                // ADD HL, rp
                let rp = self.get_rp(p);
                self.add_hl(rp);
                11
            },
            2 => match (p, q) {
                (0, 0) => { // LD (BC), A
                    let addr = self.bc();
                    self.write_byte(addr, self.a);
                    7
                }
                (0, 1) => { // LD A, (BC)
                    let addr = self.bc();
                    self.a = self.read_byte(addr);
                    7
                }
                (1, 0) => { // LD (DE), A
                    let addr = self.de();
                    self.write_byte(addr, self.a);
                    7
                }
                (1, 1) => { // LD A, (DE)
                    let addr = self.de();
                    self.a = self.read_byte(addr);
                    7
                }
                (2, 0) => { // LD (nn), HL
                    let addr = self.fetch_word();
                    self.write_word(addr, self.hl());
                    16
                }
                (2, 1) => { // LD HL, (nn)
                    let addr = self.fetch_word();
                    let val = self.read_word(addr);
                    self.set_hl(val);
                    16
                }
                (3, 0) => { // LD (nn), A
                    let addr = self.fetch_word();
                    self.write_byte(addr, self.a);
                    13
                }
                (3, 1) => { // LD A, (nn)
                    let addr = self.fetch_word();
                    self.a = self.read_byte(addr);
                    13
                }
                _ => 4,
            },
            3 => {
                // INC/DEC rp
                let rp = self.get_rp(p);
                if q == 0 {
                    self.set_rp(p, rp.wrapping_add(1));
                } else {
                    self.set_rp(p, rp.wrapping_sub(1));
                }
                6
            }
            4 => { // INC r
                let val = self.get_reg(y);
                let result = self.inc(val);
                self.set_reg(y, result);
                if y == 6 { 11 } else { 4 }
            }
            5 => { // DEC r
                let val = self.get_reg(y);
                let result = self.dec(val);
                self.set_reg(y, result);
                if y == 6 { 11 } else { 4 }
            }
            6 => { // LD r, n
                let n = self.fetch_byte();
                self.set_reg(y, n);
                if y == 6 { 10 } else { 7 }
            }
            7 => match y {
                0 => { self.rlca(); 4 }
                1 => { self.rrca(); 4 }
                2 => { self.rla(); 4 }
                3 => { self.rra(); 4 }
                4 => { // DAA
                    let mut a = self.a as u16;
                    if self.get_flag(flags::ADD_SUB) {
                        if self.get_flag(flags::HALF_CARRY) { a = a.wrapping_sub(0x06) & 0xFF; }
                        if self.get_flag(flags::CARRY) { a = a.wrapping_sub(0x60); }
                    } else {
                        if self.get_flag(flags::HALF_CARRY) || (a & 0x0F) > 9 { a = a.wrapping_add(0x06); }
                        if self.get_flag(flags::CARRY) || a > 0x9F { a = a.wrapping_add(0x60); }
                    }
                    self.set_flag(flags::HALF_CARRY, false);
                    if a > 0xFF { self.set_flag(flags::CARRY, true); }
                    self.a = a as u8;
                    self.set_sz_flags(self.a);
                    self.set_parity_flag(self.a);
                    4
                }
                5 => { // CPL
                    self.a = !self.a;
                    self.set_flag(flags::HALF_CARRY, true);
                    self.set_flag(flags::ADD_SUB, true);
                    4
                }
                6 => { // SCF
                    self.set_flag(flags::CARRY, true);
                    self.set_flag(flags::HALF_CARRY, false);
                    self.set_flag(flags::ADD_SUB, false);
                    4
                }
                7 => { // CCF
                    let c = self.get_flag(flags::CARRY);
                    self.set_flag(flags::HALF_CARRY, c);
                    self.set_flag(flags::CARRY, !c);
                    self.set_flag(flags::ADD_SUB, false);
                    4
                }
                _ => 4,
            },
            _ => 4,
        }
    }

    fn execute_x1(&mut self, y: u8, z: u8) -> u8 {
        if y == 6 && z == 6 {
            // HALT
            self.halted = true;
            4
        } else {
            // LD r, r'
            let val = self.get_reg(z);
            self.set_reg(y, val);
            if y == 6 || z == 6 { 7 } else { 4 }
        }
    }

    fn execute_x2(&mut self, y: u8, z: u8) -> u8 {
        // ALU operations
        let val = self.get_reg(z);
        match y {
            0 => self.add_a(val, false),    // ADD A, r
            1 => self.add_a(val, true),     // ADC A, r
            2 => self.sub_a(val, false, true),  // SUB r
            3 => self.sub_a(val, true, true),   // SBC A, r
            4 => self.and_a(val),           // AND r
            5 => self.xor_a(val),           // XOR r
            6 => self.or_a(val),            // OR r
            7 => self.sub_a(val, false, false), // CP r
            _ => {}
        }
        if z == 6 { 7 } else { 4 }
    }

    fn execute_x3(&mut self, _opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8 {
        match z {
            0 => { // RET cc
                if self.check_condition(y) {
                    self.pc = self.pop();
                    11
                } else {
                    5
                }
            }
            1 => if q == 0 {
                // POP rp2
                let val = self.pop();
                self.set_rp2(p, val);
                10
            } else {
                match p {
                    0 => { // RET
                        self.pc = self.pop();
                        10
                    }
                    1 => { // EXX
                        std::mem::swap(&mut self.b, &mut self.b_prime);
                        std::mem::swap(&mut self.c, &mut self.c_prime);
                        std::mem::swap(&mut self.d, &mut self.d_prime);
                        std::mem::swap(&mut self.e, &mut self.e_prime);
                        std::mem::swap(&mut self.h, &mut self.h_prime);
                        std::mem::swap(&mut self.l, &mut self.l_prime);
                        4
                    }
                    2 => { // JP HL
                        self.pc = self.hl();
                        4
                    }
                    3 => { // LD SP, HL
                        self.sp = self.hl();
                        6
                    }
                    _ => 4,
                }
            },
            2 => { // JP cc, nn
                let nn = self.fetch_word();
                if self.check_condition(y) {
                    self.pc = nn;
                }
                10
            }
            3 => match y {
                0 => { // JP nn
                    self.pc = self.fetch_word();
                    10
                }
                1 => self.execute_cb_prefix(),
                2 => { // OUT (n), A
                    let _port = self.fetch_byte();
                    // TODO: I/O implementation
                    11
                }
                3 => { // IN A, (n)
                    let _port = self.fetch_byte();
                    // TODO: I/O implementation
                    self.a = 0xFF;
                    11
                }
                4 => { // EX (SP), HL
                    let val = self.read_word(self.sp);
                    self.write_word(self.sp, self.hl());
                    self.set_hl(val);
                    19
                }
                5 => { // EX DE, HL
                    let de = self.de();
                    let hl = self.hl();
                    self.set_de(hl);
                    self.set_hl(de);
                    4
                }
                6 => { // DI
                    self.iff1 = false;
                    self.iff2 = false;
                    4
                }
                7 => { // EI
                    self.iff1 = true;
                    self.iff2 = true;
                    4
                }
                _ => 4,
            },
            4 => { // CALL cc, nn
                let nn = self.fetch_word();
                if self.check_condition(y) {
                    self.push(self.pc);
                    self.pc = nn;
                    17
                } else {
                    10
                }
            }
            5 => if q == 0 {
                // PUSH rp2
                let val = self.get_rp2(p);
                self.push(val);
                11
            } else {
                match p {
                    0 => { // CALL nn
                        let nn = self.fetch_word();
                        self.push(self.pc);
                        self.pc = nn;
                        17
                    }
                    1 => self.execute_dd_prefix(),
                    2 => self.execute_ed_prefix(),
                    3 => self.execute_fd_prefix(),
                    _ => 4,
                }
            },
            6 => { // ALU A, n
                let n = self.fetch_byte();
                match y {
                    0 => self.add_a(n, false),
                    1 => self.add_a(n, true),
                    2 => self.sub_a(n, false, true),
                    3 => self.sub_a(n, true, true),
                    4 => self.and_a(n),
                    5 => self.xor_a(n),
                    6 => self.or_a(n),
                    7 => self.sub_a(n, false, false),
                    _ => {}
                }
                7
            }
            7 => { // RST y*8
                self.push(self.pc);
                self.pc = (y as u16) * 8;
                11
            }
            _ => 4,
        }
    }

    // ========== CB Prefix (Bit operations) ==========

    fn execute_cb_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;

        match x {
            0 => { // Rotate/shift
                let val = self.get_reg(z);
                let result = match y {
                    0 => { // RLC
                        let carry = (val & 0x80) != 0;
                        let r = (val << 1) | if carry { 1 } else { 0 };
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    1 => { // RRC
                        let carry = (val & 0x01) != 0;
                        let r = (val >> 1) | if carry { 0x80 } else { 0 };
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    2 => { // RL
                        let old_carry = self.get_flag(flags::CARRY);
                        let carry = (val & 0x80) != 0;
                        let r = (val << 1) | if old_carry { 1 } else { 0 };
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    3 => { // RR
                        let old_carry = self.get_flag(flags::CARRY);
                        let carry = (val & 0x01) != 0;
                        let r = (val >> 1) | if old_carry { 0x80 } else { 0 };
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    4 => { // SLA
                        let carry = (val & 0x80) != 0;
                        let r = val << 1;
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    5 => { // SRA
                        let carry = (val & 0x01) != 0;
                        let r = (val >> 1) | (val & 0x80);
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    6 => { // SLL (undocumented)
                        let carry = (val & 0x80) != 0;
                        let r = (val << 1) | 1;
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    7 => { // SRL
                        let carry = (val & 0x01) != 0;
                        let r = val >> 1;
                        self.set_flag(flags::CARRY, carry);
                        r
                    }
                    _ => val,
                };
                self.set_flag(flags::HALF_CARRY, false);
                self.set_flag(flags::ADD_SUB, false);
                self.set_sz_flags(result);
                self.set_parity_flag(result);
                self.set_reg(z, result);
                if z == 6 { 15 } else { 8 }
            }
            1 => { // BIT y, r
                let val = self.get_reg(z);
                let bit = (val >> y) & 1;
                self.set_flag(flags::ZERO, bit == 0);
                self.set_flag(flags::HALF_CARRY, true);
                self.set_flag(flags::ADD_SUB, false);
                if z == 6 { 12 } else { 8 }
            }
            2 => { // RES y, r
                let val = self.get_reg(z);
                let result = val & !(1 << y);
                self.set_reg(z, result);
                if z == 6 { 15 } else { 8 }
            }
            3 => { // SET y, r
                let val = self.get_reg(z);
                let result = val | (1 << y);
                self.set_reg(z, result);
                if z == 6 { 15 } else { 8 }
            }
            _ => 8,
        }
    }

    // ========== ED Prefix (Extended) ==========

    fn execute_ed_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = (y >> 1) & 0x03;
        let q = y & 0x01;

        match x {
            1 => match z {
                0 => { // IN r, (C)
                    // TODO: I/O implementation
                    if y != 6 {
                        self.set_reg(y, 0xFF);
                    }
                    12
                }
                1 => { // OUT (C), r
                    // TODO: I/O implementation
                    12
                }
                2 => if q == 0 {
                    // SBC HL, rp
                    let hl = self.hl() as u32;
                    let rp = self.get_rp(p) as u32;
                    let c = if self.get_flag(flags::CARRY) { 1u32 } else { 0 };
                    let result = hl.wrapping_sub(rp).wrapping_sub(c);
                    self.set_flag(flags::CARRY, result > 0xFFFF);
                    self.set_flag(flags::ADD_SUB, true);
                    self.set_flag(flags::ZERO, (result & 0xFFFF) == 0);
                    self.set_flag(flags::SIGN, (result & 0x8000) != 0);
                    self.set_hl(result as u16);
                    15
                } else {
                    // ADC HL, rp
                    let hl = self.hl() as u32;
                    let rp = self.get_rp(p) as u32;
                    let c = if self.get_flag(flags::CARRY) { 1u32 } else { 0 };
                    let result = hl + rp + c;
                    self.set_flag(flags::CARRY, result > 0xFFFF);
                    self.set_flag(flags::ADD_SUB, false);
                    self.set_flag(flags::ZERO, (result & 0xFFFF) == 0);
                    self.set_flag(flags::SIGN, (result & 0x8000) != 0);
                    self.set_hl(result as u16);
                    15
                },
                3 => {
                    let nn = self.fetch_word();
                    if q == 0 {
                        // LD (nn), rp
                        self.write_word(nn, self.get_rp(p));
                    } else {
                        // LD rp, (nn)
                        let val = self.read_word(nn);
                        self.set_rp(p, val);
                    }
                    20
                }
                4 => { // NEG
                    let a = self.a;
                    self.a = 0;
                    self.sub_a(a, false, true);
                    8
                }
                5 => if q == 0 {
                    // RETN
                    self.iff1 = self.iff2;
                    self.pc = self.pop();
                    14
                } else {
                    // RETI
                    self.pc = self.pop();
                    14
                },
                6 => { // IM y
                    self.im = match y & 0x03 {
                        0 | 1 => 0,
                        2 => 1,
                        3 => 2,
                        _ => 0,
                    };
                    8
                }
                7 => match y {
                    0 => { // LD I, A
                        self.i = self.a;
                        9
                    }
                    1 => { // LD R, A
                        self.r = self.a;
                        9
                    }
                    2 => { // LD A, I
                        self.a = self.i;
                        self.set_sz_flags(self.a);
                        self.set_flag(flags::PARITY, self.iff2);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        9
                    }
                    3 => { // LD A, R
                        self.a = self.r;
                        self.set_sz_flags(self.a);
                        self.set_flag(flags::PARITY, self.iff2);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        9
                    }
                    4 => { // RRD
                        let hl = self.hl();
                        let m = self.read_byte(hl);
                        let new_m = (self.a << 4) | (m >> 4);
                        self.a = (self.a & 0xF0) | (m & 0x0F);
                        self.write_byte(hl, new_m);
                        self.set_sz_flags(self.a);
                        self.set_parity_flag(self.a);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        18
                    }
                    5 => { // RLD
                        let hl = self.hl();
                        let m = self.read_byte(hl);
                        let new_m = (m << 4) | (self.a & 0x0F);
                        self.a = (self.a & 0xF0) | (m >> 4);
                        self.write_byte(hl, new_m);
                        self.set_sz_flags(self.a);
                        self.set_parity_flag(self.a);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        18
                    }
                    _ => 8,
                },
                _ => 8,
            },
            2 => {
                // Block instructions
                if y >= 4 {
                    match z {
                        0 => self.execute_ldi_ldd(y),
                        1 => self.execute_cpi_cpd(y),
                        2 => self.execute_ini_ind(y),
                        3 => self.execute_outi_outd(y),
                        _ => 8,
                    }
                } else {
                    8 // Invalid
                }
            }
            _ => 8, // NONI / NOP
        }
    }

    fn execute_ldi_ldd(&mut self, y: u8) -> u8 {
        let hl = self.hl();
        let de = self.de();
        let val = self.read_byte(hl);
        self.write_byte(de, val);
        
        let bc = self.bc().wrapping_sub(1);
        self.set_bc(bc);
        
        let (new_hl, new_de) = if (y & 1) == 0 {
            (hl.wrapping_add(1), de.wrapping_add(1)) // LDI
        } else {
            (hl.wrapping_sub(1), de.wrapping_sub(1)) // LDD
        };
        
        self.set_hl(new_hl);
        self.set_de(new_de);
        
        self.set_flag(flags::PARITY, bc != 0);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        
        // LDIR/LDDR
        if y >= 6 && bc != 0 {
            self.pc = self.pc.wrapping_sub(2);
            21
        } else {
            16
        }
    }

    fn execute_cpi_cpd(&mut self, y: u8) -> u8 {
        let hl = self.hl();
        let val = self.read_byte(hl);
        let result = self.a.wrapping_sub(val);
        
        let bc = self.bc().wrapping_sub(1);
        self.set_bc(bc);
        
        let new_hl = if (y & 1) == 0 {
            hl.wrapping_add(1) // CPI
        } else {
            hl.wrapping_sub(1) // CPD
        };
        
        self.set_hl(new_hl);
        
        self.set_flag(flags::ZERO, result == 0);
        self.set_flag(flags::SIGN, (result & 0x80) != 0);
        self.set_flag(flags::HALF_CARRY, (self.a & 0x0F) < (val & 0x0F));
        self.set_flag(flags::PARITY, bc != 0);
        self.set_flag(flags::ADD_SUB, true);
        
        // CPIR/CPDR
        if y >= 6 && bc != 0 && result != 0 {
            self.pc = self.pc.wrapping_sub(2);
            21
        } else {
            16
        }
    }

    fn execute_ini_ind(&mut self, _y: u8) -> u8 {
        // TODO: I/O implementation
        16
    }

    fn execute_outi_outd(&mut self, _y: u8) -> u8 {
        // TODO: I/O implementation
        16
    }

    // ========== DD Prefix (IX) ==========

    fn execute_dd_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        
        match opcode {
            0x21 => { // LD IX, nn
                self.ix = self.fetch_word();
                14
            }
            0x22 => { // LD (nn), IX
                let addr = self.fetch_word();
                self.write_word(addr, self.ix);
                20
            }
            0x23 => { // INC IX
                self.ix = self.ix.wrapping_add(1);
                10
            }
            0x2A => { // LD IX, (nn)
                let addr = self.fetch_word();
                self.ix = self.read_word(addr);
                20
            }
            0x2B => { // DEC IX
                self.ix = self.ix.wrapping_sub(1);
                10
            }
            0x34 => { // INC (IX+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.ix as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let result = self.inc(val);
                self.write_byte(addr, result);
                23
            }
            0x35 => { // DEC (IX+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.ix as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let result = self.dec(val);
                self.write_byte(addr, result);
                23
            }
            0x36 => { // LD (IX+d), n
                let d = self.fetch_byte() as i8;
                let n = self.fetch_byte();
                let addr = (self.ix as i16 + d as i16) as u16;
                self.write_byte(addr, n);
                19
            }
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
                // LD r, (IX+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.ix as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let r = (opcode >> 3) & 0x07;
                self.set_reg(r, val);
                19
            }
            0x70..=0x77 => {
                // LD (IX+d), r
                let d = self.fetch_byte() as i8;
                let addr = (self.ix as i16 + d as i16) as u16;
                let r = opcode & 0x07;
                let val = self.get_reg(r);
                self.write_byte(addr, val);
                19
            }
            0xE1 => { // POP IX
                self.ix = self.pop();
                14
            }
            0xE3 => { // EX (SP), IX
                let val = self.read_word(self.sp);
                self.write_word(self.sp, self.ix);
                self.ix = val;
                23
            }
            0xE5 => { // PUSH IX
                self.push(self.ix);
                15
            }
            0xE9 => { // JP (IX)
                self.pc = self.ix;
                8
            }
            0xF9 => { // LD SP, IX
                self.sp = self.ix;
                10
            }
            0xCB => self.execute_ddcb_prefix(),
            _ => 8, // Treat as NOP
        }
    }

    // ========== FD Prefix (IY) ==========

    fn execute_fd_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        
        match opcode {
            0x21 => { // LD IY, nn
                self.iy = self.fetch_word();
                14
            }
            0x22 => { // LD (nn), IY
                let addr = self.fetch_word();
                self.write_word(addr, self.iy);
                20
            }
            0x23 => { // INC IY
                self.iy = self.iy.wrapping_add(1);
                10
            }
            0x2A => { // LD IY, (nn)
                let addr = self.fetch_word();
                self.iy = self.read_word(addr);
                20
            }
            0x2B => { // DEC IY
                self.iy = self.iy.wrapping_sub(1);
                10
            }
            0x34 => { // INC (IY+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.iy as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let result = self.inc(val);
                self.write_byte(addr, result);
                23
            }
            0x35 => { // DEC (IY+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.iy as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let result = self.dec(val);
                self.write_byte(addr, result);
                23
            }
            0x36 => { // LD (IY+d), n
                let d = self.fetch_byte() as i8;
                let n = self.fetch_byte();
                let addr = (self.iy as i16 + d as i16) as u16;
                self.write_byte(addr, n);
                19
            }
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
                // LD r, (IY+d)
                let d = self.fetch_byte() as i8;
                let addr = (self.iy as i16 + d as i16) as u16;
                let val = self.read_byte(addr);
                let r = (opcode >> 3) & 0x07;
                self.set_reg(r, val);
                19
            }
            0x70..=0x77 => {
                // LD (IY+d), r
                let d = self.fetch_byte() as i8;
                let addr = (self.iy as i16 + d as i16) as u16;
                let r = opcode & 0x07;
                let val = self.get_reg(r);
                self.write_byte(addr, val);
                19
            }
            0xE1 => { // POP IY
                self.iy = self.pop();
                14
            }
            0xE3 => { // EX (SP), IY
                let val = self.read_word(self.sp);
                self.write_word(self.sp, self.iy);
                self.iy = val;
                23
            }
            0xE5 => { // PUSH IY
                self.push(self.iy);
                15
            }
            0xE9 => { // JP (IY)
                self.pc = self.iy;
                8
            }
            0xF9 => { // LD SP, IY
                self.sp = self.iy;
                10
            }
            0xCB => self.execute_fdcb_prefix(),
            _ => 8, // Treat as NOP
        }
    }

    fn execute_ddcb_prefix(&mut self) -> u8 {
        let d = self.fetch_byte() as i8;
        let opcode = self.fetch_byte();
        let addr = (self.ix as i16 + d as i16) as u16;
        self.execute_indexed_cb(opcode, addr)
    }

    fn execute_fdcb_prefix(&mut self) -> u8 {
        let d = self.fetch_byte() as i8;
        let opcode = self.fetch_byte();
        let addr = (self.iy as i16 + d as i16) as u16;
        self.execute_indexed_cb(opcode, addr)
    }

    fn execute_indexed_cb(&mut self, opcode: u8, addr: u16) -> u8 {
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let val = self.read_byte(addr);

        match x {
            0 => { // Rotate/shift
                let result = match y {
                    0 => { let c = (val & 0x80) != 0; self.set_flag(flags::CARRY, c); (val << 1) | if c { 1 } else { 0 } }
                    1 => { let c = (val & 0x01) != 0; self.set_flag(flags::CARRY, c); (val >> 1) | if c { 0x80 } else { 0 } }
                    2 => { let oc = self.get_flag(flags::CARRY); let c = (val & 0x80) != 0; self.set_flag(flags::CARRY, c); (val << 1) | if oc { 1 } else { 0 } }
                    3 => { let oc = self.get_flag(flags::CARRY); let c = (val & 0x01) != 0; self.set_flag(flags::CARRY, c); (val >> 1) | if oc { 0x80 } else { 0 } }
                    4 => { let c = (val & 0x80) != 0; self.set_flag(flags::CARRY, c); val << 1 }
                    5 => { let c = (val & 0x01) != 0; self.set_flag(flags::CARRY, c); (val >> 1) | (val & 0x80) }
                    6 => { let c = (val & 0x80) != 0; self.set_flag(flags::CARRY, c); (val << 1) | 1 }
                    7 => { let c = (val & 0x01) != 0; self.set_flag(flags::CARRY, c); val >> 1 }
                    _ => val,
                };
                self.set_flag(flags::HALF_CARRY, false);
                self.set_flag(flags::ADD_SUB, false);
                self.set_sz_flags(result);
                self.set_parity_flag(result);
                self.write_byte(addr, result);
                if z != 6 { self.set_reg(z, result); }
                23
            }
            1 => { // BIT y, (IX/IY+d)
                let bit = (val >> y) & 1;
                self.set_flag(flags::ZERO, bit == 0);
                self.set_flag(flags::HALF_CARRY, true);
                self.set_flag(flags::ADD_SUB, false);
                20
            }
            2 => { // RES y, (IX/IY+d)
                let result = val & !(1 << y);
                self.write_byte(addr, result);
                if z != 6 { self.set_reg(z, result); }
                23
            }
            3 => { // SET y, (IX/IY+d)
                let result = val | (1 << y);
                self.write_byte(addr, result);
                if z != 6 { self.set_reg(z, result); }
                23
            }
            _ => 23,
        }
    }
}

#[cfg(test)]
mod tests;

#[cfg(test)]
mod proptest_tests;
