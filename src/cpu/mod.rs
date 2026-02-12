//! M68k CPU Core
//!
//! This module implements the Motorola 68000 CPU, the main processor
//! of the Sega Mega Drive/Genesis.

pub mod decoder;
pub mod addressing;
pub mod ops;
#[cfg(test)]
mod tests_m68k_extended;
#[cfg(test)]
mod tests_m68k_comprehensive;
#[cfg(test)]
mod tests_m68k_torture;
#[cfg(test)]
mod tests_m68k_alu;
#[cfg(test)]
mod tests_m68k_shift;
#[cfg(test)]
mod tests_m68k_bits;
#[cfg(test)]
mod tests_m68k_control;
#[cfg(test)]
mod tests_bug_fixes;

use crate::memory::MemoryInterface;
use self::decoder::{decode, Instruction, Size, Condition, BitSource};
use self::addressing::{read_ea, write_ea, EffectiveAddress};

/// Status Register flags
pub mod flags {
    pub const CARRY: u16 = 0x0001;      // C - Carry
    pub const OVERFLOW: u16 = 0x0002;   // V - Overflow
    pub const ZERO: u16 = 0x0004;       // Z - Zero
    pub const NEGATIVE: u16 = 0x0008;   // N - Negative
    pub const EXTEND: u16 = 0x0010;     // X - Extend
    pub const INTERRUPT_MASK: u16 = 0x0700; // I2-I0 - Interrupt mask
    pub const SUPERVISOR: u16 = 0x2000; // S - Supervisor mode
    pub const TRACE: u16 = 0x8000;      // T - Trace mode
}

/// Motorola 68000 Central Processing Unit
#[derive(Debug)]
pub struct Cpu {
    // Registers
    pub d: [u32; 8], // Data registers D0-D7
    pub a: [u32; 8], // Address registers A0-A7 (A7 is SP)
    pub pc: u32,     // Program counter
    pub sr: u16,     // Status register

    // Internal state
    pub usp: u32,    // User stack pointer (saved when in supervisor mode)
    pub ssp: u32,    // Supervisor stack pointer (saved when in user mode)

    // Cycle counter for timing
    pub cycles: u64,

    // Halted state
    pub halted: bool,

    // Pending interrupt level (0-7, 0 = none)
    pub pending_interrupt: u8,
    pub pending_exception: bool,
    
    // Interrupt pending bitmask (bit N = level N is pending)
    pub interrupt_pending_mask: u8,
}

impl Cpu {
    pub fn new<M: MemoryInterface>(memory: &mut M) -> Self {
        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc: 0,
            sr: 0x2700, // Supervisor mode, interrupts disabled
            usp: 0,
            ssp: 0,
            cycles: 0,
            halted: false,
            pending_interrupt: 0,
            pending_exception: false,
            interrupt_pending_mask: 0,
        };

        // At startup, the supervisor stack pointer is read from address 0x00000000
        // and the program counter is read from 0x00000004.
        cpu.a[7] = memory.read_long(0x0);
        cpu.ssp = cpu.a[7];
        cpu.pc = memory.read_long(0x4);

        cpu
    }

    /// Reset the CPU to initial state
    pub fn reset<M: MemoryInterface>(&mut self, memory: &mut M) {
        self.d = [0; 8];
        self.a = [0; 8];
        self.sr = 0x2700;
        self.a[7] = memory.read_long(0x0);
        self.ssp = self.a[7];
        self.pc = memory.read_long(0x4);
        self.cycles = 0;
        self.halted = false;
        self.pending_interrupt = 0;
        self.interrupt_pending_mask = 0;
    }

    /// Request an interrupt at the specified level
    /// Uses a bitmask to queue multiple interrupt levels
    pub fn request_interrupt(&mut self, level: u8) {
        if level == 0 || level > 7 { return; }
        // Set the bit for this interrupt level
        self.interrupt_pending_mask |= 1 << level;
        // Update pending_interrupt to highest priority
        self.update_pending_interrupt();
    }
    
    /// Update pending_interrupt to highest priority level from bitmask
    fn update_pending_interrupt(&mut self) {
        // Find highest set bit in mask
        for level in (1..=7).rev() {
            if (self.interrupt_pending_mask & (1 << level)) != 0 {
                self.pending_interrupt = level;
                return;
            }
        }
        self.pending_interrupt = 0;
    }
    
    /// Acknowledge an interrupt (called after processing)
    pub fn acknowledge_interrupt(&mut self, level: u8) {
        if level > 7 { return; }
        // Clear the bit for this interrupt level
        self.interrupt_pending_mask &= !(1 << level);
        // Update to next highest priority
        self.update_pending_interrupt();
    }

    /// Execute a single instruction
    pub fn step_instruction<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        self.pending_exception = false;

        // Handle interrupts before fetching next instruction
        let int_cycles = self.check_interrupts(memory);
        if int_cycles > 0 {
            self.cycles += int_cycles as u64;
            return int_cycles;
        }

        if self.halted {
            return 4; // Minimum cycles when halted
        }

        let pc = self.pc;
        let opcode = self.read_instruction_word(pc, memory);
        if self.pending_exception {
            // Address Error during fetch
            self.cycles += 34;
            return 34;
        }

        self.pc = self.pc.wrapping_add(2);

        let instruction = decode(opcode);
        let mut cycles = self.execute(instruction, memory);

        if self.pending_exception {
            // Instruction was aborted due to an exception (e.g. Address Error)
            // The exception handlers (process_exception) already did the push/pc jump.
            // We just return the cycles for the exception processing.
            cycles = 34;
        }

        self.cycles += cycles as u64;
        cycles
    }

    /// Execute a decoded instruction
    fn execute<M: MemoryInterface>(&mut self, instruction: Instruction, memory: &mut M) -> u32 {
        match instruction {
            // === Data Movement ===
            Instruction::Move { size, src, dst } => ops::data::exec_move(self, size, src, dst, memory),
            Instruction::MoveA { size, src, dst_reg } => ops::data::exec_movea(self, size, src, dst_reg, memory),
            Instruction::MoveQ { dst_reg, data } => ops::data::exec_moveq(self, dst_reg, data),
            Instruction::Lea { src, dst_reg } => ops::data::exec_lea(self, src, dst_reg, memory),
            Instruction::Exg { rx, ry, mode } => ops::data::exec_exg(self, rx, ry, mode),
            Instruction::Clr { size, dst } => ops::data::exec_clr(self, size, dst, memory),
            Instruction::Movep { size, reg, an, direction } => ops::data::exec_movep(self, size, reg, an, direction, memory),

            // === Arithmetic ===
            Instruction::Add { size, src, dst, direction } => ops::arithmetic::exec_add(self, size, src, dst, direction, memory),
            Instruction::AddA { size, src, dst_reg } => ops::arithmetic::exec_adda(self, size, src, dst_reg, memory),
            Instruction::AddI { size, dst } => ops::arithmetic::exec_addi(self, size, dst, memory),
            Instruction::AddQ { size, dst, data } => ops::arithmetic::exec_addq(self, size, dst, data, memory),
            Instruction::Sub { size, src, dst, direction } => ops::arithmetic::exec_sub(self, size, src, dst, direction, memory),
            Instruction::SubA { size, src, dst_reg } => ops::arithmetic::exec_suba(self, size, src, dst_reg, memory),
            Instruction::SubI { size, dst } => ops::arithmetic::exec_subi(self, size, dst, memory),
            Instruction::SubQ { size, dst, data } => ops::arithmetic::exec_subq(self, size, dst, data, memory),

            Instruction::Neg { size, dst } => ops::arithmetic::exec_neg(self, size, dst, memory),
            Instruction::NegX { size, dst } => ops::arithmetic::exec_negx(self, size, dst, memory),
            Instruction::AddX { size, src_reg, dst_reg, memory_mode } => ops::arithmetic::exec_addx(self, size, src_reg, dst_reg, memory_mode, memory),
            Instruction::SubX { size, src_reg, dst_reg, memory_mode } => ops::arithmetic::exec_subx(self, size, src_reg, dst_reg, memory_mode, memory),
            Instruction::MulU { src, dst_reg } => ops::arithmetic::exec_mulu(self, src, dst_reg, memory),
            Instruction::MulS { src, dst_reg } => ops::arithmetic::exec_muls(self, src, dst_reg, memory),
            Instruction::DivU { src, dst_reg } => ops::arithmetic::exec_divu(self, src, dst_reg, memory),
            Instruction::DivS { src, dst_reg } => ops::arithmetic::exec_divs(self, src, dst_reg, memory),
            Instruction::Abcd { src_reg, dst_reg, memory_mode } => ops::arithmetic::exec_abcd(self, src_reg, dst_reg, memory_mode, memory),
            Instruction::Sbcd { src_reg, dst_reg, memory_mode } => ops::arithmetic::exec_sbcd(self, src_reg, dst_reg, memory_mode, memory),
            Instruction::Nbcd { dst } => ops::arithmetic::exec_nbcd(self, dst, memory),

            // === Logical ===
            Instruction::And { size, src, dst, direction } => ops::bits::exec_and(self, size, src, dst, direction, memory),
            Instruction::AndI { size, dst } => ops::bits::exec_andi(self, size, dst, memory),
            Instruction::Or { size, src, dst, direction } => ops::bits::exec_or(self, size, src, dst, direction, memory),
            Instruction::OrI { size, dst } => ops::bits::exec_ori(self, size, dst, memory),
            Instruction::Eor { size, src_reg, dst } => ops::bits::exec_eor(self, size, src_reg, dst, memory),
            Instruction::EorI { size, dst } => ops::bits::exec_eori(self, size, dst, memory),
            Instruction::Not { size, dst } => ops::bits::exec_not(self, size, dst, memory),


            // === Shifts ===
            Instruction::Lsl { size, dst, count } => ops::bits::exec_shift(self, size, dst, count, true, false, memory),
            Instruction::Lsr { size, dst, count } => ops::bits::exec_shift(self, size, dst, count, false, false, memory),
            Instruction::Asl { size, dst, count } => ops::bits::exec_shift(self, size, dst, count, true, true, memory),
            Instruction::Asr { size, dst, count } => ops::bits::exec_shift(self, size, dst, count, false, true, memory),
            Instruction::Rol { size, dst, count } => ops::bits::exec_rotate(self, size, dst, count, true, false, memory),
            Instruction::Ror { size, dst, count } => ops::bits::exec_rotate(self, size, dst, count, false, false, memory),
            Instruction::Roxl { size, dst, count } => ops::bits::exec_roxl(self, size, dst, count, memory),
            Instruction::Roxr { size, dst, count } => ops::bits::exec_roxr(self, size, dst, count, memory),

            // === Bit Manipulation ===
            Instruction::Btst { bit, dst } => ops::bits::exec_btst(self, bit, dst, memory),
            Instruction::Bset { bit, dst } => ops::bits::exec_bset(self, bit, dst, memory),
            Instruction::Bclr { bit, dst } => ops::bits::exec_bclr(self, bit, dst, memory),
            Instruction::Bchg { bit, dst } => ops::bits::exec_bchg(self, bit, dst, memory),

            // === Compare and Test ===
            Instruction::Cmp { size, src, dst_reg } => ops::arithmetic::exec_cmp(self, size, src, dst_reg, memory),
            Instruction::CmpA { size, src, dst_reg } => ops::arithmetic::exec_cmpa(self, size, src, dst_reg, memory),
            Instruction::CmpI { size, dst } => ops::arithmetic::exec_cmpi(self, size, dst, memory),
            Instruction::CmpM { size, ax, ay } => ops::arithmetic::exec_cmpm(self, size, ax, ay, memory),
            Instruction::Tst { size, dst } => ops::arithmetic::exec_tst(self, size, dst, memory),


            // === Branch and Jump ===
            Instruction::Bra { displacement } => ops::system::exec_bra(self, displacement, memory),
            Instruction::Bsr { displacement } => ops::system::exec_bsr(self, displacement, memory),
            Instruction::Bcc { condition, displacement } => ops::system::exec_bcc(self, condition, displacement, memory),
            Instruction::Scc { condition, dst } => ops::system::exec_scc(self, condition, dst, memory),
            Instruction::DBcc { condition, reg } => ops::system::exec_dbcc(self, condition, reg, memory),
            Instruction::Jmp { dst } => ops::system::exec_jmp(self, dst, memory),
            Instruction::Jsr { dst } => ops::system::exec_jsr(self, dst, memory),
            Instruction::Rts => ops::system::exec_rts(self, memory),

            // === Misc ===
            Instruction::Nop => 4,
            Instruction::Swap { reg } => ops::data::exec_swap(self, reg),
            Instruction::Ext { size, reg } => ops::data::exec_ext(self, size, reg),
            
            // === System Control ===
            Instruction::Link { reg } => {
                let displacement = self.read_word(self.pc, memory) as i16;
                self.pc = self.pc.wrapping_add(2);
                ops::system::exec_link(self, reg, displacement, memory)
            },
            Instruction::Unlk { reg } => ops::system::exec_unlk(self, reg, memory),
            Instruction::MoveUsp { reg, to_usp } => ops::system::exec_move_usp(self, reg, to_usp, memory),
            Instruction::Trap { vector } => ops::system::exec_trap(self, vector, memory),
            Instruction::Rte => ops::system::exec_rte(self, memory),
            Instruction::Stop => ops::system::exec_stop(self, memory),
            Instruction::Reset => 132, // Reset external devices, internal state unaffected.
            Instruction::TrapV => {
                if self.get_flag(flags::OVERFLOW) {
                    self.process_exception(7, memory)
                } else {
                    4
                }
            },
            Instruction::Rtr => ops::system::exec_rtr(self, memory),
            
            // === Bounds and Atomic ===
            Instruction::Chk { src, dst_reg } => ops::arithmetic::exec_chk(self, src, dst_reg, memory),
            Instruction::Tas { dst } => ops::bits::exec_tas(self, dst, memory),
            Instruction::Movem { size, direction, mask: _, ea } => ops::data::exec_movem(self, size, direction, ea, memory),
            Instruction::Pea { src } => ops::data::exec_pea(self, src, memory),
            
            // === Status Register Operations ===
            Instruction::MoveToSr { src } => ops::system::exec_move_to_sr(self, src, memory),
            Instruction::MoveFromSr { dst } => ops::system::exec_move_from_sr(self, dst, memory),
            Instruction::MoveToCcr { src } => ops::system::exec_move_to_ccr(self, src, memory),
            Instruction::AndiToCcr => ops::system::exec_andi_to_ccr(self, memory),
            Instruction::AndiToSr => ops::system::exec_andi_to_sr(self, memory),
            Instruction::OriToCcr => ops::system::exec_ori_to_ccr(self, memory),
            Instruction::OriToSr => ops::system::exec_ori_to_sr(self, memory),
            Instruction::EoriToCcr => ops::system::exec_eori_to_ccr(self, memory),
            Instruction::EoriToSr => ops::system::exec_eori_to_sr(self, memory),

            // === Illegal/Unimplemented ===
            Instruction::Illegal => self.process_exception(4, memory),
            Instruction::LineA { opcode: _ } => self.process_exception(10, memory),
            Instruction::LineF { opcode: _ } => self.process_exception(11, memory),
            Instruction::Unimplemented { opcode: _ } => {
                self.process_exception(4, memory)
            }
        }
    }

    // === Flag helpers ===

    pub(crate) fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.sr |= flag;
        } else {
            self.sr &= !flag;
        }
    }

    pub(crate) fn get_flag(&self, flag: u16) -> bool {
        (self.sr & flag) != 0
    }

    pub(crate) fn update_nz_flags(&mut self, value: u32, size: Size) {
        let (negative, zero) = match size {
            Size::Byte => ((value & 0x80) != 0, (value & 0xFF) == 0),
            Size::Word => ((value & 0x8000) != 0, (value & 0xFFFF) == 0),
            Size::Long => ((value & 0x80000000) != 0, value == 0),
        };
        self.set_flag(flags::NEGATIVE, negative);
        self.set_flag(flags::ZERO, zero);
    }

    pub(crate) fn test_condition(&self, condition: Condition) -> bool {
        let c = self.get_flag(flags::CARRY);
        let v = self.get_flag(flags::OVERFLOW);
        let z = self.get_flag(flags::ZERO);
        let n = self.get_flag(flags::NEGATIVE);

        match condition {
            Condition::True => true,
            Condition::False => false,
            Condition::High => !c && !z,
            Condition::LowOrSame => c || z,
            Condition::CarryClear => !c,
            Condition::CarrySet => c,
            Condition::NotEqual => !z,
            Condition::Equal => z,
            Condition::OverflowClear => !v,
            Condition::OverflowSet => v,
            Condition::Plus => !n,
            Condition::Minus => n,
            Condition::GreaterOrEqual => n == v,
            Condition::LessThan => n != v,
            Condition::GreaterThan => !z && (n == v),
            Condition::LessOrEqual => z || (n != v),
        }
    }

    pub(crate) fn add_with_flags(&self, a: u32, b: u32, size: Size) -> (u32, bool, bool) {
        let (mask, sign_bit) = match size {
            Size::Byte => (0xFF, 0x80),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let a = a & mask;
        let b = b & mask;
        let result = a.wrapping_add(b);
        let result_masked = result & mask;

        let carry = result > mask;
        let a_sign = (a & sign_bit) != 0;
        let b_sign = (b & sign_bit) != 0;
        let r_sign = (result_masked & sign_bit) != 0;
        let overflow = (a_sign == b_sign) && (a_sign != r_sign);

        (result_masked, carry, overflow)
    }

    pub(crate) fn sub_with_flags(&self, a: u32, b: u32, size: Size) -> (u32, bool, bool) {
        let (mask, sign_bit) = match size {
            Size::Byte => (0xFF, 0x80),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let a = a & mask;
        let b = b & mask;
        let result = a.wrapping_sub(b);
        let result_masked = result & mask;

        let borrow = b > a;
        let a_sign = (a & sign_bit) != 0;
        let b_sign = (b & sign_bit) != 0;
        let r_sign = (result_masked & sign_bit) != 0;
        let overflow = (a_sign != b_sign) && (a_sign != r_sign);

        (result_masked, borrow, overflow)
    }

    pub(crate) fn addx_with_flags(&self, src: u32, dst: u32, x: u32, size: Size) -> (u32, bool, bool) {
        let (mask, sign_bit) = match size {
            Size::Byte => (0xFF, 0x80),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let a = src & mask;
        let b = dst & mask;
        let res = a.wrapping_add(b).wrapping_add(x);
        let res_masked = res & mask;

        let carry = if size == Size::Long {
            (a as u64 + b as u64 + x as u64) > 0xFFFFFFFF
        } else {
            res > mask
        };
        
        let a_sign = (a & sign_bit) != 0;
        let b_sign = (b & sign_bit) != 0;
        let r_sign = (res_masked & sign_bit) != 0;
        let overflow = (a_sign == b_sign) && (a_sign != r_sign);

        (res_masked, carry, overflow)
    }

    pub(crate) fn subx_with_flags(&self, dst: u32, src: u32, x: u32, size: Size) -> (u32, bool, bool) {
        let (mask, sign_bit) = match size {
            Size::Byte => (0xFF, 0x80),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let a = dst & mask;
        let b = src & mask;
        let res = a.wrapping_sub(b).wrapping_sub(x);
        let res_masked = res & mask;

        let borrow = if size == Size::Long {
            (a as u64) < (b as u64 + x as u64)
        } else {
            a < (b + x)
        };
        
        let a_sign = (a & sign_bit) != 0;
        let b_sign = (b & sign_bit) != 0;
        let r_sign = (res_masked & sign_bit) != 0;
        let overflow = (a_sign != b_sign) && (a_sign != r_sign);

        (res_masked, borrow, overflow)
    }

    pub(crate) fn resolve_bit_index(&self, bit: u8, is_memory: bool) -> u32 {
        if is_memory {
            (bit & 7) as u32
        } else {
            (bit & 31) as u32
        }
    }

    pub(crate) fn fetch_bit_num<M: MemoryInterface>(&mut self, bit: BitSource, memory: &mut M) -> u8 {
        match bit {
            BitSource::Immediate => {
                let val = memory.read_word(self.pc);
                self.pc = self.pc.wrapping_add(2);
                (val & 0xFF) as u8
            }
            BitSource::Register(reg) => self.d[reg as usize] as u8,
        }
    }
    // === Stack Helpers ===
    pub(crate) fn push_long<M: MemoryInterface>(&mut self, val: u32, memory: &mut M) {
        let addr = self.a[7].wrapping_sub(4);
        self.a[7] = addr;
        self.write_long(addr, val, memory);
    }

    pub(crate) fn push_word<M: MemoryInterface>(&mut self, val: u16, memory: &mut M) {
        let addr = self.a[7].wrapping_sub(2);
        self.a[7] = addr;
        self.write_word(addr, val, memory);
    }

    pub(crate) fn pop_long<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        let addr = self.a[7];
        let val = self.read_long(addr, memory);
        self.a[7] = self.a[7].wrapping_add(4);
        val
    }

    pub(crate) fn pop_word<M: MemoryInterface>(&mut self, memory: &mut M) -> u16 {
        let addr = self.a[7];
        let val = self.read_word(addr, memory);
        self.a[7] = self.a[7].wrapping_add(2);
        val
    }

    // === Centralized Memory Access with Alignment Checks ===

    fn read_instruction_word<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u16 {
        if addr % 2 != 0 {
            self.process_exception(3, memory); // Address Error
            return 0;
        }
        memory.read_word(addr)
    }

    pub(crate) fn read_word<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u16 {
        if addr % 2 != 0 {
            self.process_exception(3, memory); // Address Error
            return 0;
        }
        memory.read_word(addr)
    }

    pub(crate) fn read_long<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u32 {
        if addr % 2 != 0 {
            self.process_exception(3, memory); // Address Error
            return 0;
        }
        memory.read_long(addr)
    }

    pub(crate) fn write_word<M: MemoryInterface>(&mut self, addr: u32, val: u16, memory: &mut M) {
        if addr % 2 != 0 {
            self.process_exception(3, memory); // Address Error
            return;
        }
        memory.write_word(addr, val);
    }

    pub(crate) fn write_long<M: MemoryInterface>(&mut self, addr: u32, val: u32, memory: &mut M) {
        if addr % 2 != 0 {
            self.process_exception(3, memory); // Address Error
            return;
        }
        memory.write_long(addr, val);
    }

    // === Centralized Memory and Register Access Helpers ===

    pub(crate) fn cpu_read_memory<M: MemoryInterface>(&mut self, addr: u32, size: Size, memory: &mut M) -> u32 {
        match size {
            Size::Byte => memory.read_byte(addr) as u32,
            Size::Word => self.read_word(addr, memory) as u32,
            Size::Long => self.read_long(addr, memory),
        }
    }

    pub(crate) fn cpu_write_memory<M: MemoryInterface>(&mut self, addr: u32, size: Size, val: u32, memory: &mut M) {
        match size {
            Size::Byte => memory.write_byte(addr, val as u8),
            Size::Word => self.write_word(addr, val as u16, memory),
            Size::Long => self.write_long(addr, val, memory),
        }
    }

    pub(crate) fn write_data_reg(&mut self, reg: u8, size: Size, val: u32) {
        match size {
            Size::Byte => self.d[reg as usize] = (self.d[reg as usize] & !0xFF) | (val & 0xFF),
            Size::Word => self.d[reg as usize] = (self.d[reg as usize] & !0xFFFF) | (val & 0xFFFF),
            Size::Long => self.d[reg as usize] = val,
        }
    }

    pub(crate) fn cpu_read_ea<M: MemoryInterface>(&mut self, ea: EffectiveAddress, size: Size, memory: &mut M) -> u32 {
        if let EffectiveAddress::Memory(addr) = ea {
            if size != Size::Byte && addr % 2 != 0 {
                self.process_exception(3, memory);
                return 0;
            }
        }
        read_ea(ea, size, &self.d, &self.a, memory)
    }

    pub(crate) fn cpu_write_ea<M: MemoryInterface>(&mut self, ea: EffectiveAddress, size: Size, val: u32, memory: &mut M) {
        if let EffectiveAddress::Memory(addr) = ea {
            if size != Size::Byte && addr % 2 != 0 {
                self.process_exception(3, memory);
                return;
            }
        }
        write_ea(ea, size, val, &mut self.d, &mut self.a, memory);
    }

    // === System / Program Control ===

    pub(crate) fn process_exception<M: MemoryInterface>(&mut self, vector: u32, memory: &mut M) -> u32 {
        self.pending_exception = true;
        // Save old SR for pushing
        let old_sr = self.sr_value();

        // Exception processing:
        // 1. Copy SR internally
        // 2. Set Supervisor bit, clear Trace bit
        let mut new_sr = old_sr | flags::SUPERVISOR;
        new_sr &= !flags::TRACE;

        // For interrupts, the mask is set in check_interrupts
        self.set_sr(new_sr);

        // 3. Push PC
        self.push_long(self.pc, memory);

        // 4. Push SR
        self.push_word(old_sr, memory);

        // 5. Fetch vector
        let vector_addr = vector * 4;
        self.pc = memory.read_long(vector_addr);

        // Standard exception processing takes 34+ cycles
        34
    }

    fn check_interrupts<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        if self.pending_interrupt == 0 {
            return 0;
        }

        let current_mask = (self.sr & flags::INTERRUPT_MASK) >> 8;
        
        if self.pending_interrupt > current_mask as u8 || self.pending_interrupt == 7 {
            let level = self.pending_interrupt;
            if level == 6 {
                let f62a = memory.read_byte(0xFFF62A);
                let f605 = memory.read_byte(0xFFF605);
                eprintln!("DEBUG: VInt triggered! PC={:06X} F62A={:02X} F605={:02X}", self.pc, f62a, f605);
            }
            self.acknowledge_interrupt(level); // Use new queuing system
            self.halted = false;       // Wake if halted
            
            // Interrupt Exception Processing
            let old_sr = self.sr_value();
            
            // 1. Save SR
            // 2. S = 1, T = 0
            let mut new_sr = old_sr | flags::SUPERVISOR;
            new_sr &= !flags::TRACE;
            
            // 3. Update interrupt mask to the level being processed
            new_sr = (new_sr & !flags::INTERRUPT_MASK) | ((level as u16) << 8);
            
            self.set_sr(new_sr);
            
            // 4. Push PC
            self.push_long(self.pc, memory);
            
            // 5. Push old SR
            self.push_word(old_sr, memory);
            
            // 6. Fetch vector (Autovectoring: Vector 24+level)
            let vector = 24 + level as u32;
            let vector_addr = vector * 4;
            let handler_pc = memory.read_long(vector_addr);
            eprintln!("DEBUG: Interrupt Level {} Vector {} -> PC={:06X}", level, vector, handler_pc);
            self.pc = handler_pc;
            
            return 44; // Interrupt takes about 44 cycles
        }

        0
    }
    
    pub(crate) fn sr_value(&self) -> u16 {
        // Reconstruct SR from flags and internal state
        self.sr
    }
    
    pub(crate) fn set_sr(&mut self, val: u16) {
        let old_sr = self.sr;
        let new_sr = val;

        // Trace or Supervisor bit change?
        if (old_sr ^ new_sr) & flags::SUPERVISOR != 0 {
            if new_sr & flags::SUPERVISOR != 0 {
                // Switching to Supervisor mode: save USP, load SSP
                self.usp = self.a[7];
                self.a[7] = self.ssp;
            } else {
                // Switching to User mode: save SSP, load USP
                self.ssp = self.a[7];
                self.a[7] = self.usp;
            }
        }

        self.sr = new_sr;
    }


}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;
    use proptest::prelude::*;

    fn create_test_cpu() -> (Cpu, Memory) {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0, 0x1000); // SP
        memory.write_long(4, 0x100);  // PC
        let cpu = Cpu::new(&mut memory);
        (cpu, memory)
    }

    #[test]
    fn initial_state_with_memory() {
        let mut memory = Memory::new(1024);
        memory.data[0] = 0x00;
        memory.data[1] = 0x00;
        memory.data[2] = 0x12;
        memory.data[3] = 0x34;
        memory.data[4] = 0x00;
        memory.data[5] = 0x00;
        memory.data[6] = 0x56;
        memory.data[7] = 0x78;

        let cpu = Cpu::new(&mut memory);

        assert_eq!(cpu.a[7], 0x1234);
        assert_eq!(cpu.pc, 0x5678);
        assert_eq!(cpu.sr, 0x2700);
    }

    #[test]
    fn test_move_l_d1_d0() {
        let mut memory = Memory::new(1024);
        // MOVE.L D1, D0 = 0x2001
        memory.data[8] = 0x20;
        memory.data[9] = 0x01;
        memory.data[0] = 0x00; memory.data[1] = 0x00; memory.data[2] = 0x00; memory.data[3] = 0x00;
        memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

        let mut cpu = Cpu::new(&mut memory);
        cpu.d[1] = 0xABCD1234;

        assert_eq!(cpu.d[0], 0);
        assert_eq!(cpu.pc, 0x00000008);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[0], 0xABCD1234);
        assert_eq!(cpu.pc, 0x0000000A);
    }

    #[test]
    fn test_moveq() {
        let (mut cpu, mut memory) = create_test_cpu();
        // MOVEQ #42, D3 = 0x762A
        memory.write_word(0x100, 0x762A);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[3], 42);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_moveq_negative() {
        let (mut cpu, mut memory) = create_test_cpu();
        // MOVEQ #-1, D0 = 0x70FF
        memory.write_word(0x100, 0x70FF);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[0], 0xFFFFFFFF);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_addq() {
        let (mut cpu, mut memory) = create_test_cpu();
        // ADDQ.L #3, D0 = 0x5680
        memory.write_word(0x100, 0x5680);
        cpu.d[0] = 10;

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[0], 13);
    }

    #[test]
    fn test_subq() {
        let (mut cpu, mut memory) = create_test_cpu();
        // SUBQ.L #3, D0 = 0x5780
        memory.write_word(0x100, 0x5780);
        cpu.d[0] = 10;

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[0], 7);
    }

    #[test]
    fn test_bra() {
        let (mut cpu, mut memory) = create_test_cpu();
        // BRA.S $+10 = 0x6008
        memory.write_word(0x100, 0x6008);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.pc, 0x10A);
    }

    #[test]
    fn test_mul_div() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // MULU D1, D0
        // D0 = 10, D1 = 20
        // Opcode: 1100 000 0 11 000 001 = 0xC0C1
        memory.write_word(0x100, 0xC0C1);
        cpu.d[0] = 10;
        cpu.d[1] = 20;
        
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0], 200);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        
        // MULS D1, D0
        // D0 = 10, D1 = -5 (0xFFFB)
        // Opcode: 1100 000 1 11 000 001 = 0xC1C1
        cpu.pc = 0x102;
        memory.write_word(0x102, 0xC1C1);
        cpu.d[0] = 10;
        cpu.d[1] = 0xFFFB; // -5 as i16
        
        cpu.step_instruction(&mut memory);
        // Result: 10 * -5 = -50 (0xFFFFFFCE)
        assert_eq!(cpu.d[0], 0xFFFFFFCE);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        
        // DIVU D1, D0
        // D0 = 100, D1 = 10
        // Opcode: 1000 000 0 11 000 001 = 0x80C1 (Group 8)
        cpu.pc = 0x104;
        memory.write_word(0x104, 0x80C1);
        cpu.d[0] = 100;
        cpu.d[1] = 10;
        
        cpu.step_instruction(&mut memory);
        // Result: 100 / 10 = 10. Remainder 0.
        // Format: rem:quot = 0000:000A
        assert_eq!(cpu.d[0], 0x0000000A);
    }

    #[test]
    fn test_bcd() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // ABCD D0, D1
        // D0 = 0x45, D1 = 0x33
        // Result should be 0x78
        // Opcode: 1100 001 1 0000 0 000 = 0xC300
        memory.write_word(0x100, 0xC300);
        cpu.d[0] = 0x45;
        cpu.d[1] = 0x33;
        cpu.set_flag(flags::ZERO, true); // Pre-set Z
        cpu.set_flag(flags::EXTEND, false);
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[1] & 0xFF, 0x78);
        assert!(!cpu.get_flag(flags::ZERO)); // Z cleared because result non-zero
        assert!(!cpu.get_flag(flags::EXTEND));
        
        // SBCD D0, D1
        // D0 = 0x33, D1 = 0x78
        // Result 0x78 - 0x33 = 0x45
        // Opcode: 1000 001 1 0000 0 000 = 0x8300
        cpu.pc = 0x102;
        memory.write_word(0x102, 0x8300);
        cpu.d[0] = 0x33;
        cpu.d[1] = 0x78;
        cpu.set_flag(flags::ZERO, true);
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[1] & 0xFF, 0x45);
        assert!(!cpu.get_flag(flags::ZERO));
        
        // NBCD D0
        // D0 = 0x45. 100 - 45 = 55 (0x55).
        // Opcode: 0100 100 0 00 000 000 = 0x4800 (NBCD D0)
        cpu.pc = 0x104;
        memory.write_word(0x104, 0x4800);
        cpu.d[0] = 0x45;
        cpu.set_flag(flags::ZERO, true);
        cpu.set_flag(flags::EXTEND, false);
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[0] & 0xFF, 0x55);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::EXTEND)); // Borrows because 0 - 45
    }

    #[test]
    fn test_exg() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // EXG D0, D1
        // Opcode: 1100 000 1 01000 001 = 0xC141
        // Mode 8 (01000)
        memory.write_word(0x100, 0xC141);
        cpu.d[0] = 0x11111111;
        cpu.d[1] = 0x22222222;
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[0], 0x22222222);
        assert_eq!(cpu.d[1], 0x11111111);
        
        // EXG A0, A1
        // Opcode: 1100 001 1 0100 1 001 = 0xC149
        // Mode 9 (01001)
        cpu.pc = 0x102;
        memory.write_word(0x102, 0xC149);
        cpu.a[0] = 0xAAAA5555;
        cpu.a[1] = 0x5555AAAA;
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.a[0], 0x5555AAAA);
        assert_eq!(cpu.a[1], 0xAAAA5555);
        
        // EXG D0, A0
        // Opcode: 1100 001 1 1000 1 000 = 0xC188 ??
        // Mode 17 (10001) -> 0x11
        // decoder: ((opcode >> 3) & 0x1F)
        // 1 1000 1 -> 1100 001 1 1000 1 000
        // Opcode: C188
        cpu.pc = 0x104;
        memory.write_word(0x104, 0xC188);
        cpu.d[0] = 0xDEADBEEF;
        cpu.a[0] = 0xCAFEBABE;
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[0], 0xCAFEBABE);
        assert_eq!(cpu.a[0], 0xDEADBEEF);
    }
    #[test]
    fn test_beq_taken() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.set_flag(flags::ZERO, true);
        // BEQ.S $+6 = 0x6704
        memory.write_word(0x100, 0x6704);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.pc, 0x106);
    }

    #[test]
    fn test_beq_not_taken() {
        let (mut cpu, mut memory) = create_test_cpu();
        cpu.set_flag(flags::ZERO, false);
        // BEQ.S $+6 = 0x6704
        memory.write_word(0x100, 0x6704);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_nop() {
        let (mut cpu, mut memory) = create_test_cpu();
        // NOP = 0x4E71
        memory.write_word(0x100, 0x4E71);

        let cycles = cpu.step_instruction(&mut memory);

        assert_eq!(cpu.pc, 0x102);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_bit_ops() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // BSET #2, D0
        // D0 = 0
        // Opcode: 0000 100 0 11 000 000 = 0x08C0 (Group 0, BSET immediate)
        // Immediate word: 0x0002
        memory.write_word(0x100, 0x08C0);
        memory.write_word(0x102, 0x0002);
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[0], 0x00000004);
        assert!(cpu.get_flag(flags::ZERO)); // Tested bit 2 was 0
        
        // BCLR #2, D0
        // D0 = 4
        // Opcode: 0000 100 0 10 000 000 = 0x0880 (BCLR immediate)
        // Imm: 0x0002
        cpu.pc = 0x104;
        memory.write_word(0x104, 0x0880);
        memory.write_word(0x106, 0x0002);
        
        cpu.step_instruction(&mut memory);
        
        assert_eq!(cpu.d[0], 0x00000000);
        assert!(!cpu.get_flag(flags::ZERO)); // Tested bit 2 was 1
        
        // BCHG #0, D0
        // D0 = 0
        // Opcode: 0000 100 0 01 000 000 = 0x0840
        cpu.pc = 0x108;
        memory.write_word(0x108, 0x0840);
        memory.write_word(0x10A, 0x0000);
        
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0], 0x00000001);
        
        // BCHG #0, D0
        cpu.pc = 0x10C;
        memory.write_word(0x10C, 0x0840);
        memory.write_word(0x10E, 0x0000);
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0], 0x00000000);
        
        // BTST #5, D0
        // D0 = 0x20 (bit 5)
        cpu.d[0] = 0x20;
        // Opcode: 0000 100 0 00 000 000 = 0x0800
        cpu.pc = 0x110;
        memory.write_word(0x110, 0x0800);
        memory.write_word(0x112, 0x0005);
        
        cpu.step_instruction(&mut memory);
        assert!(!cpu.get_flag(flags::ZERO)); // Bit 5 is 1, so Z=0
    }
    #[test]
    fn test_rts() {
        let (mut cpu, mut memory) = create_test_cpu();
        // Push return address on stack
        cpu.a[7] = 0x0FF0;
        memory.write_long(0x0FF0, 0x200);
        // RTS = 0x4E75
        memory.write_word(0x100, 0x4E75);

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.a[7], 0x0FF4);
    }

    #[test]
    fn test_swap() {
        let (mut cpu, mut memory) = create_test_cpu();
        // SWAP D0 = 0x4840
        memory.write_word(0x100, 0x4840);
        cpu.d[0] = 0x12345678;

        cpu.step_instruction(&mut memory);

        assert_eq!(cpu.d[0], 0x56781234);
    }

    proptest! {
        #[test]
        fn test_move_l_d1_d0_proptest(val in 0..u32::MAX) {
            let mut memory = Memory::new(1024);
            memory.data[8] = 0x20;
            memory.data[9] = 0x01;
            memory.data[0] = 0x00; memory.data[1] = 0x00; memory.data[2] = 0x00; memory.data[3] = 0x00;
            memory.data[4] = 0x00; memory.data[5] = 0x00; memory.data[6] = 0x00; memory.data[7] = 0x08;

            let mut cpu = Cpu::new(&mut memory);
            cpu.d[1] = val;

            cpu.step_instruction(&mut memory);

            assert_eq!(cpu.d[0], val);
        }
    }

    #[test]
    fn test_link_unlk() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // LINK A0, #-4
        // A0 = 0x2000. SP = 0x8000.
        // Opcode: 0100 111 0 01 010 000 = 0x4E50 (LINK A0)
        // Displacement: 0xFFFC (-4)
        memory.write_word(0x100, 0x4E50);
        memory.write_word(0x102, 0xFFFC);
        
        cpu.a[0] = 0x2000;
        cpu.a[7] = 0x8000;
        
        cpu.step_instruction(&mut memory);
        
        // LINK action: 
        // 1. Push A0 -> SP=0x7FFC, Mem[0x7FFC]=0x2000
        // 2. SP -> A0 => A0=0x7FFC
        // 3. SP + Disp -> SP => 0x7FFC - 4 = 0x7FF8.
        
        assert_eq!(memory.read_long(0x7FFC), 0x2000);
        assert_eq!(cpu.a[0], 0x7FFC);
        assert_eq!(cpu.a[7], 0x7FF8);
        assert_eq!(cpu.pc, 0x104);
        
        // UNLK A0
        // Opcode: 0100 111 0 01 011 000 = 0x4E58 (UNLK A0)
        memory.write_word(0x104, 0x4E58);
        
        cpu.step_instruction(&mut memory);
        
        // UNLK action:
        // 1. A0 -> SP => SP=0x7FFC
        // 2. Pop -> A0 => A0=0x2000 (from stack), SP=0x8000
        
        assert_eq!(cpu.a[0], 0x2000);
        assert_eq!(cpu.a[7], 0x8000);
        assert_eq!(cpu.pc, 0x106);
    }

    #[test]
    fn test_trap() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // TRAP #1
        // Opcode: 0100 111 0 01 000 001 = 0x4E41
        memory.write_word(0x100, 0x4E41);
        
        // Initial State
        cpu.pc = 0x100;
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x4000; // USP
        cpu.sr = 0x0700; // Not supervisor
        
        // Set Trap Vector #33 (32+1) -> Address 33*4 = 132 (0x84)
        memory.write_long(0x84, 0x00004000); // Exception handler address
        
        cpu.step_instruction(&mut memory);
        
        // TRAP action:
        // 1. SR pushed (0x0700)
        // 2. Supervisor set (SR | 0x2000) (Actually new SR logic)
        // 3. PC pushed (0x102)
        // 4. Jump to 0x4000
        
        // Stack layout:
        // 0x7FFC: PC (0x102)
        // 0x7FFA: SR (0x0700)
        // SP = 0x7FFA
        
        assert_eq!(cpu.pc, 0x4000);
        assert_eq!(cpu.a[7], 0x7FFA);
        assert_eq!(memory.read_word(0x7FFA), 0x0700);
        assert_eq!(memory.read_long(0x7FFC), 0x102);
        assert_eq!(cpu.sr & 0x2000, 0x2000); // Supervisor Set
    }

    #[test]
    fn test_scc() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // SEQ D0 (Set if Equal: sets D0 to 0xFF if Z=1)
        // Opcode: 0101 0111 11 000 000 = 0x57C0
        memory.write_word(0x100, 0x57C0);
        
        // SNE D1 (Set if Not Equal: sets D1 to 0xFF if Z=0)
        // Opcode: 0101 0110 11 000 001 = 0x56C1
        memory.write_word(0x102, 0x56C1);
        
        cpu.pc = 0x100;
        cpu.set_flag(flags::ZERO, true);
        cpu.d[0] = 0;
        cpu.d[1] = 0;
        
        cpu.step_instruction(&mut memory); // SEQ D0 -> D0 should be 0xFF
        assert_eq!(cpu.d[0] & 0xFF, 0xFF);
        
        cpu.step_instruction(&mut memory); // SNE D1 -> D1 should be 0x00
        assert_eq!(cpu.d[1] & 0xFF, 0x00);
    }

    #[test]
    fn test_movep() {
        let (mut cpu, mut memory) = create_test_cpu();
        
        // MOVEP.W (A0), D0
        // Opcode: 0000 000 1 00 001 000 = 0x0108
        // Displacement: 0x0004
        memory.write_word(0x100, 0x0108);
        memory.write_word(0x102, 0x0004);
        
        // MOVEP.W D1, (A0)
        // Opcode: 0000 001 1 10 001 000 = 0x0388
        // Displacement: 0x0004
        memory.write_word(0x104, 0x0388);
        memory.write_word(0x106, 0x0004);

        cpu.pc = 0x100;
        cpu.a[0] = 0x2000;
        
        // Setup memory for first MOVEP (mem to reg)
        memory.write_byte(0x2004, 0x12);
        memory.write_byte(0x2006, 0x34);
        
        cpu.step_instruction(&mut memory);
        assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
        
        // Setup reg for second MOVEP (reg to mem)
        cpu.d[1] = 0x5678;
        cpu.step_instruction(&mut memory);
        assert_eq!(memory.read_byte(0x2004), 0x56);
        assert_eq!(memory.read_byte(0x2006), 0x78);
    }
}
