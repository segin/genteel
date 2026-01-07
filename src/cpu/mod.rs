//! M68k CPU Core
//!
//! This module implements the Motorola 68000 CPU, the main processor
//! of the Sega Mega Drive/Genesis.

pub mod decoder;
pub mod addressing;
#[cfg(test)]
mod tests_m68k_extended;
#[cfg(test)]
mod tests_m68k_comprehensive;

use crate::memory::MemoryInterface;
use decoder::{decode, Instruction, Size, AddressingMode, Condition, ShiftCount, BitSource};
use addressing::{calculate_ea, read_ea, write_ea, EffectiveAddress};

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

    // Memory interface
    pub memory: Box<dyn MemoryInterface>,

    // Cycle counter for timing
    pub cycles: u64,

    // Halted state
    pub halted: bool,

    // Pending interrupt level (0-7, 0 = none)
    pub pending_interrupt: u8,
    pub pending_exception: bool,
}

impl Cpu {
    pub fn new(memory: Box<dyn MemoryInterface>) -> Self {
        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc: 0,
            sr: 0x2700, // Supervisor mode, interrupts disabled
            usp: 0,
            ssp: 0,
            memory,
            cycles: 0,
            halted: false,
            pending_interrupt: 0,
            pending_exception: false,
        };

        // At startup, the supervisor stack pointer is read from address 0x00000000
        // and the program counter is read from 0x00000004.
        cpu.a[7] = cpu.memory.read_long(0x0);
        cpu.ssp = cpu.a[7];
        cpu.pc = cpu.memory.read_long(0x4);

        cpu
    }

    /// Reset the CPU to initial state
    pub fn reset(&mut self) {
        self.d = [0; 8];
        self.a = [0; 8];
        self.sr = 0x2700;
        self.a[7] = self.memory.read_long(0x0);
        self.ssp = self.a[7];
        self.pc = self.memory.read_long(0x4);
        self.cycles = 0;
        self.halted = false;
    }

    /// Execute a single instruction
    pub fn step_instruction(&mut self) -> u32 {
        self.pending_exception = false;

        // Handle interrupts before fetching next instruction
        let int_cycles = self.check_interrupts();
        if int_cycles > 0 {
            self.cycles += int_cycles as u64;
            return int_cycles;
        }

        if self.halted {
            return 4; // Minimum cycles when halted
        }

        let pc = self.pc;
        let opcode = self.read_instruction_word(pc);
        if self.pending_exception {
            // Address Error during fetch
            self.cycles += 34;
            return 34;
        }

        self.pc = self.pc.wrapping_add(2);

        let instruction = decode(opcode);
        let mut cycles = self.execute(instruction);

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
    fn execute(&mut self, instruction: Instruction) -> u32 {
        match instruction {
            // === Data Movement ===
            Instruction::Move { size, src, dst } => self.exec_move(size, src, dst),
            Instruction::MoveA { size, src, dst_reg } => self.exec_movea(size, src, dst_reg),
            Instruction::MoveQ { dst_reg, data } => self.exec_moveq(dst_reg, data),
            Instruction::Lea { src, dst_reg } => self.exec_lea(src, dst_reg),
            Instruction::Exg { rx, ry, mode } => self.exec_exg(rx, ry, mode),
            Instruction::Clr { size, dst } => self.exec_clr(size, dst),
            Instruction::Movep { size, reg, an, direction } => self.exec_movep(size, reg, an, direction),

            // === Arithmetic ===
            Instruction::Add { size, src, dst, direction } => self.exec_add(size, src, dst, direction),
            Instruction::AddA { size, src, dst_reg } => self.exec_adda(size, src, dst_reg),
            Instruction::AddQ { size, dst, data } => self.exec_addq(size, dst, data),
            Instruction::Sub { size, src, dst, direction } => self.exec_sub(size, src, dst, direction),
            Instruction::SubA { size, src, dst_reg } => self.exec_suba(size, src, dst_reg),
            Instruction::SubQ { size, dst, data } => self.exec_subq(size, dst, data),
            Instruction::Neg { size, dst } => self.exec_neg(size, dst),
            Instruction::NegX { size, dst } => self.exec_negx(size, dst),
            Instruction::AddX { size, src_reg, dst_reg, memory_mode } => self.exec_addx(size, src_reg, dst_reg, memory_mode),
            Instruction::SubX { size, src_reg, dst_reg, memory_mode } => self.exec_subx(size, src_reg, dst_reg, memory_mode),
            Instruction::MulU { src, dst_reg } => self.exec_mulu(src, dst_reg),
            Instruction::MulS { src, dst_reg } => self.exec_muls(src, dst_reg),
            Instruction::DivU { src, dst_reg } => self.exec_divu(src, dst_reg),
            Instruction::DivS { src, dst_reg } => self.exec_divs(src, dst_reg),
            Instruction::Abcd { src_reg, dst_reg, memory_mode } => self.exec_abcd(src_reg, dst_reg, memory_mode),
            Instruction::Sbcd { src_reg, dst_reg, memory_mode } => self.exec_sbcd(src_reg, dst_reg, memory_mode),
            Instruction::Nbcd { dst } => self.exec_nbcd(dst),

            // === Logical ===
            Instruction::And { size, src, dst, direction } => self.exec_and(size, src, dst, direction),
            Instruction::Or { size, src, dst, direction } => self.exec_or(size, src, dst, direction),
            Instruction::Eor { size, src_reg, dst } => self.exec_eor(size, src_reg, dst),
            Instruction::Not { size, dst } => self.exec_not(size, dst),

            // === Shifts ===
            Instruction::Lsl { size, dst, count } => self.exec_shift(size, dst, count, true, false),
            Instruction::Lsr { size, dst, count } => self.exec_shift(size, dst, count, false, false),
            Instruction::Asl { size, dst, count } => self.exec_shift(size, dst, count, true, true),
            Instruction::Asr { size, dst, count } => self.exec_shift(size, dst, count, false, true),
            Instruction::Rol { size, dst, count } => self.exec_rotate(size, dst, count, true, false),
            Instruction::Ror { size, dst, count } => self.exec_rotate(size, dst, count, false, false),
            Instruction::Roxl { size, dst, count } => self.exec_roxl(size, dst, count), // Assuming existed or I should verify?
            Instruction::Roxr { size, dst, count } => self.exec_roxr(size, dst, count),

            // === Bit Manipulation ===
            Instruction::Btst { bit, dst } => self.exec_btst(bit, dst),
            Instruction::Bset { bit, dst } => self.exec_bset(bit, dst),
            Instruction::Bclr { bit, dst } => self.exec_bclr(bit, dst),
            Instruction::Bchg { bit, dst } => self.exec_bchg(bit, dst),

            // === Compare and Test ===
            Instruction::Cmp { size, src, dst_reg } => self.exec_cmp(size, src, dst_reg),
            Instruction::CmpA { size, src, dst_reg } => self.exec_cmpa(size, src, dst_reg),
            Instruction::CmpM { size, ax, ay } => self.exec_cmpm(size, ax, ay),
            Instruction::Tst { size, dst } => self.exec_tst(size, dst),

            // === Branch and Jump ===
            Instruction::Bra { displacement } => self.exec_bra(displacement),
            Instruction::Bsr { displacement } => self.exec_bsr(displacement),
            Instruction::Bcc { condition, displacement } => self.exec_bcc(condition, displacement),
            Instruction::Scc { condition, dst } => self.exec_scc(condition, dst),
            Instruction::DBcc { condition, reg } => self.exec_dbcc(condition, reg),
            Instruction::Jmp { dst } => self.exec_jmp(dst),
            Instruction::Jsr { dst } => self.exec_jsr(dst),
            Instruction::Rts => self.exec_rts(),

            // === Misc ===
            Instruction::Nop => 4,
            Instruction::Swap { reg } => self.exec_swap(reg),
            Instruction::Ext { size, reg } => self.exec_ext(size, reg),
            
            // === System Control ===
            Instruction::Link { reg } => {
                let displacement = self.memory.read_word(self.pc) as i16;
                self.pc = self.pc.wrapping_add(2);
                self.exec_link(reg, displacement)
            },
            Instruction::Unlk { reg } => self.exec_unlk(reg),
            Instruction::MoveUsp { reg, to_usp } => self.exec_move_usp(reg, to_usp),
            Instruction::Trap { vector } => self.exec_trap(vector),
            Instruction::Rte => self.exec_rte(),
            Instruction::Stop => self.exec_stop(),
            Instruction::Reset => 132, // Reset external devices, internal state unaffected.
            Instruction::TrapV => {
                if self.get_flag(flags::OVERFLOW) {
                    self.process_exception(7)
                } else {
                    4
                }
            },
            Instruction::Rtr => self.exec_rtr(),
            
            // === Bounds and Atomic ===
            Instruction::Chk { src, dst_reg } => self.exec_chk(src, dst_reg),
            Instruction::Tas { dst } => self.exec_tas(dst),
            Instruction::Movem { size, direction, mask: _, ea } => self.exec_movem(size, direction, ea),
            Instruction::Pea { src } => self.exec_pea(src),
            
            // === Status Register Operations ===
            Instruction::MoveToSr { src } => self.exec_move_to_sr(src),
            Instruction::MoveFromSr { dst } => self.exec_move_from_sr(dst),
            Instruction::MoveToCcr { src } => self.exec_move_to_ccr(src),
            Instruction::AndiToCcr => self.exec_andi_to_ccr(),
            Instruction::AndiToSr => self.exec_andi_to_sr(),
            Instruction::OriToCcr => self.exec_ori_to_ccr(),
            Instruction::OriToSr => self.exec_ori_to_sr(),
            Instruction::EoriToCcr => self.exec_eori_to_ccr(),
            Instruction::EoriToSr => self.exec_eori_to_sr(),

            // === Illegal/Unimplemented ===
            Instruction::Illegal => self.process_exception(4),
            Instruction::LineA { opcode: _ } => self.process_exception(10),
            Instruction::LineF { opcode: _ } => self.process_exception(11),
            Instruction::Unimplemented { opcode } => {
                #[cfg(debug_assertions)]
                eprintln!("Unimplemented opcode: {:04X} at PC {:08X}", opcode, self.pc.wrapping_sub(2));
                self.process_exception(4)
            }

            // Catch-all for instructions not yet implemented
            _ => 4,
        }
    }

    // === Flag helpers ===

    fn set_flag(&mut self, flag: u16, value: bool) {
        if value {
            self.sr |= flag;
        } else {
            self.sr &= !flag;
        }
    }

    fn get_flag(&self, flag: u16) -> bool {
        (self.sr & flag) != 0
    }

    fn update_nz_flags(&mut self, value: u32, size: Size) {
        let (negative, zero) = match size {
            Size::Byte => ((value & 0x80) != 0, (value & 0xFF) == 0),
            Size::Word => ((value & 0x8000) != 0, (value & 0xFFFF) == 0),
            Size::Long => ((value & 0x80000000) != 0, value == 0),
        };
        self.set_flag(flags::NEGATIVE, negative);
        self.set_flag(flags::ZERO, zero);
    }

    fn test_condition(&self, condition: Condition) -> bool {
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

    // === Instruction implementations ===

    fn exec_move(&mut self, size: Size, src: AddressingMode, dst: AddressingMode) -> u32 {
        let mut cycles = 4u32;

        // Calculate source EA
        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        if self.pending_exception { return cycles; }

        // Read source value
        let value = self.cpu_read_ea(src_ea, size);
        if self.pending_exception { return cycles; }

        // Calculate destination EA
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        if self.pending_exception { return cycles; }

        // Write to destination
        self.cpu_write_ea(dst_ea, size, value);

        // Update flags
        self.update_nz_flags(value, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles
    }

    fn exec_movea(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let value = self.cpu_read_ea(src_ea, size);

        // Sign-extend to 32 bits for word size
        let value = match size {
            Size::Word => (value as i16) as i32 as u32,
            Size::Long => value,
            Size::Byte => value, // Should not happen for MOVEA
        };

        self.a[dst_reg as usize] = value;

        // MOVEA does not affect flags
        cycles
    }

    fn exec_moveq(&mut self, dst_reg: u8, data: i8) -> u32 {
        let value = data as i32 as u32;
        self.d[dst_reg as usize] = value;

        self.update_nz_flags(value, Size::Long);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4
    }

    fn exec_lea(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let (ea, cycles) = calculate_ea(src, Size::Long, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);

        if let EffectiveAddress::Memory(addr) = ea {
            self.a[dst_reg as usize] = addr;
        }

        4 + cycles
    }

    fn exec_clr(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);

        // Handle pre-decrement
        if let AddressingMode::AddressPreDecrement(reg) = dst {
            let dec = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_sub(dec);
        }

        self.cpu_write_ea(dst_ea, size, 0);

        // Handle post-increment
        if let AddressingMode::AddressPostIncrement(reg) = dst {
            let inc = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_add(inc);
        }

        // CLR always sets Z=1, N=0, V=0, C=0
        self.sr = (self.sr & !0x000F) | flags::ZERO;

        4 + cycles
    }

    fn exec_add(&mut self, size: Size, src: AddressingMode, dst: AddressingMode, direction: bool) -> u32 {
        let mut cycles = 4u32;

        // Source is always the EA when direction=false, Dn when direction=true
        let (src_mode, dst_mode) = if direction {
            (AddressingMode::DataRegister(((self.pc.wrapping_sub(2) >> 9) & 7) as u8), dst)
        } else {
            (src, dst)
        };

        let (src_ea, src_cycles) = calculate_ea(src_mode, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        let src_val = self.cpu_read_ea(src_ea, size);

        let (dst_ea, dst_cycles) = calculate_ea(dst_mode, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let (result, carry, overflow) = self.add_with_flags(src_val, dst_val, size);

        self.cpu_write_ea(dst_ea, size, result);

        self.update_nz_flags(result, size);
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::EXTEND, carry);
        self.set_flag(flags::OVERFLOW, overflow);

        cycles
    }

    fn add_with_flags(&self, a: u32, b: u32, size: Size) -> (u32, bool, bool) {
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

    fn exec_adda(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = self.cpu_read_ea(src_ea, size);

        // Sign-extend source to 32 bits
        let src_val = match size {
            Size::Word => (src_val as i16) as i32 as u32,
            Size::Long => src_val,
            Size::Byte => src_val,
        };

        self.a[dst_reg as usize] = self.a[dst_reg as usize].wrapping_add(src_val);

        // ADDA does not affect flags
        cycles + if size == Size::Long { 4 } else { 0 }
    }

    fn exec_addq(&mut self, size: Size, dst: AddressingMode, data: u8) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let (result, carry, overflow) = self.add_with_flags(data as u32, dst_val, size);

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        // ADDQ to An does not affect flags
        if !matches!(dst, AddressingMode::AddressRegister(_)) {
            self.update_nz_flags(result, size);
            self.set_flag(flags::CARRY, carry);
            self.set_flag(flags::EXTEND, carry);
            self.set_flag(flags::OVERFLOW, overflow);
        }

        4 + cycles
    }

    fn exec_sub(&mut self, size: Size, src: AddressingMode, dst: AddressingMode, direction: bool) -> u32 {
        let mut cycles = 4u32;

        let (src_mode, dst_mode) = if direction {
            (AddressingMode::DataRegister(((self.pc.wrapping_sub(2) >> 9) & 7) as u8), dst)
        } else {
            (src, dst)
        };

        let (src_ea, src_cycles) = calculate_ea(src_mode, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        let src_val = self.cpu_read_ea(src_ea, size);

        let (dst_ea, dst_cycles) = calculate_ea(dst_mode, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, src_val, size);

        self.cpu_write_ea(dst_ea, size, result);

        self.update_nz_flags(result, size);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::EXTEND, borrow);
        self.set_flag(flags::OVERFLOW, overflow);

        cycles
    }

    fn sub_with_flags(&self, a: u32, b: u32, size: Size) -> (u32, bool, bool) {
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

    fn exec_addx(&mut self, size: Size, src_reg: u8, dst_reg: u8, memory_mode: bool) -> u32 {
        let mut cycles = match size {
            Size::Byte | Size::Word => 4,
            Size::Long => 8,
        };
        
        let (src_val, dst_val, dst_addr) = if memory_mode {
            let src_addr = self.a[src_reg as usize].wrapping_sub(size.bytes());
            self.a[src_reg as usize] = src_addr;
            let src = self.cpu_read_memory(src_addr, size);
            
            let dst_addr = self.a[dst_reg as usize].wrapping_sub(size.bytes());
            self.a[dst_reg as usize] = dst_addr;
            let dst = self.cpu_read_memory(dst_addr, size);
            
            cycles = match size {
                Size::Byte | Size::Word => 18,
                Size::Long => 30,
            };
            (src, dst, Some(dst_addr))
        } else {
            (self.d[src_reg as usize], self.d[dst_reg as usize], None)
        };
        
        if self.pending_exception { return cycles; }

        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        let (result, carry, overflow) = self.addx_with_flags(src_val, dst_val, x, size);

        if let Some(addr) = dst_addr {
            self.cpu_write_memory(addr, size, result);
        } else {
            self.write_data_reg(dst_reg, size, result);
        }

        let msb = match size {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x80000000,
        };

        self.set_flag(flags::NEGATIVE, (result & msb) != 0);
        if result != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::OVERFLOW, overflow);
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::EXTEND, carry);
        
        cycles
    }

    fn addx_with_flags(&self, src: u32, dst: u32, x: u32, size: Size) -> (u32, bool, bool) {
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

    fn exec_subx(&mut self, size: Size, src_reg: u8, dst_reg: u8, memory_mode: bool) -> u32 {
        let mut cycles = match size {
            Size::Byte | Size::Word => 4,
            Size::Long => 8,
        };
        
        let (src_val, dst_val, dst_addr) = if memory_mode {
            let src_addr = self.a[src_reg as usize].wrapping_sub(size.bytes());
            self.a[src_reg as usize] = src_addr;
            let src = self.cpu_read_memory(src_addr, size);
            
            let dst_addr = self.a[dst_reg as usize].wrapping_sub(size.bytes());
            self.a[dst_reg as usize] = dst_addr;
            let dst = self.cpu_read_memory(dst_addr, size);
            
            cycles = match size {
                Size::Byte | Size::Word => 18,
                Size::Long => 30,
            };
            (src, dst, Some(dst_addr))
        } else {
            (self.d[src_reg as usize], self.d[dst_reg as usize], None)
        };
        
        if self.pending_exception { return cycles; }

        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        let (result, borrow, overflow) = self.subx_with_flags(dst_val, src_val, x, size);

        if let Some(addr) = dst_addr {
            self.cpu_write_memory(addr, size, result);
        } else {
            self.write_data_reg(dst_reg, size, result);
        }

        let msb = match size {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x80000000,
        };

        self.set_flag(flags::NEGATIVE, (result & msb) != 0);
        if result != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::OVERFLOW, overflow);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::EXTEND, borrow);
        
        cycles
    }

    fn subx_with_flags(&self, dst: u32, src: u32, x: u32, size: Size) -> (u32, bool, bool) {
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

    fn exec_negx(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        let (result, borrow, overflow) = self.subx_with_flags(0, dst_val, x, size);

        self.cpu_write_ea(dst_ea, size, result);

        let msb = match size {
            Size::Byte => 0x80,
            Size::Word => 0x8000,
            Size::Long => 0x80000000,
        };

        self.set_flag(flags::NEGATIVE, (result & msb) != 0);
        if result != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::OVERFLOW, overflow);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::EXTEND, borrow);

        cycles + match size {
            Size::Long => 8,
            _ => 4,
        }
    }

    fn exec_cmpm(&mut self, size: Size, ax: u8, ay: u8) -> u32 {
        let ay_addr = self.a[ay as usize];
        let src_val = self.cpu_read_memory(ay_addr, size);
        self.a[ay as usize] = ay_addr.wrapping_add(size.bytes());

        let ax_addr = self.a[ax as usize];
        let dst_val = self.cpu_read_memory(ax_addr, size);
        self.a[ax as usize] = ax_addr.wrapping_add(size.bytes());

        let (_, borrow, overflow) = self.sub_with_flags(dst_val, src_val, size);
        
        let res = dst_val.wrapping_sub(src_val); // Need actual result for NZ bits
        self.update_nz_flags(res, size);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::OVERFLOW, overflow);

        match size {
            Size::Byte | Size::Word => 12,
            Size::Long => 20,
        }
    }

    fn exec_suba(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = read_ea(src_ea, size, &self.d, &self.a, &mut self.memory);

        let src_val = match size {
            Size::Word => (src_val as i16) as i32 as u32,
            Size::Long => src_val,
            Size::Byte => src_val,
        };

        self.a[dst_reg as usize] = self.a[dst_reg as usize].wrapping_sub(src_val);

        cycles + if size == Size::Long { 4 } else { 0 }
    }

    fn exec_subq(&mut self, size: Size, dst: AddressingMode, data: u8) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, data as u32, size);

        self.cpu_write_ea(dst_ea, size, result);

        if !matches!(dst, AddressingMode::AddressRegister(_)) {
            self.update_nz_flags(result, size);
            self.set_flag(flags::CARRY, borrow);
            self.set_flag(flags::EXTEND, borrow);
            self.set_flag(flags::OVERFLOW, overflow);
        }

        4 + cycles
    }

    fn exec_neg(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = self.cpu_read_ea(dst_ea, size);

        let (result, _borrow, overflow) = self.sub_with_flags(0, val, size);

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        self.set_flag(flags::CARRY, val != 0);
        self.set_flag(flags::EXTEND, val != 0);
        self.set_flag(flags::OVERFLOW, overflow);

        4 + cycles
    }

    fn exec_and(&mut self, size: Size, src: AddressingMode, dst: AddressingMode, _direction: bool) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        let src_val = self.cpu_read_ea(src_ea, size);

        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let result = src_val & dst_val;

        self.cpu_write_ea(dst_ea, size, result);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles
    }

    fn exec_or(&mut self, size: Size, src: AddressingMode, dst: AddressingMode, _direction: bool) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        let src_val = self.cpu_read_ea(src_ea, size);

        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let result = src_val | dst_val;

        self.cpu_write_ea(dst_ea, size, result);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles
    }

    fn exec_eor(&mut self, size: Size, src_reg: u8, dst: AddressingMode) -> u32 {
        let src_val = match size {
            Size::Byte => self.d[src_reg as usize] & 0xFF,
            Size::Word => self.d[src_reg as usize] & 0xFFFF,
            Size::Long => self.d[src_reg as usize],
        };

        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let dst_val = self.cpu_read_ea(dst_ea, size);

        let result = src_val ^ dst_val;

        self.cpu_write_ea(dst_ea, size, result);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4 + cycles
    }

    fn exec_not(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &mut self.memory);

        let result = !val;

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4 + cycles
    }

    fn exec_shift(&mut self, size: Size, dst: AddressingMode, count: ShiftCount, left: bool, arithmetic: bool) -> u32 {
        let count_val = match count {
            ShiftCount::Immediate(n) => n as u32,
            ShiftCount::Register(r) => self.d[r as usize] & 63,
        };

        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &mut self.memory);

        let (mask, sign_bit) = match size {
            Size::Byte => (0xFFu32, 0x80u32),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let val = val & mask;
        let mut result = val;
        let mut carry = false;
        let mut overflow = false;

        for _ in 0..count_val {
            if left {
                carry = (result & sign_bit) != 0;
                result = (result << 1) & mask;
                if arithmetic {
                    overflow = overflow || (carry != ((result & sign_bit) != 0));
                }
            } else {
                carry = (result & 1) != 0;
                if arithmetic {
                    // ASR: preserve sign bit
                    let sign = result & sign_bit;
                    result = (result >> 1) | sign;
                } else {
                    result >>= 1;
                }
            }
        }

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        if count_val > 0 {
            self.set_flag(flags::CARRY, carry);
            self.set_flag(flags::EXTEND, carry);
        } else {
            self.set_flag(flags::CARRY, false);
        }
        self.set_flag(flags::OVERFLOW, overflow);

        6 + cycles + 2 * count_val
    }

    fn exec_rotate(&mut self, size: Size, dst: AddressingMode, count: ShiftCount, left: bool, _extend: bool) -> u32 {
        let count_val = match count {
            ShiftCount::Immediate(n) => n as u32,
            ShiftCount::Register(r) => self.d[r as usize] & 63,
        };

        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &mut self.memory);

        let (mask, bits) = match size {
            Size::Byte => (0xFFu32, 8u32),
            Size::Word => (0xFFFF, 16),
            Size::Long => (0xFFFFFFFF, 32),
        };

        let val = val & mask;
        let effective_count = count_val % bits;
        let msb = 1 << (bits - 1);
        let result;
        let mut carry = false;

        if left {
            if effective_count == 0 {
                result = val;
                if count_val > 0 {
                    carry = (val & msb) != 0;
                }
            } else {
                result = ((val << effective_count) | (val >> (bits - effective_count))) & mask;
                carry = ((val >> (bits - effective_count)) & 1) != 0;
            }
        } else {
            if effective_count == 0 {
                result = val;
                if count_val > 0 {
                    carry = (val & 1) != 0;
                }
            } else {
                result = ((val >> effective_count) | (val << (bits - effective_count))) & mask;
                carry = ((val >> (effective_count - 1)) & 1) != 0;
            }
        }

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::OVERFLOW, false);

        6 + cycles + 2 * count_val
    }

    fn exec_cmp(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let (src_ea, cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let src_val = self.cpu_read_ea(src_ea, size);

        let dst_val = match size {
            Size::Byte => self.d[dst_reg as usize] & 0xFF,
            Size::Word => self.d[dst_reg as usize] & 0xFFFF,
            Size::Long => self.d[dst_reg as usize],
        };

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, src_val, size);

        self.update_nz_flags(result, size);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::OVERFLOW, overflow);

        4 + cycles
    }

    fn exec_cmpa(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let (src_ea, cycles) = calculate_ea(src, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &mut self.memory);

        // Sign-extend source to 32 bits
        let src_val = match size {
            Size::Word => (src_val as i16) as i32 as u32,
            Size::Long => src_val,
            Size::Byte => src_val,
        };

        let dst_val = self.a[dst_reg as usize];

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, src_val, Size::Long);

        self.update_nz_flags(result, Size::Long);
        self.set_flag(flags::CARRY, borrow);
        self.set_flag(flags::OVERFLOW, overflow);

        6 + cycles
    }

    fn exec_tst(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = self.cpu_read_ea(dst_ea, size);

        self.update_nz_flags(val, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4 + cycles
    }

    fn exec_bra(&mut self, displacement: i16) -> u32 {
        if displacement == 0 {
            // 16-bit displacement follows
            let disp = self.memory.read_word(self.pc) as i16;
            self.pc = (self.pc as i32 + disp as i32) as u32;
            10
        } else {
            self.pc = (self.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
            10
        }
    }

    fn exec_bsr(&mut self, displacement: i16) -> u32 {
        let return_addr = if displacement == 0 {
            self.pc + 2
        } else {
            self.pc
        };

        // Push return address
        self.a[7] = self.a[7].wrapping_sub(4);
        self.memory.write_long(self.a[7], return_addr);

        if displacement == 0 {
            let disp = self.memory.read_word(self.pc) as i16;
            self.pc = (self.pc as i32 + disp as i32) as u32;
            18
        } else {
            self.pc = (self.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
            18
        }
    }

    fn exec_bcc(&mut self, condition: Condition, displacement: i16) -> u32 {
        if self.test_condition(condition) {
            if displacement == 0 {
                let disp = self.memory.read_word(self.pc) as i16;
                self.pc = (self.pc as i32 + disp as i32) as u32;
                10
            } else {
                self.pc = (self.pc.wrapping_sub(2) as i32 + 2 + displacement as i32) as u32;
                10
            }
        } else {
            if displacement == 0 {
                self.pc = self.pc.wrapping_add(2);
            }
            8
        }
    }

    fn exec_scc(&mut self, condition: Condition, dst: AddressingMode) -> u32 {
        let mut cycles = 4u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, Size::Byte, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        let val = if self.test_condition(condition) { 0xFF } else { 0x00 };
        self.cpu_write_ea(dst_ea, Size::Byte, val);
        
        cycles + if matches!(dst, AddressingMode::DataRegister(_)) { 0 } else { 4 }
    }

    fn exec_dbcc(&mut self, condition: Condition, reg: u8) -> u32 {
        if self.test_condition(condition) {
            self.pc = self.pc.wrapping_add(2); // Skip displacement word
            12
        } else {
            let counter = (self.d[reg as usize] as u16).wrapping_sub(1);
            self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF0000) | counter as u32;

            if counter == 0xFFFF {
                self.pc = self.pc.wrapping_add(2);
                14
            } else {
                let disp = self.memory.read_word(self.pc) as i16;
                self.pc = (self.pc as i32 + disp as i32) as u32;
                10
            }
        }
    }

    fn exec_jmp(&mut self, dst: AddressingMode) -> u32 {
        let (ea, cycles) = calculate_ea(dst, Size::Long, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);

        if let EffectiveAddress::Memory(addr) = ea {
            self.pc = addr;
        }

        4 + cycles
    }

    fn exec_jsr(&mut self, dst: AddressingMode) -> u32 {
        let (ea, cycles) = calculate_ea(dst, Size::Long, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);

        if let EffectiveAddress::Memory(addr) = ea {
            // Push return address
            self.a[7] = self.a[7].wrapping_sub(4);
            self.memory.write_long(self.a[7], self.pc);
            self.pc = addr;
        }

        12 + cycles
    }

    fn exec_rts(&mut self) -> u32 {
        self.pc = self.memory.read_long(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(4);
        16
    }

    fn exec_swap(&mut self, reg: u8) -> u32 {
        let val = self.d[reg as usize];
        let result = (val >> 16) | (val << 16);
        self.d[reg as usize] = result;

        self.update_nz_flags(result, Size::Long);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4
    }

    fn exec_ext(&mut self, size: Size, reg: u8) -> u32 {
        let val = self.d[reg as usize];
        let result = match size {
            Size::Word => (val as i8) as i16 as u32 & 0xFFFF | (val & 0xFFFF0000),
            Size::Long => (val as i16) as i32 as u32,
            Size::Byte => val, // Should not happen
        };
        self.d[reg as usize] = result;

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4
    }

    fn exec_mulu(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = self.cpu_read_ea(src_ea, Size::Word) as u16;
        let dst_val = self.d[dst_reg as usize] as u16;

        let result = (src_val as u32) * (dst_val as u32);
        self.d[dst_reg as usize] = result;

        self.update_nz_flags(result, Size::Long);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles + 70
    }

    fn exec_muls(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = read_ea(src_ea, Size::Word, &self.d, &self.a, &mut self.memory) as i16;
        let dst_val = self.d[dst_reg as usize] as i16;

        let result = (src_val as i32) * (dst_val as i32);
        self.d[dst_reg as usize] = result as u32;

        self.update_nz_flags(result as u32, Size::Long);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles + 70
    }

    fn exec_divu(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = self.cpu_read_ea(src_ea, Size::Word) as u16;
        
        if src_val == 0 {
            // Divide by zero trap
            #[cfg(debug_assertions)]
            eprintln!("TRAP 5: Division by zero at PC={:08X}", self.pc);
            return cycles + 38;
        }

        let dst_val = self.d[dst_reg as usize];
        let quotient = dst_val / (src_val as u32);
        let remainder = dst_val % (src_val as u32);

        if quotient > 0xFFFF {
            self.set_flag(flags::OVERFLOW, true);
            self.set_flag(flags::CARRY, false);
            return cycles + 10;
        }

        let result = (remainder << 16) | quotient;
        self.d[dst_reg as usize] = result;

        let n = (quotient & 0x8000) != 0; 
        self.set_flag(flags::NEGATIVE, n);
        self.set_flag(flags::ZERO, quotient == 0);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles + 140
    }

    fn exec_divs(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;

        let src_val = read_ea(src_ea, Size::Word, &self.d, &self.a, &mut self.memory) as i16;
        
        if src_val == 0 {
            #[cfg(debug_assertions)]
            eprintln!("TRAP 5: Division by zero at PC={:08X}", self.pc);
            return cycles + 38;
        }

        let dst_val = self.d[dst_reg as usize] as i32;
        let quotient = dst_val / (src_val as i32);
        let remainder = dst_val % (src_val as i32);

        if quotient > 32767 || quotient < -32768 {
            self.set_flag(flags::OVERFLOW, true);
            self.set_flag(flags::CARRY, false);
            return cycles + 10;
        }

        let result = ((remainder as u32 & 0xFFFF) << 16) | (quotient as u32 & 0xFFFF);
        self.d[dst_reg as usize] = result;

        self.set_flag(flags::NEGATIVE, quotient < 0);
        self.set_flag(flags::ZERO, quotient == 0);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles + 158
    }

    fn exec_abcd(&mut self, src_reg: u8, dst_reg: u8, memory_mode: bool) -> u32 {
        let mut cycles = 6u32;
        
        let (src_val, dst_val, dst_addr) = if memory_mode {
            let src_addr = self.a[src_reg as usize].wrapping_sub(1);
            self.a[src_reg as usize] = src_addr;
            let src = self.memory.read_byte(src_addr);
            
            let dst_addr = self.a[dst_reg as usize].wrapping_sub(1);
            self.a[dst_reg as usize] = dst_addr;
            let dst = self.memory.read_byte(dst_addr);
            
            cycles += 12;
            (src, dst, Some(dst_addr))
        } else {
            (self.d[src_reg as usize] as u8, self.d[dst_reg as usize] as u8, None)
        };

        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        
        let mut tmp = (src_val & 0x0F) as u16 + (dst_val & 0x0F) as u16 + x as u16;
        if tmp > 9 { tmp += 6; }
        tmp += (src_val & 0xF0) as u16 + (dst_val & 0xF0) as u16;
        
        let carry = tmp > 0x99;
        if carry { tmp += 0x60; }
        
        let res = (tmp & 0xFF) as u8;

        if let Some(addr) = dst_addr {
            self.memory.write_byte(addr, res);
        } else {
            self.d[dst_reg as usize] = (self.d[dst_reg as usize] & 0xFFFFFF00) | res as u32;
        }

        if res != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::EXTEND, carry);
        self.set_flag(flags::NEGATIVE, (res & 0x80) != 0); 
        self.set_flag(flags::OVERFLOW, false); 
        
        cycles
    }

    fn exec_sbcd(&mut self, src_reg: u8, dst_reg: u8, memory_mode: bool) -> u32 {
        let mut cycles = 6u32;
        
        let (src_val, dst_val, dst_addr) = if memory_mode {
            let src_addr = self.a[src_reg as usize].wrapping_sub(1);
            self.a[src_reg as usize] = src_addr;
            let src = self.memory.read_byte(src_addr);
            
            let dst_addr = self.a[dst_reg as usize].wrapping_sub(1);
            self.a[dst_reg as usize] = dst_addr;
            let dst = self.memory.read_byte(dst_addr);
            
            cycles += 12;
            (src, dst, Some(dst_addr))
        } else {
            (self.d[src_reg as usize] as u8, self.d[dst_reg as usize] as u8, None)
        };

        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        
        let mut tmp = (dst_val & 0x0F) as i16 - (src_val & 0x0F) as i16 - x as i16;
        if tmp < 0 { tmp -= 6; }
        tmp += (dst_val & 0xF0) as i16 - (src_val & 0xF0) as i16;
        
        let carry = tmp < 0;
        if carry { tmp -= 0x60; }
        
        let res = (tmp & 0xFF) as u8;

        if let Some(addr) = dst_addr {
            self.memory.write_byte(addr, res);
        } else {
            self.d[dst_reg as usize] = (self.d[dst_reg as usize] & 0xFFFFFF00) | res as u32;
        }

        if res != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::EXTEND, carry);
        self.set_flag(flags::NEGATIVE, (res & 0x80) != 0); 
        self.set_flag(flags::OVERFLOW, false); 
        
        cycles
    }

    fn exec_nbcd(&mut self, dst: AddressingMode) -> u32 {
        let mut cycles = 6u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, Size::Byte, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        let dst_val = self.cpu_read_ea(dst_ea, Size::Byte) as u8;
        let x = if self.get_flag(flags::EXTEND) { 1 } else { 0 };
        
        let mut tmp = 0 - (dst_val & 0x0F) as i16 - x as i16;
        if tmp < 0 { tmp -= 6; }
        tmp += 0 - (dst_val & 0xF0) as i16;
        
        let carry = tmp < 0;
        if carry { tmp -= 0x60; }
        
        let res = (tmp & 0xFF) as u8;
        
        self.cpu_write_ea(dst_ea, Size::Byte, res as u32);
        
        if res != 0 {
            self.set_flag(flags::ZERO, false);
        }
        self.set_flag(flags::CARRY, carry);
        self.set_flag(flags::EXTEND, carry);
        self.set_flag(flags::NEGATIVE, (res & 0x80) != 0);
        self.set_flag(flags::OVERFLOW, false);
        
        cycles
    }

    fn exec_exg(&mut self, rx: u8, ry: u8, mode: u8) -> u32 {
        // Mode comes from bits 3-7 of opcode.
        // 01000 (8): Dx, Dy
        // 01001 (9): Ax, Ay
        // 10001 (17): Dx, Ay
        
        match mode {
            0x08 => { // Dx, Dy
                let tmp = self.d[rx as usize];
                self.d[rx as usize] = self.d[ry as usize];
                self.d[ry as usize] = tmp;
            }
            0x09 => { // Ax, Ay
                let tmp = self.a[rx as usize];
                self.a[rx as usize] = self.a[ry as usize];
                self.a[ry as usize] = tmp;
            }
            0x11 => { // Dx, Ay
                let tmp = self.d[rx as usize];
                self.d[rx as usize] = self.a[ry as usize];
                self.a[ry as usize] = tmp;
            }
            _ => {
                // Should not happen if decoder is correct
                #[cfg(debug_assertions)]
                 eprintln!("Invalid EXG mode: {:02X}", mode);
            }
        }
        
        6 
    }

    fn resolve_bit_index(&self, bit: u8, is_memory: bool) -> u32 {
        if is_memory {
            (bit & 7) as u32
        } else {
            (bit & 31) as u32
        }
    }

    fn fetch_bit_num(&mut self, bit: BitSource) -> u8 {
        match bit {
            BitSource::Immediate => {
                let val = self.memory.read_word(self.pc);
                self.pc = self.pc.wrapping_add(2);
                (val & 0xFF) as u8
            }
            BitSource::Register(reg) => self.d[reg as usize] as u8,
        }
    }

    fn exec_btst(&mut self, bit: BitSource, dst: AddressingMode) -> u32 {
        let bit_num = self.fetch_bit_num(bit);
        let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
        let size = if is_memory { Size::Byte } else { Size::Long };
        
        let mut cycles = 4u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        let val = if matches!(dst, AddressingMode::Immediate) {
            // Immediate data for BTST is valid? No, destination EA.
            // BTST #n, #m is not valid.
            // But BTST #n, (xxx) is.
            self.cpu_read_ea(dst_ea, size)
        } else {
             self.cpu_read_ea(dst_ea, size)
        };
        
        // Immediate allowed for BTST?
        // Check manual: Destination <ea> Data.
        // Data addressing modes: Dn, (An), (An)+, -(An), d(An), ...
        // Immediate is NOT data addressing mode except source.
        // But read_ea handles it.
        
        let bit_idx = self.resolve_bit_index(bit_num, is_memory);
        let bit_val = (val >> bit_idx) & 1;
        
        self.set_flag(flags::ZERO, bit_val == 0);
        
        if is_memory { cycles += 4; } else { cycles += 6; } // Timing approx
        cycles
    }

    fn exec_bset(&mut self, bit: BitSource, dst: AddressingMode) -> u32 {
        let bit_num = self.fetch_bit_num(bit);
        let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
        let size = if is_memory { Size::Byte } else { Size::Long };
        
        let mut cycles = 8u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        let val = self.cpu_read_ea(dst_ea, size);
        let bit_idx = self.resolve_bit_index(bit_num, is_memory);
        let bit_val = (val >> bit_idx) & 1;
        
        self.set_flag(flags::ZERO, bit_val == 0);
        
        let new_val = val | (1 << bit_idx);
        self.cpu_write_ea(dst_ea, size, new_val);
        
        cycles
    }

    fn exec_bclr(&mut self, bit: BitSource, dst: AddressingMode) -> u32 {
        let bit_num = self.fetch_bit_num(bit);
        let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
        let size = if is_memory { Size::Byte } else { Size::Long };

        let mut cycles = 8u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;

        let val = self.cpu_read_ea(dst_ea, size);
        let bit_idx = self.resolve_bit_index(bit_num, is_memory);
        let bit_val = (val >> bit_idx) & 1;

        self.set_flag(flags::ZERO, bit_val == 0);

        let new_val = val & !(1 << bit_idx);
        self.cpu_write_ea(dst_ea, size, new_val);

        cycles
    }

    fn exec_bchg(&mut self, bit: BitSource, dst: AddressingMode) -> u32 {
        let bit_num = self.fetch_bit_num(bit);
        let is_memory = !matches!(dst, AddressingMode::DataRegister(_));
        let size = if is_memory { Size::Byte } else { Size::Long };

        let mut cycles = 8u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;

        let val = self.cpu_read_ea(dst_ea, size);
        let bit_idx = self.resolve_bit_index(bit_num, is_memory);
        let bit_val = (val >> bit_idx) & 1;

        self.set_flag(flags::ZERO, bit_val == 0);

        let new_val = val ^ (1 << bit_idx);
        self.cpu_write_ea(dst_ea, size, new_val);

        cycles
    }
    
    // Missing ROXL/ROXR stubs to satisfy match arms if I added them
    fn exec_roxl(&mut self, size: Size, dst: AddressingMode, count: ShiftCount) -> u32 {
        let count_val = match count {
            ShiftCount::Immediate(n) => n as u32,
            ShiftCount::Register(r) => self.d[r as usize] & 63,
        };

        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = self.cpu_read_ea(dst_ea, size);

        let (mask, msb) = match size {
            Size::Byte => (0xFFu32, 0x80u32),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let mut res = val & mask;
        let mut x = self.get_flag(flags::EXTEND);
        let mut last_carry = x;

        for _ in 0..count_val {
            let next_x = (res & msb) != 0;
            res = ((res << 1) | (if x { 1 } else { 0 })) & mask;
            x = next_x;
            last_carry = x;
        }

        self.cpu_write_ea(dst_ea, size, res);
        self.update_nz_flags(res, size);
        self.set_flag(flags::OVERFLOW, false);
        if count_val > 0 {
            self.set_flag(flags::CARRY, last_carry);
            self.set_flag(flags::EXTEND, last_carry);
        } else {
            self.set_flag(flags::CARRY, self.get_flag(flags::EXTEND));
        }

        cycles + 6 + 2 * count_val
    }

    fn exec_roxr(&mut self, size: Size, dst: AddressingMode, count: ShiftCount) -> u32 {
        let count_val = match count {
            ShiftCount::Immediate(n) => n as u32,
            ShiftCount::Register(r) => self.d[r as usize] & 63,
        };

        let (dst_ea, cycles) = calculate_ea(dst, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        let val = self.cpu_read_ea(dst_ea, size);

        let (mask, msb) = match size {
            Size::Byte => (0xFFu32, 0x80u32),
            Size::Word => (0xFFFF, 0x8000),
            Size::Long => (0xFFFFFFFF, 0x80000000),
        };

        let mut res = val & mask;
        let mut x = self.get_flag(flags::EXTEND);
        let mut last_carry = x;

        for _ in 0..count_val {
            let next_x = (res & 1) != 0;
            res = (res >> 1) | (if x { msb } else { 0 });
            x = next_x;
            last_carry = x;
        }

        self.cpu_write_ea(dst_ea, size, res);
        self.update_nz_flags(res, size);
        self.set_flag(flags::OVERFLOW, false);
        if count_val > 0 {
            self.set_flag(flags::CARRY, last_carry);
            self.set_flag(flags::EXTEND, last_carry);
        } else {
            self.set_flag(flags::CARRY, self.get_flag(flags::EXTEND));
        }

        cycles + 6 + 2 * count_val
    }


    // === Stack Helpers ===
    fn push_long(&mut self, val: u32) {
        let addr = self.a[7].wrapping_sub(4);
        self.a[7] = addr;
        self.write_long(addr, val);
    }
    
    fn push_word(&mut self, val: u16) {
        let addr = self.a[7].wrapping_sub(2);
        self.a[7] = addr;
        self.write_word(addr, val);
    }
    
    fn pop_long(&mut self) -> u32 {
        let addr = self.a[7];
        let val = self.read_long(addr);
        self.a[7] = self.a[7].wrapping_add(4);
        val
    }

    fn pop_word(&mut self) -> u16 {
        let addr = self.a[7];
        let val = self.read_word(addr);
        self.a[7] = self.a[7].wrapping_add(2);
        val
    }

    // === Centralized Memory Access with Alignment Checks ===
    
    fn read_instruction_word(&mut self, addr: u32) -> u16 {
        if addr % 2 != 0 {
            self.process_exception(3); // Address Error
            return 0;
        }
        self.memory.read_word(addr)
    }

    fn read_word(&mut self, addr: u32) -> u16 {
        if addr % 2 != 0 {
            self.process_exception(3); // Address Error
            return 0;
        }
        self.memory.read_word(addr)
    }

    fn read_long(&mut self, addr: u32) -> u32 {
        if addr % 2 != 0 {
            self.process_exception(3); // Address Error
            return 0;
        }
        self.memory.read_long(addr)
    }

    fn write_word(&mut self, addr: u32, val: u16) {
        if addr % 2 != 0 {
            self.process_exception(3); // Address Error
            return;
        }
        self.memory.write_word(addr, val);
    }

    fn write_long(&mut self, addr: u32, val: u32) {
        if addr % 2 != 0 {
            self.process_exception(3); // Address Error
            return;
        }
        self.memory.write_long(addr, val);
    }

    // === Centralized Memory and Register Access Helpers ===

    fn cpu_read_memory(&mut self, addr: u32, size: Size) -> u32 {
        match size {
            Size::Byte => self.memory.read_byte(addr) as u32,
            Size::Word => self.read_word(addr) as u32,
            Size::Long => self.read_long(addr),
        }
    }

    fn cpu_write_memory(&mut self, addr: u32, size: Size, val: u32) {
        match size {
            Size::Byte => self.memory.write_byte(addr, val as u8),
            Size::Word => self.write_word(addr, val as u16),
            Size::Long => self.write_long(addr, val),
        }
    }

    fn write_data_reg(&mut self, reg: u8, size: Size, val: u32) {
        match size {
            Size::Byte => self.d[reg as usize] = (self.d[reg as usize] & !0xFF) | (val & 0xFF),
            Size::Word => self.d[reg as usize] = (self.d[reg as usize] & !0xFFFF) | (val & 0xFFFF),
            Size::Long => self.d[reg as usize] = val,
        }
    }

    fn cpu_read_ea(&mut self, ea: EffectiveAddress, size: Size) -> u32 {
        if let EffectiveAddress::Memory(addr) = ea {
            if size != Size::Byte && addr % 2 != 0 {
                self.process_exception(3);
                return 0;
            }
        }
        read_ea(ea, size, &self.d, &self.a, &mut self.memory)
    }

    fn cpu_write_ea(&mut self, ea: EffectiveAddress, size: Size, val: u32) {
        if let EffectiveAddress::Memory(addr) = ea {
            if size != Size::Byte && addr % 2 != 0 {
                self.process_exception(3);
                return;
            }
        }
        write_ea(ea, size, val, &mut self.d, &mut self.a, &mut self.memory);
    }
    
    // === System / Program Control ===
    
    fn exec_link(&mut self, reg: u8, displacement: i16) -> u32 {
        let old_an = self.a[reg as usize];
        self.push_long(old_an);
        self.a[reg as usize] = self.a[7];
        self.a[7] = self.a[7].wrapping_add(displacement as u32);
        16
    }
    
    fn exec_unlk(&mut self, reg: u8) -> u32 {
        self.a[7] = self.a[reg as usize];
        let old_an = self.pop_long();
        self.a[reg as usize] = old_an;
        12
    }
    
    fn exec_trap(&mut self, vector: u8) -> u32 {
        // TRAP #n uses vectors 32-47 (0x20-0x2F).
        self.process_exception(32 + vector as u32)
    }
    
    fn exec_rte(&mut self) -> u32 {
        if (self.sr & 0x2000) == 0 {
            // Not supervisor
            return self.process_exception(8); // Privilege Violation
        }
        
        let new_sr = self.pop_word();
        let new_pc = self.pop_long();
        
        self.set_sr(new_sr);
        self.pc = new_pc;
        
        20 
    }
    
    fn exec_stop(&mut self) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8);
        }
        
        let imm = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.set_sr(imm);
        self.halted = true; // STOP stops the processor until interrupt/reset.
        // In emulator, we might just set a flag.
        // For now, halted = true is close, but interrupts should wake it.
        // We'll leave it as halted.
        4
    }

    fn exec_move_usp(&mut self, reg: u8, to_usp: bool) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8); // Privilege violation
        }
        if to_usp {
            self.usp = self.a[reg as usize];
        } else {
            self.a[reg as usize] = self.usp;
        }
        4
    }
    
    fn process_exception(&mut self, vector: u32) -> u32 {
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
        self.push_long(self.pc);
        
        // 4. Push SR
        self.push_word(old_sr);
        
        // 5. Fetch vector
        let vector_addr = vector * 4;
        self.pc = self.memory.read_long(vector_addr);
        
        // Standard exception processing takes 34+ cycles
        34
    }

    fn check_interrupts(&mut self) -> u32 {
        if self.pending_interrupt == 0 {
            return 0;
        }

        let current_mask = (self.sr & flags::INTERRUPT_MASK) >> 8;
        
        // Level 7 is non-maskable (NMI)
        if self.pending_interrupt > current_mask as u8 || self.pending_interrupt == 7 {
            let level = self.pending_interrupt;
            self.pending_interrupt = 0; // Clear pending
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
            self.push_long(self.pc);
            
            // 5. Push old SR
            self.push_word(old_sr);
            
            // 6. Fetch vector (Autovectoring: Vector 24+level)
            let vector = 24 + level as u32;
            let vector_addr = vector * 4;
            self.pc = self.memory.read_long(vector_addr);
            
            return 44; // Interrupt takes about 44 cycles
        }

        0
    }
    
    fn sr_value(&self) -> u16 {
        // Reconstruct SR from flags and internal state
        // Currently self.sr holds it? 
        // Mod.rs fields:
        // status register:
        // pub d: [u32; 8],
        // pub a: [u32; 8],
        // pub pc: u32,
        // pub sr: u16, // Stores full SR?
        self.sr
    }
    
    fn set_sr(&mut self, val: u16) {
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

    // === CHK - Check Register Against Bounds ===
    fn exec_chk(&mut self, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 10u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        
        let bound = self.cpu_read_ea(src_ea, Size::Word) as i16;
        let dn = (self.d[dst_reg as usize] & 0xFFFF) as i16;
        
        if dn < 0 {
            self.set_flag(flags::NEGATIVE, true);
            return self.process_exception(6); // CHK exception
        }
        if dn > bound {
            self.set_flag(flags::NEGATIVE, false);
            return self.process_exception(6);
        }
        
        cycles
    }

    // === TAS - Test and Set (Atomic) ===
    fn exec_tas(&mut self, dst: AddressingMode) -> u32 {
        let mut cycles = 4u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, Size::Byte, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        let val = self.cpu_read_ea(dst_ea, Size::Byte) as u8;
        
        // Set flags based on original value
        self.set_flag(flags::NEGATIVE, (val & 0x80) != 0);
        self.set_flag(flags::ZERO, val == 0);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);
        
        // Set high bit (atomically on real hardware)
        let new_val = val | 0x80;
        self.cpu_write_ea(dst_ea, Size::Byte, new_val as u32);
        
        cycles + 4
    }

    // === MOVEM - Move Multiple Registers ===
    fn exec_movem(&mut self, size: Size, to_memory: bool, ea: AddressingMode) -> u32 {
        let mask = self.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        
        let reg_size: u32 = if size == Size::Word { 2 } else { 4 };
        let mut cycles = 8u32;
        
        let base_addr = match ea {
            AddressingMode::AddressPostIncrement(reg) => {
                let addr = self.a[reg as usize];
                cycles += 4; // Cycles for (An)+
                addr
            }
            AddressingMode::AddressPreDecrement(reg) => {
                let addr = self.a[reg as usize];
                cycles += 6; // Cycles for -(An)
                addr
            }
            _ => {
                let (ea_result, ea_cycles) = calculate_ea(ea, size, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
                cycles += ea_cycles;
                match ea_result {
                    EffectiveAddress::Memory(addr) => addr,
                    _ => return cycles, // Invalid for MOVEM
                }
            }
        };
        
        if to_memory {
            // Registers to Memory
            let is_predec = matches!(ea, AddressingMode::AddressPreDecrement(_));
            let mut addr = base_addr;
            
            if is_predec {
                // Predecrement: Store A7-A0, then D7-D0 (reverse order)
                for i in (0..16).rev() {
                    if (mask & (1 << (15 - i))) != 0 {
                        addr = addr.wrapping_sub(reg_size);
                        let val = if i < 8 { self.d[i] } else { self.a[i - 8] };
                        if size == Size::Word {
                            self.write_word(addr, val as u16);
                        } else {
                            self.write_long(addr, val);
                        }
                        cycles += if size == Size::Word { 4 } else { 8 };
                    }
                }
                // Update An for predecrement mode
                if let AddressingMode::AddressPreDecrement(reg) = ea {
                    self.a[reg as usize] = addr;
                }
            } else {
                // Normal: Store D0-D7, then A0-A7
                for i in 0..16 {
                    if (mask & (1 << i)) != 0 {
                        let val = if i < 8 { self.d[i] } else { self.a[i - 8] };
                        if size == Size::Word {
                            self.write_word(addr, val as u16);
                        } else {
                            self.write_long(addr, val);
                        }
                        addr = addr.wrapping_add(reg_size);
                        cycles += if size == Size::Word { 4 } else { 8 };
                    }
                }
            }
        } else {
            // Memory to Registers
            let mut addr = base_addr;
            
            for i in 0..16 {
                if (mask & (1 << i)) != 0 {
                    let val = if size == Size::Word {
                        self.read_word(addr) as i16 as i32 as u32 // Sign extend
                    } else {
                        self.read_long(addr)
                    };
                    
                    if i < 8 {
                        self.d[i] = val;
                    } else {
                        self.a[i - 8] = val;
                    }
                    addr = addr.wrapping_add(reg_size);
                    cycles += if size == Size::Word { 4 } else { 8 };
                }
            }
            
            // Update An for postincrement mode
            if let AddressingMode::AddressPostIncrement(reg) = ea {
                self.a[reg as usize] = addr;
            }
        }
        
        cycles
    }

    // === PEA - Push Effective Address ===
    fn exec_pea(&mut self, src: AddressingMode) -> u32 {
        let mut cycles = 12u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Long, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        
        let addr = match src_ea {
            EffectiveAddress::Memory(a) => a,
            _ => 0, // Should not happen for control addressing modes
        };
        
        self.push_long(addr);
        cycles
    }

    // === RTR - Return and Restore CCR ===
    fn exec_rtr(&mut self) -> u32 {
        let ccr = self.pop_word();
        let new_pc = self.pop_long();
        
        // Only restore lower 5 bits (CCR portion)
        self.sr = (self.sr & 0xFF00) | (ccr & 0x00FF);
        self.pc = new_pc;
        
        20
    }

    // === Status Register Operations ===
    
    fn exec_move_to_sr(&mut self, src: AddressingMode) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8); // Privilege violation
        }
        
        let mut cycles = 12u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        
        let val = self.cpu_read_ea(src_ea, Size::Word) as u16;
        self.set_sr(val);
        cycles
    }

    fn exec_move_from_sr(&mut self, dst: AddressingMode) -> u32 {
        // On 68000, this is not privileged. On 68010+, it is.
        let mut cycles = 6u32;
        let (dst_ea, dst_cycles) = calculate_ea(dst, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += dst_cycles;
        
        self.cpu_write_ea(dst_ea, Size::Word, self.sr as u32);
        cycles
    }

    fn exec_movep(&mut self, size: Size, reg: u8, an: u8, reg_to_mem: bool) -> u32 {
        let disp = self.read_word(self.pc) as i16;
        self.pc = self.pc.wrapping_add(2);
        
        let mut addr = self.a[an as usize].wrapping_add(disp as u32);
        
        match size {
            Size::Word => {
                if reg_to_mem {
                    let val = self.d[reg as usize] as u16;
                    self.memory.write_byte(addr, (val >> 8) as u8);
                    self.memory.write_byte(addr.wrapping_add(2), val as u8);
                } else {
                    let hi = self.memory.read_byte(addr);
                    let lo = self.memory.read_byte(addr.wrapping_add(2));
                    let val = ((hi as u16) << 8) | (lo as u16);
                    self.d[reg as usize] = (self.d[reg as usize] & 0xFFFF0000) | (val as u32);
                }
                16
            }
            Size::Long => {
                if reg_to_mem {
                    let val = self.d[reg as usize];
                    self.memory.write_byte(addr, (val >> 24) as u8);
                    self.memory.write_byte(addr.wrapping_add(2), (val >> 16) as u8);
                    self.memory.write_byte(addr.wrapping_add(4), (val >> 8) as u8);
                    self.memory.write_byte(addr.wrapping_add(6), val as u8);
                } else {
                    let b3 = self.memory.read_byte(addr);
                    let b2 = self.memory.read_byte(addr.wrapping_add(2));
                    let b1 = self.memory.read_byte(addr.wrapping_add(4));
                    let b0 = self.memory.read_byte(addr.wrapping_add(6));
                    self.d[reg as usize] = ((b3 as u32) << 24) | ((b2 as u32) << 16) | ((b1 as u32) << 8) | (b0 as u32);
                }
                24
            }
            _ => 4, // Should not happen for MOVEC
        }
    }

    fn exec_move_to_ccr(&mut self, src: AddressingMode) -> u32 {
        let mut cycles = 12u32;
        let (src_ea, src_cycles) = calculate_ea(src, Size::Word, &mut self.d, &mut self.a, &mut self.pc, &mut self.memory);
        cycles += src_cycles;
        
        let val = self.cpu_read_ea(src_ea, Size::Word) as u16;
        self.sr = (self.sr & 0xFF00) | (val & 0x00FF);
        cycles
    }

    fn exec_andi_to_ccr(&mut self) -> u32 {
        let imm = self.memory.read_word(self.pc) & 0x00FF;
        self.pc = self.pc.wrapping_add(2);
        self.sr = (self.sr & 0xFF00) | ((self.sr & imm) & 0x00FF);
        20
    }

    fn exec_andi_to_sr(&mut self) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8);
        }
        let imm = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.set_sr(self.sr & imm);
        20
    }

    fn exec_ori_to_ccr(&mut self) -> u32 {
        let imm = self.memory.read_word(self.pc) & 0x00FF;
        self.pc = self.pc.wrapping_add(2);
        self.sr = (self.sr & 0xFF00) | ((self.sr | imm) & 0x00FF);
        20
    }

    fn exec_ori_to_sr(&mut self) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8);
        }
        let imm = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.set_sr(self.sr | imm);
        20
    }

    fn exec_eori_to_ccr(&mut self) -> u32 {
        let imm = self.memory.read_word(self.pc) & 0x00FF;
        self.pc = self.pc.wrapping_add(2);
        self.sr = (self.sr & 0xFF00) | ((self.sr ^ imm) & 0x00FF);
        20
    }

    fn exec_eori_to_sr(&mut self) -> u32 {
        if (self.sr & 0x2000) == 0 {
            return self.process_exception(8);
        }
        let imm = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.set_sr(self.sr ^ imm);
        20

    }

}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::Memory;
    use proptest::prelude::*;

    fn create_test_cpu() -> Cpu {
        let mut memory = Memory::new(0x10000);
        // Initial SP and PC
        memory.write_long(0, 0x1000); // SP
        memory.write_long(4, 0x100);  // PC
        Cpu::new(Box::new(memory))
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

        let cpu = Cpu::new(Box::new(memory));

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

        let mut cpu = Cpu::new(Box::new(memory));
        cpu.d[1] = 0xABCD1234;

        assert_eq!(cpu.d[0], 0);
        assert_eq!(cpu.pc, 0x00000008);

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 0xABCD1234);
        assert_eq!(cpu.pc, 0x0000000A);
    }

    #[test]
    fn test_moveq() {
        let mut cpu = create_test_cpu();
        // MOVEQ #42, D3 = 0x762A
        cpu.memory.write_word(0x100, 0x762A);

        cpu.step_instruction();

        assert_eq!(cpu.d[3], 42);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_moveq_negative() {
        let mut cpu = create_test_cpu();
        // MOVEQ #-1, D0 = 0x70FF
        cpu.memory.write_word(0x100, 0x70FF);

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 0xFFFFFFFF);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
    }

    #[test]
    fn test_addq() {
        let mut cpu = create_test_cpu();
        // ADDQ.L #3, D0 = 0x5680
        cpu.memory.write_word(0x100, 0x5680);
        cpu.d[0] = 10;

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 13);
    }

    #[test]
    fn test_subq() {
        let mut cpu = create_test_cpu();
        // SUBQ.L #3, D0 = 0x5780
        cpu.memory.write_word(0x100, 0x5780);
        cpu.d[0] = 10;

        cpu.step_instruction();

        assert_eq!(cpu.d[0], 7);
    }

    #[test]
    fn test_bra() {
        let mut cpu = create_test_cpu();
        // BRA.S $+10 = 0x6008
        cpu.memory.write_word(0x100, 0x6008);

        cpu.step_instruction();

        assert_eq!(cpu.pc, 0x10A);
    }

    #[test]
    fn test_mul_div() {
        let mut cpu = create_test_cpu();
        
        // MULU D1, D0
        // D0 = 10, D1 = 20
        // Opcode: 1100 000 0 11 000 001 = 0xC0C1
        cpu.memory.write_word(0x100, 0xC0C1);
        cpu.d[0] = 10;
        cpu.d[1] = 20;
        
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 200);
        assert!(!cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        
        // MULS D1, D0
        // D0 = 10, D1 = -5 (0xFFFB)
        // Opcode: 1100 000 1 11 000 001 = 0xC1C1
        cpu.pc = 0x102;
        cpu.memory.write_word(0x102, 0xC1C1);
        cpu.d[0] = 10;
        cpu.d[1] = 0xFFFB; // -5 as i16
        
        cpu.step_instruction();
        // Result: 10 * -5 = -50 (0xFFFFFFCE)
        assert_eq!(cpu.d[0], 0xFFFFFFCE);
        assert!(cpu.get_flag(flags::NEGATIVE));
        assert!(!cpu.get_flag(flags::ZERO));
        
        // DIVU D1, D0
        // D0 = 100, D1 = 10
        // Opcode: 1000 000 0 11 000 001 = 0x80C1 (Group 8)
        cpu.pc = 0x104;
        cpu.memory.write_word(0x104, 0x80C1);
        cpu.d[0] = 100;
        cpu.d[1] = 10;
        
        cpu.step_instruction();
        // Result: 100 / 10 = 10. Remainder 0.
        // Format: rem:quot = 0000:000A
        assert_eq!(cpu.d[0], 0x0000000A);
    }

    #[test]
    fn test_bcd() {
        let mut cpu = create_test_cpu();
        
        // ABCD D0, D1
        // D0 = 0x45, D1 = 0x33
        // Result should be 0x78
        // Opcode: 1100 001 1 0000 0 000 = 0xC300
        cpu.memory.write_word(0x100, 0xC300);
        cpu.d[0] = 0x45;
        cpu.d[1] = 0x33;
        cpu.set_flag(flags::ZERO, true); // Pre-set Z
        cpu.set_flag(flags::EXTEND, false);
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[1] & 0xFF, 0x78);
        assert!(!cpu.get_flag(flags::ZERO)); // Z cleared because result non-zero
        assert!(!cpu.get_flag(flags::EXTEND));
        
        // SBCD D0, D1
        // D0 = 0x33, D1 = 0x78
        // Result 0x78 - 0x33 = 0x45
        // Opcode: 1000 001 1 0000 0 000 = 0x8300
        cpu.pc = 0x102;
        cpu.memory.write_word(0x102, 0x8300);
        cpu.d[0] = 0x33;
        cpu.d[1] = 0x78;
        cpu.set_flag(flags::ZERO, true);
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[1] & 0xFF, 0x45);
        assert!(!cpu.get_flag(flags::ZERO));
        
        // NBCD D0
        // D0 = 0x45. 100 - 45 = 55 (0x55).
        // Opcode: 0100 100 0 00 000 000 = 0x4800 (NBCD D0)
        cpu.pc = 0x104;
        cpu.memory.write_word(0x104, 0x4800);
        cpu.d[0] = 0x45;
        cpu.set_flag(flags::ZERO, true);
        cpu.set_flag(flags::EXTEND, false);
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[0] & 0xFF, 0x55);
        assert!(!cpu.get_flag(flags::ZERO));
        assert!(cpu.get_flag(flags::EXTEND)); // Borrows because 0 - 45
    }

    #[test]
    fn test_exg() {
        let mut cpu = create_test_cpu();
        
        // EXG D0, D1
        // Opcode: 1100 000 1 01000 001 = 0xC141
        // Mode 8 (01000)
        cpu.memory.write_word(0x100, 0xC141);
        cpu.d[0] = 0x11111111;
        cpu.d[1] = 0x22222222;
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[0], 0x22222222);
        assert_eq!(cpu.d[1], 0x11111111);
        
        // EXG A0, A1
        // Opcode: 1100 001 1 0100 1 001 = 0xC149
        // Mode 9 (01001)
        cpu.pc = 0x102;
        cpu.memory.write_word(0x102, 0xC149);
        cpu.a[0] = 0xAAAA5555;
        cpu.a[1] = 0x5555AAAA;
        
        cpu.step_instruction();
        
        assert_eq!(cpu.a[0], 0x5555AAAA);
        assert_eq!(cpu.a[1], 0xAAAA5555);
        
        // EXG D0, A0
        // Opcode: 1100 001 1 1000 1 000 = 0xC188 ??
        // Mode 17 (10001) -> 0x11
        // decoder: ((opcode >> 3) & 0x1F)
        // 1 1000 1 -> 1100 001 1 1000 1 000
        // Opcode: C188
        cpu.pc = 0x104;
        cpu.memory.write_word(0x104, 0xC188);
        cpu.d[0] = 0xDEADBEEF;
        cpu.a[0] = 0xCAFEBABE;
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[0], 0xCAFEBABE);
        assert_eq!(cpu.a[0], 0xDEADBEEF);
    }
    #[test]
    fn test_beq_taken() {
        let mut cpu = create_test_cpu();
        cpu.set_flag(flags::ZERO, true);
        // BEQ.S $+6 = 0x6704
        cpu.memory.write_word(0x100, 0x6704);

        cpu.step_instruction();

        assert_eq!(cpu.pc, 0x106);
    }

    #[test]
    fn test_beq_not_taken() {
        let mut cpu = create_test_cpu();
        cpu.set_flag(flags::ZERO, false);
        // BEQ.S $+6 = 0x6704
        cpu.memory.write_word(0x100, 0x6704);

        cpu.step_instruction();

        assert_eq!(cpu.pc, 0x102);
    }

    #[test]
    fn test_nop() {
        let mut cpu = create_test_cpu();
        // NOP = 0x4E71
        cpu.memory.write_word(0x100, 0x4E71);

        let cycles = cpu.step_instruction();

        assert_eq!(cpu.pc, 0x102);
        assert_eq!(cycles, 4);
    }

    #[test]
    fn test_bit_ops() {
        let mut cpu = create_test_cpu();
        
        // BSET #2, D0
        // D0 = 0
        // Opcode: 0000 100 0 11 000 000 = 0x08C0 (Group 0, BSET immediate)
        // Immediate word: 0x0002
        cpu.memory.write_word(0x100, 0x08C0);
        cpu.memory.write_word(0x102, 0x0002);
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[0], 0x00000004);
        assert!(cpu.get_flag(flags::ZERO)); // Tested bit 2 was 0
        
        // BCLR #2, D0
        // D0 = 4
        // Opcode: 0000 100 0 10 000 000 = 0x0880 (BCLR immediate)
        // Imm: 0x0002
        cpu.pc = 0x104;
        cpu.memory.write_word(0x104, 0x0880);
        cpu.memory.write_word(0x106, 0x0002);
        
        cpu.step_instruction();
        
        assert_eq!(cpu.d[0], 0x00000000);
        assert!(!cpu.get_flag(flags::ZERO)); // Tested bit 2 was 1
        
        // BCHG #0, D0
        // D0 = 0
        // Opcode: 0000 100 0 01 000 000 = 0x0840
        cpu.pc = 0x108;
        cpu.memory.write_word(0x108, 0x0840);
        cpu.memory.write_word(0x10A, 0x0000);
        
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 0x00000001);
        
        // BCHG #0, D0
        cpu.pc = 0x10C;
        cpu.memory.write_word(0x10C, 0x0840);
        cpu.memory.write_word(0x10E, 0x0000);
        cpu.step_instruction();
        assert_eq!(cpu.d[0], 0x00000000);
        
        // BTST #5, D0
        // D0 = 0x20 (bit 5)
        cpu.d[0] = 0x20;
        // Opcode: 0000 100 0 00 000 000 = 0x0800
        cpu.pc = 0x110;
        cpu.memory.write_word(0x110, 0x0800);
        cpu.memory.write_word(0x112, 0x0005);
        
        cpu.step_instruction();
        assert!(!cpu.get_flag(flags::ZERO)); // Bit 5 is 1, so Z=0
    }
    #[test]
    fn test_rts() {
        let mut cpu = create_test_cpu();
        // Push return address on stack
        cpu.a[7] = 0x0FF0;
        cpu.memory.write_long(0x0FF0, 0x200);
        // RTS = 0x4E75
        cpu.memory.write_word(0x100, 0x4E75);

        cpu.step_instruction();

        assert_eq!(cpu.pc, 0x200);
        assert_eq!(cpu.a[7], 0x0FF4);
    }

    #[test]
    fn test_swap() {
        let mut cpu = create_test_cpu();
        // SWAP D0 = 0x4840
        cpu.memory.write_word(0x100, 0x4840);
        cpu.d[0] = 0x12345678;

        cpu.step_instruction();

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

            let mut cpu = Cpu::new(Box::new(memory));
            cpu.d[1] = val;

            cpu.step_instruction();

            assert_eq!(cpu.d[0], val);
        }
    }

    #[test]
    fn test_link_unlk() {
        let mut cpu = create_test_cpu();
        
        // LINK A0, #-4
        // A0 = 0x2000. SP = 0x8000.
        // Opcode: 0100 111 0 01 010 000 = 0x4E50 (LINK A0)
        // Displacement: 0xFFFC (-4)
        cpu.memory.write_word(0x100, 0x4E50);
        cpu.memory.write_word(0x102, 0xFFFC);
        
        cpu.a[0] = 0x2000;
        cpu.a[7] = 0x8000;
        
        cpu.step_instruction();
        
        // LINK action: 
        // 1. Push A0 -> SP=0x7FFC, Mem[0x7FFC]=0x2000
        // 2. SP -> A0 => A0=0x7FFC
        // 3. SP + Disp -> SP => 0x7FFC - 4 = 0x7FF8.
        
        assert_eq!(cpu.memory.read_long(0x7FFC), 0x2000);
        assert_eq!(cpu.a[0], 0x7FFC);
        assert_eq!(cpu.a[7], 0x7FF8);
        assert_eq!(cpu.pc, 0x104);
        
        // UNLK A0
        // Opcode: 0100 111 0 01 011 000 = 0x4E58 (UNLK A0)
        cpu.memory.write_word(0x104, 0x4E58);
        
        cpu.step_instruction();
        
        // UNLK action:
        // 1. A0 -> SP => SP=0x7FFC
        // 2. Pop -> A0 => A0=0x2000 (from stack), SP=0x8000
        
        assert_eq!(cpu.a[0], 0x2000);
        assert_eq!(cpu.a[7], 0x8000);
        assert_eq!(cpu.pc, 0x106);
    }

    #[test]
    fn test_trap() {
        let mut cpu = create_test_cpu();
        
        // TRAP #1
        // Opcode: 0100 111 0 01 000 001 = 0x4E41
        cpu.memory.write_word(0x100, 0x4E41);
        
        // Initial State
        cpu.pc = 0x100;
        cpu.ssp = 0x8000;
        cpu.a[7] = 0x4000; // USP
        cpu.sr = 0x0700; // Not supervisor
        
        // Set Trap Vector #33 (32+1) -> Address 33*4 = 132 (0x84)
        cpu.memory.write_long(0x84, 0x00004000); // Exception handler address
        
        cpu.step_instruction();
        
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
        assert_eq!(cpu.memory.read_word(0x7FFA), 0x0700);
        assert_eq!(cpu.memory.read_long(0x7FFC), 0x102);
        assert_eq!(cpu.sr & 0x2000, 0x2000); // Supervisor Set
    }

    #[test]
    fn test_scc() {
        let mut cpu = create_test_cpu();
        
        // SEQ D0 (Set if Equal: sets D0 to 0xFF if Z=1)
        // Opcode: 0101 0111 11 000 000 = 0x57C0
        cpu.memory.write_word(0x100, 0x57C0);
        
        // SNE D1 (Set if Not Equal: sets D1 to 0xFF if Z=0)
        // Opcode: 0101 0110 11 000 001 = 0x56C1
        cpu.memory.write_word(0x102, 0x56C1);
        
        cpu.pc = 0x100;
        cpu.set_flag(flags::ZERO, true);
        cpu.d[0] = 0;
        cpu.d[1] = 0;
        
        cpu.step_instruction(); // SEQ D0 -> D0 should be 0xFF
        assert_eq!(cpu.d[0] & 0xFF, 0xFF);
        
        cpu.step_instruction(); // SNE D1 -> D1 should be 0x00
        assert_eq!(cpu.d[1] & 0xFF, 0x00);
    }

    #[test]
    fn test_movep() {
        let mut cpu = create_test_cpu();
        
        // MOVEP.W (A0), D0
        // Opcode: 0000 000 1 00 001 000 = 0x0108
        // Displacement: 0x0004
        cpu.memory.write_word(0x100, 0x0108);
        cpu.memory.write_word(0x102, 0x0004);
        
        // MOVEP.W D1, (A0)
        // Opcode: 0000 001 1 10 001 000 = 0x0388
        // Displacement: 0x0004
        cpu.memory.write_word(0x104, 0x0388);
        cpu.memory.write_word(0x106, 0x0004);

        cpu.pc = 0x100;
        cpu.a[0] = 0x2000;
        
        // Setup memory for first MOVEP (mem to reg)
        cpu.memory.write_byte(0x2004, 0x12);
        cpu.memory.write_byte(0x2006, 0x34);
        
        cpu.step_instruction();
        assert_eq!(cpu.d[0] & 0xFFFF, 0x1234);
        
        // Setup reg for second MOVEP (reg to mem)
        cpu.d[1] = 0x5678;
        cpu.step_instruction();
        assert_eq!(cpu.memory.read_byte(0x2004), 0x56);
        assert_eq!(cpu.memory.read_byte(0x2006), 0x78);
    }
}
