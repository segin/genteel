//! Z80 CPU Implementation for Genesis Sound Co-processor
//!
//! The Z80 is used as a sound co-processor in the Sega Genesis, running at 3.58 MHz.
//! It has access to 8KB of dedicated sound RAM and controls the YM2612 and SN76489.

use crate::memory::{IoInterface, MemoryInterface};

// #[cfg(test)]
// pub mod test_utils;

pub mod op_cb;
use op_cb::CbOps;

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
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[macro_use]
mod macros;

pub mod op_general;
use op_general::GeneralOps;

pub mod op_ed;
use op_ed::EdOps;

pub mod op_index;
use op_index::IndexOps;

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

    // Memory interface (Generic for static dispatch performance)
    pub memory: M,

    // I/O interface (Generic for static dispatch performance)
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

    pub(crate) fn set_sz_flags(&mut self, value: u8) {
        let mut new_f = self.f & !(flags::SIGN | flags::ZERO | flags::Y_FLAG | flags::X_FLAG);
        new_f |= value & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if value == 0 {
            new_f |= flags::ZERO;
        }
        self.f = new_f;
    }

    pub(crate) fn set_parity_flag(&mut self, value: u8) {
        let parity = value.count_ones().is_multiple_of(2);
        self.set_flag(flags::PARITY, parity);
    }

    // ========== Memory access helpers ==========

    pub(crate) fn fetch_byte(&mut self) -> u8 {
        let byte = self.memory.read_byte(self.pc as u32);
        self.pc = self.pc.wrapping_add(1);

        // Refresh register (R) increments on every instruction fetch
        // Bits 0-6 increment, Bit 7 is stable
        self.r = (self.r & 0x80) | ((self.r.wrapping_add(1)) & 0x7F);

        byte
    }

    pub(crate) fn fetch_word(&mut self) -> u16 {
        let low = self.fetch_byte() as u16;
        let high = self.fetch_byte() as u16;
        (high << 8) | low
    }

    pub(crate) fn read_byte(&mut self, addr: u16) -> u8 {
        self.memory.read_byte(addr as u32)
    }

    pub(crate) fn write_byte(&mut self, addr: u16, value: u8) {
        self.memory.write_byte(addr as u32, value);
    }

    pub(crate) fn read_word(&mut self, addr: u16) -> u16 {
        let low = self.read_byte(addr) as u16;
        let high = self.read_byte(addr.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    pub(crate) fn write_word(&mut self, addr: u16, value: u16) {
        self.write_byte(addr, value as u8);
        self.write_byte(addr.wrapping_add(1), (value >> 8) as u8);
    }

    // ========== I/O access helpers ==========

    pub(crate) fn read_port(&mut self, port: u16) -> u8 {
        self.io.read_port(port)
    }

    pub(crate) fn write_port(&mut self, port: u16, value: u8) {
        self.io.write_port(port, value);
    }

    pub(crate) fn push(&mut self, value: u16) {
        self.sp = self.sp.wrapping_sub(2);
        self.write_word(self.sp, value);
    }

    pub(crate) fn pop(&mut self) -> u16 {
        let value = self.read_word(self.sp);
        self.sp = self.sp.wrapping_add(2);
        value
    }

    // ========== ALU operations ==========

    pub(crate) fn add_a(&mut self, value: u8, with_carry: bool) {
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
        if result > 0xFF {
            f |= flags::CARRY;
        }
        if half_carry {
            f |= flags::HALF_CARRY;
        }
        if overflow {
            f |= flags::PARITY;
        }
        // ADD_SUB is false (0)

        // Inline set_sz_flags logic
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }
        self.f = f;
    }

    pub(crate) fn sub_a(&mut self, value: u8, with_carry: bool, store: bool) {
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
        if result > 0xFF {
            f |= flags::CARRY;
        }
        if half_carry {
            f |= flags::HALF_CARRY;
        }
        if overflow {
            f |= flags::PARITY;
        }

        let res_u8 = result as u8;
        f |= res_u8 & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if res_u8 == 0 {
            f |= flags::ZERO;
        }
        self.f = f;
    }

    pub(crate) fn and_a(&mut self, value: u8) {
        self.a &= value;

        // H=1, N=0, C=0
        let mut f = flags::HALF_CARRY;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        let parity = self.a.count_ones().is_multiple_of(2);
        if parity {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    pub(crate) fn or_a(&mut self, value: u8) {
        self.a |= value;

        // H=0, N=0, C=0
        let mut f = 0;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        let parity = self.a.count_ones().is_multiple_of(2);
        if parity {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    pub(crate) fn xor_a(&mut self, value: u8) {
        self.a ^= value;

        // H=0, N=0, C=0
        let mut f = 0;

        // S, Z, X, Y
        f |= self.a & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if self.a == 0 {
            f |= flags::ZERO;
        }

        // P
        let parity = self.a.count_ones().is_multiple_of(2);
        if parity {
            f |= flags::PARITY;
        }

        self.f = f;
    }

    pub(crate) fn inc(&mut self, value: u8) -> u8 {
        let result = value.wrapping_add(1);

        let mut f = self.f & flags::CARRY; // Preserve Carry
        if (value & 0x0F) == 0x0F {
            f |= flags::HALF_CARRY;
        }
        if value == 0x7F {
            f |= flags::PARITY;
        }
        // ADD_SUB is false (0)

        f |= result & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if result == 0 {
            f |= flags::ZERO;
        }

        self.f = f;
        result
    }

    pub(crate) fn dec(&mut self, value: u8) -> u8 {
        let result = value.wrapping_sub(1);

        let mut f = (self.f & flags::CARRY) | flags::ADD_SUB; // Preserve Carry, set N
        if (value & 0x0F) == 0x00 {
            f |= flags::HALF_CARRY;
        }
        if value == 0x80 {
            f |= flags::PARITY;
        }

        f |= result & (flags::SIGN | flags::Y_FLAG | flags::X_FLAG);
        if result == 0 {
            f |= flags::ZERO;
        }

        self.f = f;
        result
    }

    pub(crate) fn add_hl(&mut self, value: u16) {
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

    // ========== Rotate/Shift operations ==========

    pub(crate) fn rlca(&mut self) {
        let carry = (self.a & 0x80) != 0;
        self.a = (self.a << 1) | if carry { 1 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    pub(crate) fn rrca(&mut self) {
        let carry = (self.a & 0x01) != 0;
        self.a = (self.a >> 1) | if carry { 0x80 } else { 0 };
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::HALF_CARRY, false);
        self.set_flag(flags::ADD_SUB, false);
        // X/Y from A
        self.set_flag(flags::X_FLAG, (self.a & 0x08) != 0);
        self.set_flag(flags::Y_FLAG, (self.a & 0x20) != 0);
    }

    pub(crate) fn rla(&mut self) {
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

    pub(crate) fn rra(&mut self) {
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

    pub(crate) fn get_reg(&mut self, index: u8) -> u8 {
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

    pub(crate) fn set_reg(&mut self, index: u8, value: u8) {
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

    pub(crate) fn get_rp(&self, index: u8) -> u16 {
        match index {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            3 => self.sp,
            _ => 0,
        }
    }

    pub(crate) fn set_rp(&mut self, index: u8, value: u16) {
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.sp = value,
            _ => {}
        }
    }

    pub(crate) fn get_rp2(&self, index: u8) -> u16 {
        match index {
            0 => self.bc(),
            1 => self.de(),
            2 => self.hl(),
            3 => self.af(),
            _ => 0,
        }
    }

    pub(crate) fn set_rp2(&mut self, index: u8, value: u16) {
        match index {
            0 => self.set_bc(value),
            1 => self.set_de(value),
            2 => self.set_hl(value),
            3 => self.set_af(value),
            _ => {}
        }
    }

    pub(crate) fn check_condition(&self, cc: u8) -> bool {
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
            log::debug!("Z80 | PC:{:04X} OP:{:02X} | A:{:02X} F:{:02X} | BC:{:04X} DE:{:04X} HL:{:04X} SP:{:04X} | CYC:{}", 
                _pc_before, opcode, self.a, self.f, self.bc(), self.de(), self.hl(), self.sp, self.cycles);
        }

        let x = (opcode >> 6) & 0x03;
        let y = (opcode >> 3) & 0x07;
        let z = opcode & 0x07;
        let p = y >> 1;
        let q = y & 1;

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

}

#[derive(Serialize, Deserialize)]
struct Z80State {
    a: Option<u8>,
    f: Option<u8>,
    b: Option<u8>,
    c: Option<u8>,
    d: Option<u8>,
    e: Option<u8>,
    h: Option<u8>,
    l: Option<u8>,
    ix: Option<u16>,
    iy: Option<u16>,
    sp: Option<u16>,
    pc: Option<u16>,
    iff1: Option<bool>,
    iff2: Option<bool>,
    im: Option<u8>,
    halted: Option<bool>,
    cycles: Option<u64>,
}

impl<M: MemoryInterface, I: IoInterface> Debuggable for Z80<M, I> {
    fn read_state(&self) -> Value {
        serde_json::to_value(Z80State {
            a: Some(self.a),
            f: Some(self.f),
            b: Some(self.b),
            c: Some(self.c),
            d: Some(self.d),
            e: Some(self.e),
            h: Some(self.h),
            l: Some(self.l),
            ix: Some(self.ix),
            iy: Some(self.iy),
            sp: Some(self.sp),
            pc: Some(self.pc),
            iff1: Some(self.iff1),
            iff2: Some(self.iff2),
            im: Some(self.im),
            halted: Some(self.halted),
            cycles: Some(self.cycles),
        })
        .unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        let z80_state: Z80State = serde_json::from_value(state.clone()).unwrap_or_else(|_| {
            let default: Z80State = serde_json::from_str("{}").unwrap();
            default
        });

        if let Some(v) = z80_state.a {
            self.a = v;
        }
        if let Some(v) = z80_state.f {
            self.f = v;
        }
        if let Some(v) = z80_state.b {
            self.b = v;
        }
        if let Some(v) = z80_state.c {
            self.c = v;
        }
        if let Some(v) = z80_state.d {
            self.d = v;
        }
        if let Some(v) = z80_state.e {
            self.e = v;
        }
        if let Some(v) = z80_state.h {
            self.h = v;
        }
        if let Some(v) = z80_state.l {
            self.l = v;
        }
        if let Some(v) = z80_state.ix {
            self.ix = v;
        }
        if let Some(v) = z80_state.iy {
            self.iy = v;
        }
        if let Some(v) = z80_state.sp {
            self.sp = v;
        }
        if let Some(v) = z80_state.pc {
            self.pc = v;
        }
        if let Some(v) = z80_state.iff1 {
            self.iff1 = v;
        }
        if let Some(v) = z80_state.iff2 {
            self.iff2 = v;
        }
        if let Some(v) = z80_state.im {
            self.im = v;
        }
        if let Some(v) = z80_state.halted {
            self.halted = v;
        }
        if let Some(v) = z80_state.cycles {
            self.cycles = v;
        }
    }
}

// #[cfg(test)]
// mod tests;

// #[cfg(test)]
// mod tests_alu;

// #[cfg(test)]
// mod tests_cb;

// #[cfg(test)]
// mod tests_control;

// #[cfg(test)]
// mod tests_load;

// #[cfg(test)]
// mod tests_regression;

// #[cfg(test)]
// mod tests_undoc;

// #[cfg(test)]
// mod tests_exhaustive;

// #[cfg(test)]
// mod tests_block;

// #[cfg(test)]
// mod tests_halfcarry;

// #[cfg(test)]
// mod tests_interrupt;

// #[cfg(test)]
// mod tests_reset;

// #[cfg(test)]
// mod tests_rrd_rld;

// #[cfg(test)]
// mod tests_timing;

// #[cfg(test)]
// mod tests_torture;

// #[cfg(test)]
// mod tests_gaps;

// #[cfg(test)]
// mod tests_memptr;

// #[cfg(test)]
// mod tests_ddcb;

// #[cfg(test)]
// mod tests_ex_sp_hl_expanded;
