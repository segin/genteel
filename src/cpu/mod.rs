//! M68k CPU Core
//!
//! This module implements the Motorola 68000 CPU, the main processor
//! of the Sega Mega Drive/Genesis.

pub mod decoder;
pub mod addressing;

use crate::memory::Memory;
use decoder::{decode, Instruction, Size, AddressingMode, Condition, ShiftCount};
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
    memory: Memory,

    // Cycle counter for timing
    pub cycles: u64,

    // Halted state
    pub halted: bool,
}

impl Cpu {
    pub fn new(memory: Memory) -> Self {
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
        if self.halted {
            return 4; // Minimum cycles when halted
        }

        let opcode = self.memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);

        let instruction = decode(opcode);
        let cycles = self.execute(instruction);

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
            Instruction::Clr { size, dst } => self.exec_clr(size, dst),

            // === Arithmetic ===
            Instruction::Add { size, src, dst, direction } => self.exec_add(size, src, dst, direction),
            Instruction::AddA { size, src, dst_reg } => self.exec_adda(size, src, dst_reg),
            Instruction::AddQ { size, dst, data } => self.exec_addq(size, dst, data),
            Instruction::Sub { size, src, dst, direction } => self.exec_sub(size, src, dst, direction),
            Instruction::SubA { size, src, dst_reg } => self.exec_suba(size, src, dst_reg),
            Instruction::SubQ { size, dst, data } => self.exec_subq(size, dst, data),
            Instruction::Neg { size, dst } => self.exec_neg(size, dst),

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

            // === Compare and Test ===
            Instruction::Cmp { size, src, dst_reg } => self.exec_cmp(size, src, dst_reg),
            Instruction::CmpA { size, src, dst_reg } => self.exec_cmpa(size, src, dst_reg),
            Instruction::Tst { size, dst } => self.exec_tst(size, dst),

            // === Branch and Jump ===
            Instruction::Bra { displacement } => self.exec_bra(displacement),
            Instruction::Bsr { displacement } => self.exec_bsr(displacement),
            Instruction::Bcc { condition, displacement } => self.exec_bcc(condition, displacement),
            Instruction::DBcc { condition, reg } => self.exec_dbcc(condition, reg),
            Instruction::Jmp { dst } => self.exec_jmp(dst),
            Instruction::Jsr { dst } => self.exec_jsr(dst),
            Instruction::Rts => self.exec_rts(),

            // === Misc ===
            Instruction::Nop => 4,
            Instruction::Swap { reg } => self.exec_swap(reg),
            Instruction::Ext { size, reg } => self.exec_ext(size, reg),

            // === Not yet implemented ===
            Instruction::Illegal => {
                self.halted = true;
                4
            }
            Instruction::Unimplemented { opcode } => {
                // For now, treat as NOP but log
                #[cfg(debug_assertions)]
                eprintln!("Unimplemented opcode: {:04X} at PC {:08X}", opcode, self.pc.wrapping_sub(2));
                4
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
        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;

        // Handle post-increment for source
        if let AddressingMode::AddressPostIncrement(reg) = src {
            let inc = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_add(inc);
        }

        // Handle pre-decrement for source (already calculated with decremented address)
        if let AddressingMode::AddressPreDecrement(reg) = src {
            let dec = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_sub(dec);
        }

        // Read source value
        let value = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        // Calculate destination EA
        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += dst_cycles;

        // Handle pre-decrement for destination
        if let AddressingMode::AddressPreDecrement(reg) = dst {
            let dec = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_sub(dec);
        }

        // Write to destination
        write_ea(dst_ea, size, value, &mut self.d, &mut self.a, &mut self.memory);

        // Handle post-increment for destination
        if let AddressingMode::AddressPostIncrement(reg) = dst {
            let inc = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_add(inc);
        }

        // Update flags
        self.update_nz_flags(value, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles
    }

    fn exec_movea(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;

        // Handle post-increment
        if let AddressingMode::AddressPostIncrement(reg) = src {
            let inc = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_add(inc);
        }

        let value = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

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
        let (ea, cycles) = calculate_ea(src, Size::Long, &self.d, &self.a, &mut self.pc, &self.memory);

        if let EffectiveAddress::Memory(addr) = ea {
            self.a[dst_reg as usize] = addr;
        }

        4 + cycles
    }

    fn exec_clr(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);

        // Handle pre-decrement
        if let AddressingMode::AddressPreDecrement(reg) = dst {
            let dec = match size {
                Size::Byte => if reg == 7 { 2 } else { 1 },
                Size::Word => 2,
                Size::Long => 4,
            };
            self.a[reg as usize] = self.a[reg as usize].wrapping_sub(dec);
        }

        write_ea(dst_ea, size, 0, &mut self.d, &mut self.a, &mut self.memory);

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

        let (src_ea, src_cycles) = calculate_ea(src_mode, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        let (dst_ea, dst_cycles) = calculate_ea(dst_mode, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += dst_cycles;
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let (result, carry, overflow) = self.add_with_flags(src_val, dst_val, size);

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

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

        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;

        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

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
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

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

        let (src_ea, src_cycles) = calculate_ea(src_mode, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        let (dst_ea, dst_cycles) = calculate_ea(dst_mode, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += dst_cycles;
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, src_val, size);

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

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

    fn exec_suba(&mut self, size: Size, src: AddressingMode, dst_reg: u8) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;

        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        let src_val = match size {
            Size::Word => (src_val as i16) as i32 as u32,
            Size::Long => src_val,
            Size::Byte => src_val,
        };

        self.a[dst_reg as usize] = self.a[dst_reg as usize].wrapping_sub(src_val);

        cycles + if size == Size::Long { 4 } else { 0 }
    }

    fn exec_subq(&mut self, size: Size, dst: AddressingMode, data: u8) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let (result, borrow, overflow) = self.sub_with_flags(dst_val, data as u32, size);

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        if !matches!(dst, AddressingMode::AddressRegister(_)) {
            self.update_nz_flags(result, size);
            self.set_flag(flags::CARRY, borrow);
            self.set_flag(flags::EXTEND, borrow);
            self.set_flag(flags::OVERFLOW, overflow);
        }

        4 + cycles
    }

    fn exec_neg(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

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

        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += dst_cycles;
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let result = src_val & dst_val;

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        cycles
    }

    fn exec_or(&mut self, size: Size, src: AddressingMode, dst: AddressingMode, _direction: bool) -> u32 {
        let mut cycles = 4u32;

        let (src_ea, src_cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += src_cycles;
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

        let (dst_ea, dst_cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        cycles += dst_cycles;
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let result = src_val | dst_val;

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

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

        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let dst_val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let result = src_val ^ dst_val;

        write_ea(dst_ea, size, result, &mut self.d, &mut self.a, &mut self.memory);

        self.update_nz_flags(result, size);
        self.set_flag(flags::OVERFLOW, false);
        self.set_flag(flags::CARRY, false);

        4 + cycles
    }

    fn exec_not(&mut self, size: Size, dst: AddressingMode) -> u32 {
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

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

        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

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

        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

        let (mask, bits) = match size {
            Size::Byte => (0xFFu32, 8u32),
            Size::Word => (0xFFFF, 16),
            Size::Long => (0xFFFFFFFF, 32),
        };

        let val = val & mask;
        let effective_count = count_val % bits;
        let result;
        let mut carry = false;

        if left {
            result = ((val << effective_count) | (val >> (bits - effective_count))) & mask;
            if count_val > 0 {
                carry = ((val >> (bits - effective_count)) & 1) != 0;
            }
        } else {
            result = ((val >> effective_count) | (val << (bits - effective_count))) & mask;
            if count_val > 0 {
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
        let (src_ea, cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

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
        let (src_ea, cycles) = calculate_ea(src, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let src_val = read_ea(src_ea, size, &self.d, &self.a, &self.memory);

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
        let (dst_ea, cycles) = calculate_ea(dst, size, &self.d, &self.a, &mut self.pc, &self.memory);
        let val = read_ea(dst_ea, size, &self.d, &self.a, &self.memory);

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
        let (ea, cycles) = calculate_ea(dst, Size::Long, &self.d, &self.a, &mut self.pc, &self.memory);

        if let EffectiveAddress::Memory(addr) = ea {
            self.pc = addr;
        }

        4 + cycles
    }

    fn exec_jsr(&mut self, dst: AddressingMode) -> u32 {
        let (ea, cycles) = calculate_ea(dst, Size::Long, &self.d, &self.a, &mut self.pc, &self.memory);

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
        Cpu::new(memory)
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

        let cpu = Cpu::new(memory);

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

        let mut cpu = Cpu::new(memory);
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

            let mut cpu = Cpu::new(memory);
            cpu.d[1] = val;

            cpu.step_instruction();

            assert_eq!(cpu.d[0], val);
        }
    }
}
