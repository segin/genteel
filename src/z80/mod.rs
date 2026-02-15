#![allow(unused_imports)]
//! Z80 CPU Implementation for Genesis Sound Co-processor
//!
//! The Z80 is used as a sound co-processor in the Sega Genesis, running at 3.58 MHz.
//! It has access to 8KB of dedicated sound RAM and controls the YM2612 and SN76489.

use crate::memory::{IoInterface, MemoryInterface};

/// Z80 Flag bits in the F register
pub mod flags {
    pub const CARRY: u8 = 0b0000_0001; // C - Carry flag
    pub const ADD_SUB: u8 = 0b0000_0010; // N - Add/Subtract flag
    pub const PARITY: u8 = 0b0000_0100; // P/V - Parity/Overflow flag
    pub const X_FLAG: u8 = 0b0000_1000; // X - Undocumented (copy of bit 3)
    pub const HALF_CARRY: u8 = 0b0001_0000; // H - Half-carry flag
    pub const Y_FLAG: u8 = 0b0010_0000; // Y - Undocumented (copy of bit 5)
    pub const ZERO: u8 = 0b0100_0000; // Z - Zero flag
    pub const SIGN: u8 = 0b1000_0000; // S - Sign flag
}

use crate::debugger::Debuggable;
use serde_json::{json, Value};

/// Z80 CPU
#[derive(Debug)]
pub struct Z80<M: MemoryInterface, I: IoInterface> {
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

    // Internal hidden register (WZ/MEMPTR)
    pub memptr: u16,

    // Halted state
    pub halted: bool,

    // Interrupt logic
    pub pending_ei: bool,

    // Memory interface
    pub memory: M,

    // I/O interface
    pub io: I,

    // Cycle counter for timing
    pub cycles: u64,

    // Debug flag
    pub debug: bool,
}

impl<M: MemoryInterface, I: IoInterface> Z80<M, I> {
    pub fn new(memory: M, io: I) -> Self {
        Self {
            a: 0xFF,
            f: 0xFF,
            b: 0,
            c: 0,
            d: 0,
            e: 0,
            h: 0,
            l: 0,
            a_prime: 0,
            f_prime: 0,
            b_prime: 0,
            c_prime: 0,
            d_prime: 0,
            e_prime: 0,
            h_prime: 0,
            l_prime: 0,
            ix: 0,
            iy: 0,
            sp: 0xFFFF,
            pc: 0,
            i: 0,
            r: 0,
            iff1: false,
            iff2: false,
            im: 0,
            memptr: 0,
            halted: false,
            pending_ei: false,
            memory,
            io,
            cycles: 0,
            debug: false,
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
        self.memptr = 0;
        self.halted = false;
        self.pending_ei = false;
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
        let mut new_f = self.f & !(flags::SIGN | flags::ZERO | flags::Y_FLAG | flags::X_FLAG);
        new_f |= value & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if value == 0 {
            new_f |= flags::ZERO;
        }
        self.f = new_f;
    }

    fn set_parity_flag(&mut self, value: u8) {
        let parity = value.count_ones().is_multiple_of(2);
        self.set_flag(flags::PARITY, parity);
    }

    // ========== Memory access helpers ==========

    fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory.read_byte(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);

        // Refresh register (R) increments on every instruction fetch
        // Bits 0-6 increment, Bit 7 is stable
        self.r = (self.r & 0x80) | ((self.r.wrapping_add(1)) & 0x7F);

        byte
    }

    fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    fn read_byte(&mut self, addr: u16) -> u8 {
        self.memory.read_byte(addr as u32)
    }

    fn write_byte(&mut self, addr: u16, value: u8) {
        self.memory.write_byte(addr as u32, value);
    }

    fn read_word(&mut self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, value as u8);
        self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
    }

    // ========== I/O access helpers ==========

    fn read_port(&mut self, port: u16) -> u8 {
        self.io.read_port(port)
    }

    fn write_port(&mut self, port: u16, value: u8) {
        self.io.write_port(port, value);
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
        let carry = if with_carry && (self.f & flags::CARRY) != 0 {
            1u16
        } else {
            0
        };
        let a = self.a as u16;
        let v = value as u16;
        let result = a + v + carry;

        let half_carry = ((a & 0x0F) + (v & 0x0F) + carry) > 0x0F;
        let overflow = ((a ^ result) & (v ^ result) & 0x80) != 0;

        self.a = result as u8;

        let mut f = 0;
        if result > 0xFF { f |= flags::CARRY; }
        if half_carry { f |= flags::HALF_CARRY; }
        if overflow { f |= flags::PARITY; }
        // ADD_SUB is false (0)

        // Inline set_sz_flags logic
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }
        self.f = f;
    }

    fn sub_a(&mut self, value: u8, with_carry: bool, store: bool) {
        let carry = if with_carry && (self.f & flags::CARRY) != 0 {
            1u16
        } else {
            0
        };
        let a = self.a as u16;
        let v = value as u16;
        let result = a.wrapping_sub(v).wrapping_sub(carry);

        let half_carry = (a & 0x0F) < (v & 0x0F) + carry;
        let overflow = ((a ^ v) & (a ^ result) & 0x80) != 0;

        if store {
            self.a = result as u8;
        }

        let mut f = flags::ADD_SUB;
        if result > 0xFF { f |= flags::CARRY; }
        if half_carry { f |= flags::HALF_CARRY; }
        if overflow { f |= flags::PARITY; }

        let res_u8 = result as u8;
        f |= res_u8 & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if res_u8 == 0 {
            f |= flags::ZERO;
        }
        self.f = f;
    }

    fn and_a(&mut self, value: u8) {
        self.a &= value;

        // H=1, N=0, C=0
        let mut f = flags::HALF_CARRY;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        if self.a.count_ones().is_multiple_of(2) {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    fn or_a(&mut self, value: u8) {
        self.a |= value;

        // H=0, N=0, C=0
        let mut f = 0;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        if self.a.count_ones().is_multiple_of(2) {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    fn xor_a(&mut self, value: u8) {
        self.a ^= value;

        // H=0, N=0, C=0
        let mut f = 0;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        if self.a.count_ones().is_multiple_of(2) {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);

        let mut f = self.f & flags::CARRY; // Preserve Carry
        if (value & 0x0F) == 0x0F { f |= flags::HALF_CARRY; }
        if value == 0x7F { f |= flags::PARITY; }
        // ADD_SUB is false (0)

        f |= result & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if result == 0 {
            f |= flags::ZERO;
        }

        self.f = f;
        result
    }

    fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let mut f = (self.f & flags::CARRY) | flags::ADD_SUB; // Preserve Carry, set N
        if (value & 0x0F) == 0x00 { f |= flags::HALF_CARRY; }
        if value == 0x80 { f |= flags::PARITY; }

        f |= result & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if result == 0 {
            f |= flags::ZERO;
        }

        self.f = f;
        result
    }

    fn add_hl(&mut self, value: u16) {
        let hl = self.hl() as u32;
        let v = value as u32;
        let result = hl + v;

        self.set_flag(flags::CARRY, result > 0xFFFF);
        self.set_flag(flags::HALF_CARRY, ((hl & 0x0FFF) + (v & 0x0FFF)) > 0x0FFF);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from High Byte
        let h_res = (result >> 8) as u8;
        self.set_flag(flags::X_FLAG, (h_res & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (h_res & 0x20) != 0);

        self.memptr = hl.wrapping_add(1) as u16;
        self.set_hl(result as u16);
    }

    // Index Register Half Accessors
    pub fn ixh(&self) -> u8 {
        (self.ix >> 8) as u8
    }
    pub fn ixl(&self) -> u8 {
        (self.ix & 0xFF) as u8
    }
    pub fn iyh(&self) -> u8 {
        (self.iy >> 8) as u8
    }
    pub fn iyl(&self) -> u8 {
        (self.iy & 0xFF) as u8
    }

    pub fn set_ixh(&mut self, val: u8) {
        self.ix = (self.ix & 0x00FF) | ((val as u16) << 8);
    }
    pub fn set_ixl(&mut self, val: u8) {
        self.ix = (self.ix & 0xFF00) | (val as u16);
    }
    pub fn set_iyh(&mut self, val: u8) {
        self.iy = (self.iy & 0x00FF) | ((val as u16) << 8);
    }
    pub fn set_iyl(&mut self, val: u8) {
        self.iy = (self.iy & 0xFF00) | (val as u16);
    }

    fn get_index_byte(&self, r: u8, is_ix: bool) -> u8 {
        match r {
            0 => self.b,
            1 => self.c,
            2 => self.d,
            3 => self.e,
            4 => {
                if is_ix {
                    self.ixh()
                } else {
                    self.iyh()
                }
            }
            5 => {
                if is_ix {
                    self.ixl()
                } else {
                    self.iyl()
                }
            }
            7 => self.a,
            _ => 0,
        }
    }

    fn set_index_byte(&mut self, r: u8, val: u8, is_ix: bool) {
        match r {
            0 => self.b = val,
            1 => self.c = val,
            2 => self.d = val,
            3 => self.e = val,
            4 => {
                if is_ix {
                    self.set_ixh(val)
                } else {
                    self.set_iyh(val)
                }
            }
            5 => {
                if is_ix {
                    self.set_ixl(val)
                } else {
                    self.set_iyl(val)
                }
            }
            7 => self.a = val,
            _ => {}
        }
    }

    // ========== Rotate/Shift operations ==========

    fn rlca(&mut self) {
        let carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if carry { 1 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    fn rrca(&mut self) {
        let carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if carry { 0x80 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    fn rla(&mut self) {
        let old_carry = self.get_flag(flags::CARRY);
        let new_carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if old_carry { 1 } else { 0 };
        self.set_flag(flags::CARRY, new_carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    fn rra(&mut self) {
        let old_carry = self.get_flag(flags::CARRY);
        let new_carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if old_carry { 0x80 } else { 0 };
        self.set_flag(flags::CARRY, new_carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    // ========== Helper to get/set register by index ==========

    fn get_reg(&mut self, index: u8) -> u8 {
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
            0 => !self.get_flag(flags::ZERO),   // NZ
            1 => self.get_flag(flags::ZERO),    // Z
            2 => !self.get_flag(flags::CARRY),  // NC
            3 => self.get_flag(flags::CARRY),   // C
            4 => !self.get_flag(flags::PARITY), // PO
            5 => self.get_flag(flags::PARITY),  // PE
            6 => !self.get_flag(flags::SIGN),   // P
            7 => self.get_flag(flags::SIGN),    // M
            _ => false,
        }
    }

    // ========== Main execution ==========

    /// Trigger a maskable interrupt
    pub fn trigger_interrupt(&mut self, vector: u8) -> u8 {
        if !self.iff1 || self.pending_ei {
            return 0;
        }

        self.halted = false;
        self.iff1 = false;
        self.iff2 = false;

        match self.im {
            0 | 1 => {
                self.push(self.pc);
                self.pc = 0x0038;
                13
            }
            2 => {
                self.push(self.pc);
                let addr = ((self.i as u16) << 8) | vector as u16;
                self.memptr = addr;
                let handler = self.read_word(addr);
                self.pc = handler;
                8
            }
            _ => 0,
        }
    }

    /// Trigger a non-maskable interrupt (NMI)
    pub fn trigger_nmi(&mut self) -> u8 {
        self.halted = false;
        self.iff2 = self.iff1;
        self.iff1 = false;
        self.push(self.pc);
        self.pc = 0x0066;
        11
    }

    /// Execute one instruction, returns number of T-states used
    pub fn step(&mut self) -> u8 {
        if self.halted {
            return 4;
        }

        // Handle EI shadow: iff1/iff2 are set but interrupts are inhibited for one instruction
        let _interrupts_inhibited = self.pending_ei;
        self.pending_ei = false;

        let _pc_before = self.pc;
        let opcode = self.fetch_byte();

        if self.debug {
            eprintln!("DEBUG: Z80 STEP: PC=0x{:04X} Op=0x{:02X} A={:02X} F={:02X} BC={:04X} DE={:04X} HL={:04X} SP={:04X} CYC={}", 
                _pc_before, opcode, self.a, self.f, self.bc(), self.de(), self.hl(), self.sp, self.cycles);
        }

        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = (y >> 1) & 0x03;
        let q = y & 0x01;

        let t_states = match x {
            0 => self.execute_x0(opcode, y, z, p, q),
            1 => self.execute_x1(y, z),
            2 => self.execute_x2(y, z),
            3 => self.execute_x3(opcode, y, z, p, q),
            _ => 4,
        };

        self.cycles += t_states as u64;
        t_states
    }

    fn execute_x0(&mut self, _opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8 {
        match z {
            0 => self.execute_x0_control_misc(y),
            1 => self.execute_x0_load_add_hl(p, q),
            2 => self.execute_x0_load_indirect(p, q),
            3 => self.execute_x0_inc_dec_rp(p, q),
            4 => self.execute_x0_inc_r(y),
            5 => self.execute_x0_dec_r(y),
            6 => self.execute_x0_ld_r_n(y),
            7 => self.execute_x0_rotate_accum_flags(y),
            _ => 4,
        }
    }

    fn execute_x0_control_misc(&mut self, y: u8) -> u8 {
        match y {
            0 => 4, // NOP
            1 => {
                // EX AF, AF'
                std::mem::swap(&mut self.a, &mut self.a_prime);
                std::mem::swap(&mut self.f, &mut self.f_prime);
                4
            }
            2 => {
                // DJNZ d
                let d = self.fetch_byte() as i8;
                self.b = self.b.wrapping_sub(1);
                if self.b != 0 {
                    self.pc = (self.pc as i32 + d as i32) as u16;
                    13
                } else {
                    8
                }
            }
            3 => {
                // JR d
                let d = self.fetch_byte() as i8;
                self.pc = (self.pc as i32 + d as i32) as u16;
                12
            }
            4..=7 => {
                // JR cc, d
                let d = self.fetch_byte() as i8;
                if self.check_condition(y - 4) {
                    self.pc = (self.pc as i32 + d as i32) as u16;
                    12
                } else {
                    7
                }
            }
            _ => 4,
        }
    }

    fn execute_x0_load_add_hl(&mut self, p: u8, q: u8) -> u8 {
        if q == 0 {
            // LD rp, nn
            let nn = self.fetch_word();
            self.set_rp(p, nn);
            10
        } else {
            // ADD HL, rp
            let rp = self.get_rp(p);
            self.add_hl(rp);
            11
        }
    }

    fn execute_x0_load_indirect(&mut self, p: u8, q: u8) -> u8 {
        match (p, q) {
            (0, 0) => {
                // LD (BC), A
                let addr = self.bc();
                self.write_byte(addr, self.a);
                self.memptr = ((self.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
                7
            }
            (0, 1) => {
                // LD A, (BC)
                let addr = self.bc();
                self.a = self.read_byte(addr);
                self.memptr = addr.wrapping_add(1);
                7
            }
            (1, 0) => {
                // LD (DE), A
                let addr = self.de();
                self.write_byte(addr, self.a);
                self.memptr = ((self.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
                7
            }
            (1, 1) => {
                // LD A, (DE)
                let addr = self.de();
                self.a = self.read_byte(addr);
                self.memptr = addr.wrapping_add(1);
                7
            }
            (2, 0) => {
                // LD (nn), HL
                let addr = self.fetch_word();
                self.write_word(addr, self.hl());
                self.memptr = addr.wrapping_add(1);
                16
            }
            (2, 1) => {
                // LD HL, (nn)
                let addr = self.fetch_word();
                let val = self.read_word(addr);
                self.set_hl(val);
                self.memptr = addr.wrapping_add(1);
                16
            }
            (3, 0) => {
                // LD (nn), A
                let addr = self.fetch_word();
                self.write_byte(addr, self.a);
                self.memptr = (self.a as u16) << 8 | addr.wrapping_add(1) & 0xFF;
                self.memptr = ((self.a as u16) << 8) | (addr.wrapping_add(1) & 0xFF);
                13
            }
            (3, 1) => {
                // LD A, (nn)
                let addr = self.fetch_word();
                self.a = self.read_byte(addr);
                self.memptr = addr.wrapping_add(1);
                13
            }
            _ => 4,
        }
    }

    fn execute_x0_inc_dec_rp(&mut self, p: u8, q: u8) -> u8 {
        // INC/DEC rp
        let rp = self.get_rp(p);
        if q == 0 {
            self.set_rp(p, rp.wrapping_add(1));
        } else {
            self.set_rp(p, rp.wrapping_sub(1));
        }
        6
    }

    fn execute_x0_inc_r(&mut self, y: u8) -> u8 {
        // INC r
        let val = self.get_reg(y);
        let result = self.inc(val);
        self.set_reg(y, result);
        if y == 6 {
            11
        } else {
            4
        }
    }

    fn execute_x0_dec_r(&mut self, y: u8) -> u8 {
        // DEC r
        let val = self.get_reg(y);
        let result = self.dec(val);
        self.set_reg(y, result);
        if y == 6 {
            11
        } else {
            4
        }
    }

    fn execute_x0_ld_r_n(&mut self, y: u8) -> u8 {
        // LD r, n
        let n = self.fetch_byte();
        self.set_reg(y, n);
        if y == 6 {
            10
        } else {
            7
        }
    }

    fn execute_x0_rotate_accum_flags(&mut self, y: u8) -> u8 {
        match y {
            0 => {
                self.rlca();
                4
            }
            1 => {
                self.rrca();
                4
            }
            2 => {
                self.rla();
                4
            }
            3 => {
                self.rra();
                4
            }
            4 => {
                // DAA - Decimal Adjust Accumulator
                // DAA adjusts A to valid BCD based on N, H, C flags
                let mut correction: u8 = 0;
                let mut carry = self.get_flag(flags::CARRY);

                if self.get_flag(flags::ADD_SUB) {
                    // After subtraction
                    if self.get_flag(flags::HALF_CARRY) {
                        correction |= 0x06;
                    }
                    if carry {
                        correction |= 0x60;
                    }
                    self.a = self.a.wrapping_sub(correction);
                } else {
                    // After addition
                    if self.get_flag(flags::HALF_CARRY) || (self.a & 0x0F) > 9 {
                        correction |= 0x06;
                    }
                    if carry || self.a > 0x99 {
                        correction |= 0x60;
                        carry = true;
                    }
                    self.a = self.a.wrapping_add(correction);
                }

                self.set_flag(flags::CARRY, carry);
                self.set_flag(flags::HALF_CARRY, (correction & 0x06) != 0);
                self.set_sz_flags(self.a);
                self.set_parity_flag(self.a);
                4
            }
            5 => {
                // CPL
                self.a = !self.a;
                self.set_flag(flags::HALF_CARRY, true);
                self.set_flag(flags::ADD_SUB, true);
                self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
                self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
                4
            }
            6 => {
                // SCF
                self.set_flag(flags::CARRY, true);
                self.set_flag(flags::HALF_CARRY, false);
                self.set_flag(flags::ADD_SUB, false);
                self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
                self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
                4
            }
            7 => {
                // CCF
                let c = self.get_flag(flags::CARRY);
                self.set_flag(flags::HALF_CARRY, c);
                self.set_flag(flags::CARRY, !c);
                self.set_flag(flags::ADD_SUB, false);
                self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
                self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
                4
            }
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
            if y == 6 || z == 6 {
                7
            } else {
                4
            }
        }
    }

    fn execute_x2(&mut self, y: u8, z: u8) -> u8 {
        // ALU operations
        let val = self.get_reg(z);
        match y {
            0 => self.add_a(val, false),        // ADD A, r
            1 => self.add_a(val, true),         // ADC A, r
            2 => self.sub_a(val, false, true),  // SUB r
            3 => self.sub_a(val, true, true),   // SBC A, r
            4 => self.and_a(val),               // AND r
            5 => self.xor_a(val),               // XOR r
            6 => self.or_a(val),                // OR r
            7 => self.sub_a(val, false, false), // CP r
            _ => {}
        }
        if z == 6 {
            7
        } else {
            4
        }
    }

    fn execute_x3(&mut self, _opcode: u8, y: u8, z: u8, p: u8, q: u8) -> u8 {
        match z {
            0 => self.execute_x3_ret_cc(y),
            1 => self.execute_x3_pop_ret_exx(p, q),
            2 => self.execute_x3_jp_cc(y),
            3 => self.execute_x3_jp_out_ex_di_ei(y),
            4 => self.execute_x3_call_cc(y),
            5 => self.execute_x3_push_call_prefixes(p, q),
            6 => self.execute_x3_alu_n(y),
            7 => self.execute_x3_rst(y),
            _ => 4,
        }
    }

    fn execute_x3_ret_cc(&mut self, y: u8) -> u8 {
        // RET cc
        if self.check_condition(y) {
            self.pc = self.pop();
            11
        } else {
            5
        }
    }

    fn execute_x3_pop_ret_exx(&mut self, p: u8, q: u8) -> u8 {
        if q == 0 {
            // POP rp2
            let val = self.pop();
            self.set_rp2(p, val);
            10
        } else {
            match p {
                0 => {
                    // RET
                    self.pc = self.pop();
                    10
                }
                1 => {
                    // EXX
                    std::mem::swap(&mut self.b, &mut self.b_prime);
                    std::mem::swap(&mut self.c, &mut self.c_prime);
                    std::mem::swap(&mut self.d, &mut self.d_prime);
                    std::mem::swap(&mut self.e, &mut self.e_prime);
                    std::mem::swap(&mut self.h, &mut self.h_prime);
                    std::mem::swap(&mut self.l, &mut self.l_prime);
                    4
                }
                2 => {
                    // JP HL
                    self.pc = self.hl();
                    4
                }
                3 => {
                    // LD SP, HL
                    self.sp = self.hl();
                    6
                }
                _ => 4,
            }
        }
    }

    fn execute_x3_jp_cc(&mut self, y: u8) -> u8 {
        // JP cc, nn
        let nn = self.fetch_word();
        if self.check_condition(y) {
            self.pc = nn;
        }
        10
    }

    fn execute_x3_jp_out_ex_di_ei(&mut self, y: u8) -> u8 {
        match y {
            0 => {
                // JP nn
                self.pc = self.fetch_word();
                10
            }
            1 => self.execute_cb_prefix(),
            2 => {
                // OUT (n), A
                let n = self.fetch_byte();
                let port = (n as u16) | ((self.a as u16) << 8);
                self.write_port(port, self.a);
                11
            }
            3 => {
                // IN A, (n)
                let n = self.fetch_byte();
                let port = (n as u16) | ((self.a as u16) << 8);
                self.a = self.read_port(port);
                11
            }
            4 => {
                // EX (SP), HL
                let val = self.read_word(self.sp);
                self.write_word(self.sp, self.hl());
                self.set_hl(val);
                self.memptr = val;
                19
            }
            5 => {
                // EX DE, HL
                let de = self.de();
                let hl = self.hl();
                self.set_de(hl);
                self.set_hl(de);
                4
            }
            6 => {
                // DI
                self.iff1 = false;
                self.iff2 = false;
                4
            }
            7 => {
                // EI
                self.iff1 = true;
                self.iff2 = true;
                self.pending_ei = true;
                4
            }
            _ => 4,
        }
    }

    fn execute_x3_call_cc(&mut self, y: u8) -> u8 {
        // CALL cc, nn
        let nn = self.fetch_word();
        if self.check_condition(y) {
            self.push(self.pc);
            self.pc = nn;
            17
        } else {
            10
        }
    }

    fn execute_x3_push_call_prefixes(&mut self, p: u8, q: u8) -> u8 {
        if q == 0 {
            // PUSH rp2
            let val = self.get_rp2(p);
            self.push(val);
            11
        } else {
            match p {
                0 => {
                    // CALL nn
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
        }
    }

    fn execute_x3_alu_n(&mut self, y: u8) -> u8 {
        // ALU A, n
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

    fn execute_x3_rst(&mut self, y: u8) -> u8 {
        // RST y*8
        self.push(self.pc);
        self.pc = (y as u16) * 8;
        11
    }

    // ========== CB Prefix (Bit operations) ==========

    fn cb_rotate_shift(&mut self, val: u8, y: u8) -> u8 {
        let result = match y {
            0 => {
                // RLC
                let carry = (val & 0x80) != 0;
                self.set_flag(flags::CARRY, carry);
                (val << 1) | if carry { 1 } else { 0 }
            }
            1 => {
                // RRC
                let carry = (val & 0x01) != 0;
                self.set_flag(flags::CARRY, carry);
                (val >> 1) | if carry { 0x80 } else { 0 }
            }
            2 => {
                // RL
                let old_carry = self.get_flag(flags::CARRY);
                let carry = (val & 0x80) != 0;
                self.set_flag(flags::CARRY, carry);
                (val << 1) | if old_carry { 1 } else { 0 }
            }
            3 => {
                // RR
                let old_carry = self.get_flag(flags::CARRY);
                let carry = (val & 0x01) != 0;
                self.set_flag(flags::CARRY, carry);
                (val >> 1) | if old_carry { 0x80 } else { 0 }
            }
            4 => {
                // SLA
                let carry = (val & 0x80) != 0;
                self.set_flag(flags::CARRY, carry);
                val << 1
            }
            5 => {
                // SRA
                let carry = (val & 0x01) != 0;
                self.set_flag(flags::CARRY, carry);
                (val >> 1) | (val & 0x80)
            }
            6 => {
                // SLL (undocumented)
                let carry = (val & 0x80) != 0;
                self.set_flag(flags::CARRY, carry);
                (val << 1) | 1
            }
            7 => {
                // SRL
                let carry = (val & 0x01) != 0;
                self.set_flag(flags::CARRY, carry);
                val >> 1
            }
            _ => val,
        };
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        self.set_sz_flags(result);
        self.set_parity_flag(result);
        result
    }

    fn cb_bit(&mut self, val: u8, bit: u8) {
        let b = (val >> bit) & 1;
        self.set_flag(flags::ZERO, b == 0);
        self.set_flag(flags::HALF_CARRY, true);
        self.set_flag(flags::ADD_SUB, false);
    }

    fn cb_res(&mut self, val: u8, bit: u8) -> u8 {
        val & !(1 << bit)
    }

    fn cb_set(&mut self, val: u8, bit: u8) -> u8 {
        val | (1 << bit)
    }

    fn execute_cb_prefix(&mut self) -> u8 {
        let opcode = self.fetch_byte();
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;

        let val = self.get_reg(z);

        match x {
            0 => {
                // Rotate/shift
                let result = self.cb_rotate_shift(val, y);
                self.set_reg(z, result);
                if z == 6 { 15 } else { 8 }
            }
            1 => {
                // BIT y, r
                self.cb_bit(val, y);

                if z != 6 {
                    self.set_flag(flags::X_FLAG, (val & 0x08) != 0);
                    self.set_flag(flags::Y_FLAG, (val & 0x20) != 0);
                } else {
                    // For (HL), X/Y come from MEMPTR (WZ) high byte.
                    let h_memptr = (self.memptr >> 8) as u8;
                    self.set_flag(flags::X_FLAG, (h_memptr & 0x08) != 0);
                    self.set_flag(flags::Y_FLAG, (h_memptr & 0x20) != 0);
                }

                if z == 6 { 12 } else { 8 }
            }
            2 => {
                // RES y, r
                let result = self.cb_res(val, y);
                self.set_reg(z, result);
                if z == 6 { 15 } else { 8 }
            }
            3 => {
                // SET y, r
                let result = self.cb_set(val, y);
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
                0 => {
                    // IN r, (C)
                    let port = self.bc();
                    let val = self.read_port(port);
                    if y != 6 {
                        self.set_reg(y, val);
                    }
                    self.set_sz_flags(val);
                    self.set_parity_flag(val);
                    self.set_flag(flags::HALF_CARRY, false);
                    self.set_flag(flags::ADD_SUB, false);
                    12
                }
                1 => {
                    // OUT (C), r
                    let port = self.bc();
                    let val = if y == 6 { 0 } else { self.get_reg(y) };
                    self.write_port(port, val);
                    12
                }
                2 => {
                    if q == 0 {
                        // SBC HL, rp
                        let hl = self.hl() as u32;
                        let rp = self.get_rp(p) as u32;
                        let c = if self.get_flag(flags::CARRY) { 1u32 } else { 0 };
                        let result = hl.wrapping_sub(rp).wrapping_sub(c);

                        self.set_flag(flags::CARRY, result > 0xFFFF);
                        self.set_flag(flags::ADD_SUB, true);
                        self.set_flag(flags::ZERO, (result & 0xFFFF) == 0);
                        self.set_flag(flags::SIGN, (result & 0x8000) != 0);
                        // Half borrow: (HL & 0xFFF) - (RP & 0xFFF) - C < 0
                        let h_check = (hl & 0xFFF).wrapping_sub(rp & 0xFFF).wrapping_sub(c);
                        self.set_flag(flags::HALF_CARRY, h_check > 0xFFF);
                        // P/V: Overflow
                        let overflow = ((hl ^ rp) & (hl ^ result) & 0x8000) != 0;
                        self.set_flag(flags::PARITY, overflow);

                        // X/Y from High Byte
                        let h_res = (result >> 8) as u8;
                        self.set_flag(flags::X_FLAG, (h_res & 0x08) != 0);
                        self.set_flag(flags::Y_FLAG, (h_res & 0x20) != 0);

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
                        // Half carry: Carry from bit 11
                        self.set_flag(flags::HALF_CARRY, ((hl & 0xFFF) + (rp & 0xFFF) + c) > 0xFFF);
                        // P/V: Overflow
                        let overflow = (!(hl ^ rp) & (hl ^ result) & 0x8000) != 0;
                        self.set_flag(flags::PARITY, overflow);

                        // X/Y from High Byte
                        let h_res = (result >> 8) as u8;
                        self.set_flag(flags::X_FLAG, (h_res & 0x08) != 0);
                        self.set_flag(flags::Y_FLAG, (h_res & 0x20) != 0);

                        self.set_hl(result as u16);
                        15
                    }
                }
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
                    self.memptr = nn.wrapping_add(1);
                    20
                }
                4 => {
                    // NEG
                    let a = self.a;
                    self.a = 0;
                    self.sub_a(a, false, true);
                    8
                }
                5 => {
                    if q == 0 {
                        // RETN
                        self.iff1 = self.iff2;
                        self.pc = self.pop();
                        14
                    } else {
                        // RETI
                        self.pc = self.pop();
                        14
                    }
                }
                6 => {
                    // IM y
                    self.im = match y & 0x03 {
                        0 | 1 => 0,
                        2 => 1,
                        3 => 2,
                        _ => 0,
                    };
                    8
                }
                7 => match y {
                    0 => {
                        // LD I, A
                        self.i = self.a;
                        9
                    }
                    1 => {
                        // LD R, A
                        self.r = self.a;
                        9
                    }
                    2 => {
                        // LD A, I
                        self.a = self.i;
                        self.set_sz_flags(self.a);
                        self.set_flag(flags::PARITY, self.iff2);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        9
                    }
                    3 => {
                        // LD A, R
                        self.a = self.r;
                        self.set_sz_flags(self.a);
                        self.set_flag(flags::PARITY, self.iff2);
                        self.set_flag(flags::HALF_CARRY, false);
                        self.set_flag(flags::ADD_SUB, false);
                        9
                    }
                    4 => {
                        // RRD
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
                    5 => {
                        // RLD
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

        let n_val = val.wrapping_add(self.a);
        self.set_flag(flags::Y_FLAG, (n_val & 0x02) != 0);
        self.set_flag(flags::X_FLAG, (n_val & 0x08) != 0);
        self.set_flag(flags::PARITY, bc != 0);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);

        // LDIR/LDDR
        if y >= 6 && bc != 0 {
            self.pc = self.pc.wrapping_sub(2);
            self.memptr = self.pc.wrapping_add(1);
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

        let h = (self.a & 0x0F) < (val & 0x0F);
        self.set_flag(flags::ZERO, result == 0);
        self.set_flag(flags::SIGN, (result & 0x80) != 0);
        self.set_flag(flags::HALF_CARRY, h);
        self.set_flag(flags::PARITY, bc != 0);
        self.set_flag(flags::ADD_SUB, true);

        // CPI/CPD X/Y flags: based on A - val - H
        let mut x_val = self.a.wrapping_sub(val);
        if h {
            x_val = x_val.wrapping_sub(1);
        }
        self.set_flag(flags::Y_FLAG, (x_val & 0x02) != 0);
        self.set_flag(flags::X_FLAG, (x_val & 0x08) != 0);

        self.memptr = self.memptr.wrapping_add(1);

        // CPIR/CPDR
        if y >= 6 && bc != 0 && result != 0 {
            self.pc = self.pc.wrapping_sub(2);
            self.memptr = self.pc.wrapping_add(1);
            21
        } else {
            16
        }
    }

    fn execute_ini_ind(&mut self, y: u8) -> u8 {
        // INI (y=4), IND (y=5), INIR (y=6), INDR (y=7)

        let port = self.bc();
        let hl = self.hl();

        let io_val = self.read_port(port);
        self.write_byte(hl, io_val);

        let b = self.b.wrapping_sub(1);
        self.b = b;

        let new_hl = if (y & 1) == 0 {
            hl.wrapping_add(1)
        } else {
            hl.wrapping_sub(1)
        };
        self.set_hl(new_hl);

        // Flags:
        // Z: set if B=0
        // N: Set (bit 7 of internal calculation?) -> Z80 manual says N is Set.
        self.set_flag(flags::ZERO, b == 0);
        self.set_flag(flags::ADD_SUB, true);

        // Repeat logic for INIR/INDR (y>=6)
        if y >= 6 && b != 0 {
            self.pc = self.pc.wrapping_sub(2);
            21
        } else {
            16
        }
    }

    fn execute_outi_outd(&mut self, y: u8) -> u8 {
        // OUTI (y=4), OUTD (y=5), OTIR (y=6), OTDR (y=7)

        let hl = self.hl();
        let val = self.read_byte(hl);

        let port = self.bc();
        self.write_port(port, val);

        let b = self.b.wrapping_sub(1);
        self.b = b;

        let new_hl = if (y & 1) == 0 {
            hl.wrapping_add(1)
        } else {
            hl.wrapping_sub(1)
        };
        self.set_hl(new_hl);

        self.set_flag(flags::ZERO, b == 0);
        self.set_flag(flags::ADD_SUB, true);

        if y >= 6 && b != 0 {
            self.pc = self.pc.wrapping_sub(2);
            21
        } else {
            16
        }
    }

    // ========== Index Register Helpers (for DD/FD prefixes) ==========

    fn get_index_val(&self, is_ix: bool) -> u16 {
        if is_ix {
            self.ix
        } else {
            self.iy
        }
    }

    fn set_index_val(&mut self, val: u16, is_ix: bool) {
        if is_ix {
            self.ix = val;
        } else {
            self.iy = val;
        }
    }

    fn get_index_h(&self, is_ix: bool) -> u8 {
        if is_ix {
            self.ixh()
        } else {
            self.iyh()
        }
    }

    fn set_index_h(&mut self, val: u8, is_ix: bool) {
        if is_ix {
            self.set_ixh(val);
        } else {
            self.set_iyh(val);
        }
    }

    fn get_index_l(&self, is_ix: bool) -> u8 {
        if is_ix {
            self.ixl()
        } else {
            self.iyl()
        }
    }

    fn set_index_l(&mut self, val: u8, is_ix: bool) {
        if is_ix {
            self.set_ixl(val);
        } else {
            self.set_iyl(val);
        }
    }

    fn add_index(&mut self, value: u16, is_ix: bool) {
        let idx = if is_ix { self.ix } else { self.iy } as u32;
        let v = value as u32;
        let result = idx + v;

        self.set_flag(flags::CARRY, result > 0xFFFF);
        self.set_flag(flags::HALF_CARRY, ((idx & 0x0FFF) + (v & 0x0FFF)) > 0x0FFF);
        self.set_flag(flags::ADD_SUB, false);

        self.memptr = idx.wrapping_add(1) as u16;
        if is_ix {
            self.ix = result as u16;
        } else {
            self.iy = result as u16;
        }
    }

    fn calc_index_addr(&mut self, offset: i8, is_ix: bool) -> u16 {
        let idx = self.get_index_val(is_ix);
        let addr = (idx as i16 + offset as i16) as u16;
        self.memptr = addr;
        addr
    }

    fn execute_index_alu(&mut self, op_index: u8, val: u8) {
        match op_index {
            0 => self.add_a(val, false),
            1 => self.add_a(val, true),
            2 => self.sub_a(val, false, true),
            3 => self.sub_a(val, true, true),
            4 => self.and_a(val),
            5 => self.xor_a(val),
            6 => self.or_a(val),
            7 => self.sub_a(val, false, false),
            _ => {}
        }
    }

    fn execute_index_prefix(&mut self, is_ix: bool) -> u8 {
        let opcode = self.fetch_byte();

        match opcode {
            0x09 => {
                let val = self.bc();
                self.add_index(val, is_ix);
                15
            }
            0x19 => {
                let val = self.de();
                self.add_index(val, is_ix);
                15
            }
            0x21 => {
                let val = self.fetch_word();
                self.set_index_val(val, is_ix);
                14
            }
            0x22 => {
                let addr = self.fetch_word();
                let val = self.get_index_val(is_ix);
                self.write_word(addr, val);
                20
            }
            0x23 => {
                let val = self.get_index_val(is_ix);
                self.set_index_val(val.wrapping_add(1), is_ix);
                10
            }
            0x24 => {
                let val = self.get_index_h(is_ix);
                let res = self.inc(val);
                self.set_index_h(res, is_ix);
                8
            }
            0x25 => {
                let val = self.get_index_h(is_ix);
                let res = self.dec(val);
                self.set_index_h(res, is_ix);
                8
            }
            0x26 => {
                let n = self.fetch_byte();
                self.set_index_h(n, is_ix);
                11
            }
            0x29 => {
                let val = self.get_index_val(is_ix);
                self.add_index(val, is_ix);
                15
            }
            0x2A => {
                let addr = self.fetch_word();
                let val = self.read_word(addr);
                self.set_index_val(val, is_ix);
                20
            }
            0x2B => {
                let val = self.get_index_val(is_ix);
                self.set_index_val(val.wrapping_sub(1), is_ix);
                10
            }
            0x2C => {
                let val = self.get_index_l(is_ix);
                let res = self.inc(val);
                self.set_index_l(res, is_ix);
                8
            }
            0x2D => {
                let val = self.get_index_l(is_ix);
                let res = self.dec(val);
                self.set_index_l(res, is_ix);
                8
            }
            0x2E => {
                let n = self.fetch_byte();
                self.set_index_l(n, is_ix);
                11
            }
            0x34 => {
                let d = self.fetch_byte() as i8;
                let addr = self.calc_index_addr(d, is_ix);
                let val = self.read_byte(addr);
                let result = self.inc(val);
                self.write_byte(addr, result);
                23
            }
            0x35 => {
                let d = self.fetch_byte() as i8;
                let addr = self.calc_index_addr(d, is_ix);
                let val = self.read_byte(addr);
                let result = self.dec(val);
                self.write_byte(addr, result);
                23
            }
            0x36 => {
                let d = self.fetch_byte() as i8;
                let n = self.fetch_byte();
                let addr = self.calc_index_addr(d, is_ix);
                self.write_byte(addr, n);
                19
            }
            0x39 => {
                self.add_index(self.sp, is_ix);
                15
            }

            // Specific ALU ops
            0x86 | 0x8E | 0x96 | 0x9E | 0xA6 | 0xAE | 0xB6 | 0xBE => {
                let d = self.fetch_byte() as i8;
                let addr = self.calc_index_addr(d, is_ix);
                let val = self.read_byte(addr);
                self.execute_index_alu((opcode >> 3) & 0x07, val);
                19
            }

            // LD r, (IX/IY+d) and LD (IX/IY+d), r
            // LD r, (IX/IY+d) and LD (IX/IY+d), r
            0x46 | 0x4E | 0x56 | 0x5E | 0x66 | 0x6E | 0x7E => {
                let d = self.fetch_byte() as i8;
                let addr = self.calc_index_addr(d, is_ix);
                let val = self.read_byte(addr);
                let r = (opcode >> 3) & 0x07;
                self.set_reg(r, val);
                19
            }
            0x70..=0x75 | 0x77 => {
                let d = self.fetch_byte() as i8;
                let addr = self.calc_index_addr(d, is_ix);
                let r = opcode & 0x07;
                let val = self.get_reg(r);
                self.write_byte(addr, val);
                19
            }
            0x76 => {
                self.halted = true;
                8
            }

            // Generic Undocumented (using index halves)
            0x40..=0x7F => {
                if opcode == 0x76 {
                    self.halted = true;
                    return 8;
                }
                let r_src = opcode & 0x07;
                let r_dest = (opcode >> 3) & 0x07;
                let val = self.get_index_byte(r_src, is_ix);
                self.set_index_byte(r_dest, val, is_ix);
                8
            }

            // Generic Undocumented ALU
            0x80..=0xBF => {
                let val = self.get_index_byte(opcode & 0x07, is_ix);
                self.execute_index_alu((opcode >> 3) & 0x07, val);
                8
            }

            0xE1 => {
                let val = self.pop();
                self.set_index_val(val, is_ix);
                14
            }
            0xE3 => {
                let val = self.read_word(self.sp);
                let idx = self.get_index_val(is_ix);
                self.write_word(self.sp, idx);
                self.set_index_val(val, is_ix);
                self.memptr = val;
                23
            }
            0xE5 => {
                let idx = self.get_index_val(is_ix);
                self.push(idx);
                15
            }
            0xE9 => {
                self.pc = self.get_index_val(is_ix);
                8
            }
            0xF9 => {
                self.sp = self.get_index_val(is_ix);
                10
            }
            0xCB => {
                let d = self.fetch_byte() as i8;
                let opcode = self.fetch_byte();
                let addr = self.calc_index_addr(d, is_ix);
                self.execute_indexed_cb(opcode, addr)
            }
            _ => 8, // Treat as NOP
        }
    }

    // ========== DD Prefix (IX) ==========

    fn execute_dd_prefix(&mut self) -> u8 {
        self.execute_index_prefix(true)
    }

    // ========== FD Prefix (IY) ==========

    fn execute_fd_prefix(&mut self) -> u8 {
        self.execute_index_prefix(false)
    }

    fn execute_indexed_cb(&mut self, opcode: u8, addr: u16) -> u8 {
        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let val = self.read_byte(addr);

        match x {
            0 => {
                // Rotate/shift
                let result = self.cb_rotate_shift(val, y);
                self.write_byte(addr, result);
                if z != 6 {
                    self.set_reg(z, result);
                }
                23
            }
            1 => {
                // BIT y, (IX/IY+d)
                self.cb_bit(val, y);

                // X/Y from High Byte of EA
                let h_ea = (addr >> 8) as u8;
                self.set_flag(flags::X_FLAG, (h_ea & 0x08) != 0);
                self.set_flag(flags::Y_FLAG, (h_ea & 0x20) != 0);
                20
            }
            2 => {
                // RES y, (IX/IY+d)
                let result = self.cb_res(val, y);
                self.write_byte(addr, result);
                if z != 6 {
                    self.set_reg(z, result);
                }
                23
            }
            3 => {
                // SET y, (IX/IY+d)
                let result = self.cb_set(val, y);
                self.write_byte(addr, result);
                if z != 6 {
                    self.set_reg(z, result);
                }
                23
            }
            _ => 20,
        }
    }
}

impl<M: MemoryInterface, I: IoInterface> Debuggable for Z80<M, I> {
    fn read_state(&self) -> Value {
        json!({
            "a": self.a, "f": self.f,
            "b": self.b, "c": self.c,
            "d": self.d, "e": self.e,
            "h": self.h, "l": self.l,
            "ix": self.ix, "iy": self.iy,
            "sp": self.sp, "pc": self.pc,
            "iff1": self.iff1, "iff2": self.iff2,
            "im": self.im, "halted": self.halted,
            "cycles": self.cycles,
        })
    }

    fn write_state(&mut self, state: &Value) {
        if let Some(a) = state["a"].as_u64() { self.a = a as u8; }
        if let Some(f) = state["f"].as_u64() { self.f = f as u8; }
        if let Some(b) = state["b"].as_u64() { self.b = b as u8; }
        if let Some(c) = state["c"].as_u64() { self.c = c as u8; }
        if let Some(d) = state["d"].as_u64() { self.d = d as u8; }
        if let Some(e) = state["e"].as_u64() { self.e = e as u8; }
        if let Some(h) = state["h"].as_u64() { self.h = h as u8; }
        if let Some(l) = state["l"].as_u64() { self.l = l as u8; }
        if let Some(ix) = state["ix"].as_u64() { self.ix = ix as u16; }
        if let Some(iy) = state["iy"].as_u64() { self.iy = iy as u16; }
        if let Some(sp) = state["sp"].as_u64() { self.sp = sp as u16; }
        if let Some(pc) = state["pc"].as_u64() { self.pc = pc as u16; }
        if let Some(iff1) = state["iff1"].as_bool() { self.iff1 = iff1; }
        if let Some(iff2) = state["iff2"].as_bool() { self.iff2 = iff2; }
        if let Some(im) = state["im"].as_u64() { self.im = im as u8; }
        if let Some(halted) = state["halted"].as_bool() { self.halted = halted; }
        if let Some(cycles) = state["cycles"].as_u64() { self.cycles = cycles; }
    }
}

#[cfg(test)]
pub mod test_utils;

#[cfg(test)]
mod tests;

#[cfg(test)]
mod tests_alu;

#[cfg(test)]
mod tests_cb;

#[cfg(test)]
mod tests_control;

#[cfg(test)]
mod tests_load;

#[cfg(test)]
mod tests_regression;

#[cfg(test)]
mod tests_undoc;

#[cfg(test)]
mod tests_exhaustive;

#[cfg(test)]
mod tests_block;

#[cfg(test)]
mod tests_halfcarry;

#[cfg(test)]
mod tests_interrupt;

#[cfg(test)]
mod tests_reset;

#[cfg(test)]
mod tests_rrd_rld;

#[cfg(test)]
mod tests_timing;

#[cfg(test)]
mod tests_torture;

#[cfg(test)]
mod tests_gaps;

#[cfg(test)]
mod tests_memptr;

#[cfg(test)]
mod tests_ddcb;
