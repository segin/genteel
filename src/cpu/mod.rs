//! M68k CPU Core
//!
//! This module implements the Motorola 68000 CPU, the main processor
//! of the Sega Mega Drive/Genesis.

pub mod addressing;
pub mod decoder;
pub mod instructions;
pub mod ops;

use crate::cpu::decoder::decode;
use crate::cpu::instructions::{
    ArithmeticInstruction, BitSource, BitsInstruction, Condition, DataInstruction,
    DecodeCacheEntry, Instruction, Size, SystemInstruction,
};
use crate::memory::MemoryInterface;

/// Status Register flags
pub mod flags {
    pub const CARRY: u16 = 0x0001; // C - Carry
    pub const OVERFLOW: u16 = 0x0002; // V - Overflow
    pub const ZERO: u16 = 0x0004; // Z - Zero
    pub const NEGATIVE: u16 = 0x0008; // N - Negative
    pub const EXTEND: u16 = 0x0010; // X - Extend
    pub const INTERRUPT_MASK: u16 = 0x0700; // I2-I0 - Interrupt mask
    pub const SUPERVISOR: u16 = 0x2000; // S - Supervisor mode
    pub const TRACE: u16 = 0x8000; // T - Trace mode
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
    pub usp: u32, // User stack pointer (saved when in supervisor mode)
    pub ssp: u32, // Supervisor stack pointer (saved when in user mode)

    // Cycle counter for timing
    pub cycles: u64,

    // Halted state
    pub halted: bool,

    // Pending interrupt level (0-7, 0 = none)
    pub pending_interrupt: u8,
    pub pending_exception: bool,

    // Interrupt pending bitmask (bit N = level N is pending)
    pub interrupt_pending_mask: u8,

    // Instruction cache (Direct Mapped, 64K entries)
    pub decode_cache: Box<[DecodeCacheEntry]>,
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
            decode_cache: vec![DecodeCacheEntry::default(); 65536].into_boxed_slice(),
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
        // Invalidate cache on reset
        self.decode_cache.fill(DecodeCacheEntry::default());
    }

    /// Invalidate the instruction cache.
    /// Should be called when code in ROM/RAM is modified (e.g. self-modifying code, or tests).
    pub fn invalidate_cache(&mut self) {
        self.decode_cache.fill(DecodeCacheEntry::default());
    }

    fn invalidate_cache_line(&mut self, addr: u32) {
        let index = ((addr >> 1) & 0xFFFF) as usize;
        if let Some(entry) = self.decode_cache.get_mut(index) {
            entry.pc = u32::MAX;
        }
    }

    /// Request an interrupt at the specified level
    /// Uses a bitmask to queue multiple interrupt levels
    pub fn request_interrupt(&mut self, level: u8) {
        if level == 0 || level > 7 {
            return;
        }
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
        if level > 7 {
            return;
        }
        // Clear the bit for this interrupt level
        self.interrupt_pending_mask &= !(1 << level);
        // Update to next highest priority
        self.update_pending_interrupt();
    }

    pub fn get_flag(&self, flag: u16) -> bool {
        (self.sr & flag) != 0
    }

    pub fn set_flag(&mut self, flag: u16, val: bool) {
        if val {
            self.sr |= flag;
        } else {
            self.sr &= !flag;
        }
    }

    pub fn update_nz_flags(&mut self, val: u32, size: Size) {
        let val = match size {
            Size::Byte => val & 0xFF,
            Size::Word => val & 0xFFFF,
            Size::Long => val,
        };
        self.set_flag(flags::ZERO, val == 0);
        self.set_flag(flags::NEGATIVE, size.is_negative(val));
    }

    pub fn add_with_flags(&self, src: u32, dst: u32, size: Size) -> (u32, bool, bool) {
        let mask = size.mask();
        let sign_bit = size.sign_bit();
        let s = src & mask;
        let d = dst & mask;
        let res = s.wrapping_add(d) & mask;

        let carry = (s as u64 + d as u64) > mask as u64;
        let overflow = ((s ^ res) & (d ^ res) & sign_bit) != 0;

        (res, carry, overflow)
    }

    pub fn sub_with_flags(&self, dst: u32, src: u32, size: Size) -> (u32, bool, bool) {
        let mask = size.mask();
        let sign_bit = size.sign_bit();
        let s = src & mask;
        let d = dst & mask;
        let res = d.wrapping_sub(s) & mask;

        let carry = d < s;
        let overflow = ((d ^ s) & (d ^ res) & sign_bit) != 0;

        (res, carry, overflow)
    }

    pub fn addx_with_flags(&self, src: u32, dst: u32, x: u32, size: Size) -> (u32, bool, bool) {
        let mask = size.mask();
        let sign_bit = size.sign_bit();
        let s = src & mask;
        let d = dst & mask;
        let res = s.wrapping_add(d).wrapping_add(x) & mask;

        let carry = (s as u64 + d as u64 + x as u64) > mask as u64;
        let overflow = ((s ^ res) & (d ^ res) & sign_bit) != 0;

        (res, carry, overflow)
    }

    pub fn subx_with_flags(&self, dst: u32, src: u32, x: u32, size: Size) -> (u32, bool, bool) {
        let mask = size.mask();
        let sign_bit = size.sign_bit();
        let s = src & mask;
        let d = dst & mask;
        let res = d.wrapping_sub(s).wrapping_sub(x) & mask;

        let carry = (d as u64) < (s as u64 + x as u64);
        let overflow = ((d ^ s) & (d ^ res) & sign_bit) != 0;

        (res, carry, overflow)
    }

    pub fn push_long<M: MemoryInterface>(&mut self, val: u32, memory: &mut M) {
        self.a[7] = self.a[7].wrapping_sub(4);
        memory.write_long(self.a[7], val);
    }

    pub fn push_word<M: MemoryInterface>(&mut self, val: u16, memory: &mut M) {
        self.a[7] = self.a[7].wrapping_sub(2);
        memory.write_word(self.a[7], val);
    }

    pub fn pop_long<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        let val = memory.read_long(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(4);
        val
    }

    pub fn pop_word<M: MemoryInterface>(&mut self, memory: &mut M) -> u16 {
        let val = memory.read_word(self.a[7]);
        self.a[7] = self.a[7].wrapping_add(2);
        val
    }

    pub fn process_exception<M: MemoryInterface>(&mut self, vector: u32, memory: &mut M) -> u32 {
        if self.pending_exception {
            eprintln!("Double fault detected at PC={:X}. Halting.", self.pc);
            self.halted = true;
            return 0;
        }
        self.pending_exception = true;
        let old_sr = self.sr;
        self.set_flag(flags::SUPERVISOR, true);
        self.set_flag(flags::TRACE, false);

        // Switch to supervisor stack if necessary
        if (old_sr & flags::SUPERVISOR) == 0 {
            self.usp = self.a[7];
            self.a[7] = self.ssp;
        }

        self.push_long(self.pc, memory);
        self.push_word(old_sr, memory);

        let vector_addr = vector * 4;
        self.pc = memory.read_long(vector_addr);

        34 // Most exceptions take ~34 cycles
    }

    pub fn set_sr(&mut self, val: u16) {
        let old_sr = self.sr;
        self.sr = val;

        // Stack switching
        if (old_sr ^ self.sr) & flags::SUPERVISOR != 0 {
            if (self.sr & flags::SUPERVISOR) != 0 {
                self.usp = self.a[7];
                self.a[7] = self.ssp;
            } else {
                self.ssp = self.a[7];
                self.a[7] = self.usp;
            }
        }
    }

    pub fn step_instruction<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        self.pending_exception = false;

        // Handle interrupts before fetching next instruction
        let int_cycles = self.check_interrupts(memory);
        if int_cycles > 0 {
            self.cycles += int_cycles as u64;
            return int_cycles;
        }

        if self.halted {
            return 4;
        }

        let pc = self.pc;
        let instruction;

        // Optimized instruction fetch with cache
        if pc < 0x400000 {
            // ROM/Cartridge space - Cacheable
            // Index: (PC / 2) & 0xFFFF. Maps 0-128KB repeating or just lower bits.
            // Since we check entry.pc == pc, aliasing is handled safely.
            let cache_index = ((pc >> 1) & 0xFFFF) as usize;

            // Try to read from cache safely
            // If the cache has been resized to be smaller than 65536, get() returns None
            // and we fall back to uncached fetch, preventing out-of-bounds access.
            if let Some(entry) = self.decode_cache.get(cache_index).copied() {
                if entry.pc == pc {
                    // Cache Hit
                    instruction = entry.instruction;
                    self.pc = pc.wrapping_add(2);
                } else {
                    // Cache Miss
                    let opcode = self.read_instruction_word(pc, memory);
                    if self.pending_exception {
                        // Address Error during fetch
                        self.cycles += 34;
                        return 34;
                    }

                    self.pc = self.pc.wrapping_add(2);
                    instruction = decode(opcode);

                    // Update Cache
                    // We know the index is valid because get() succeeded earlier,
                    // but we use get_mut() for safety in case of concurrent modification (unlikely here)
                    // or weird edge cases.
                    if let Some(entry_mut) = self.decode_cache.get_mut(cache_index) {
                        *entry_mut = DecodeCacheEntry { pc, instruction };
                    }
                }
            } else {
                // Cache index out of bounds (cache too small/invalid)
                // Fallback to uncached fetch
                let opcode = self.read_instruction_word(pc, memory);
                if self.pending_exception {
                    // Address Error during fetch
                    self.cycles += 34;
                    return 34;
                }

                self.pc = self.pc.wrapping_add(2);
                instruction = decode(opcode);
            }
        } else {
            // Uncached (RAM, I/O, etc.)
            let opcode = self.read_instruction_word(pc, memory);
            if self.pending_exception {
                // Address Error during fetch
                self.cycles += 34;
                return 34;
            }

            self.pc = self.pc.wrapping_add(2);
            instruction = decode(opcode);
        }

        let cycles = self.execute(instruction, memory);
        self.cycles += cycles as u64;
        cycles
    }

    fn read_instruction_word<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u16 {
        if !addr.is_multiple_of(2) {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_word(addr)
    }

    fn execute<M: MemoryInterface>(&mut self, instruction: Instruction, memory: &mut M) -> u32 {
        match instruction {
            Instruction::Data(data_instr) => match data_instr {
                DataInstruction::Move { size, src, dst } => {
                    ops::data::exec_move(self, size, src, dst, memory)
                }
                DataInstruction::MoveA { size, src, dst_reg } => {
                    ops::data::exec_movea(self, size, src, dst_reg, memory)
                }
                DataInstruction::MoveQ { dst_reg, data } => {
                    ops::data::exec_moveq(self, dst_reg, data)
                }
                DataInstruction::Lea { src, dst_reg } => {
                    ops::data::exec_lea(self, src, dst_reg, memory)
                }
                DataInstruction::Pea { src } => ops::data::exec_pea(self, src, memory),
                DataInstruction::Clr { size, dst } => {
                    ops::arithmetic::exec_clr(self, size, dst, memory)
                }
                DataInstruction::Exg { rx, ry, mode } => ops::data::exec_exg(self, rx, ry, mode),
                DataInstruction::Movep {
                    size,
                    reg,
                    an,
                    direction,
                } => ops::data::exec_movep(self, size, reg, an, direction, memory),
                DataInstruction::Movem {
                    size,
                    direction,
                    mask: _,
                    ea,
                } => ops::data::exec_movem(self, size, direction, ea, memory),
                DataInstruction::Swap { reg } => ops::data::exec_swap(self, reg),
                DataInstruction::Ext { size, reg } => ops::data::exec_ext(self, size, reg),
            },
            Instruction::Arithmetic(arith_instr) => match arith_instr {
                ArithmeticInstruction::Add { size, src, dst, .. } => {
                    ops::arithmetic::exec_add(self, size, src, dst, memory)
                }
                ArithmeticInstruction::AddA { size, src, dst_reg } => {
                    ops::arithmetic::exec_adda(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::AddI { size, dst } => {
                    ops::arithmetic::exec_addi(self, size, dst, memory)
                }
                ArithmeticInstruction::AddQ { size, dst, data } => {
                    ops::arithmetic::exec_addq(self, size, data, dst, memory)
                }
                ArithmeticInstruction::Sub { size, src, dst, .. } => {
                    ops::arithmetic::exec_sub(self, size, src, dst, memory)
                }
                ArithmeticInstruction::SubA { size, src, dst_reg } => {
                    ops::arithmetic::exec_suba(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::SubI { size, dst } => {
                    ops::arithmetic::exec_subi(self, size, dst, memory)
                }
                ArithmeticInstruction::SubQ { size, dst, data } => {
                    ops::arithmetic::exec_subq(self, size, data, dst, memory)
                }
                ArithmeticInstruction::MulU { src, dst_reg } => {
                    ops::arithmetic::exec_mulu(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::MulS { src, dst_reg } => {
                    ops::arithmetic::exec_muls(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::DivU { src, dst_reg } => {
                    ops::arithmetic::exec_divu(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::DivS { src, dst_reg } => {
                    ops::arithmetic::exec_divs(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::Neg { size, dst } => {
                    ops::arithmetic::exec_neg(self, size, dst, memory)
                }
                ArithmeticInstruction::Abcd {
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ops::arithmetic::exec_abcd(self, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::Sbcd {
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ops::arithmetic::exec_sbcd(self, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::Nbcd { dst } => {
                    ops::arithmetic::exec_nbcd(self, dst, memory)
                }
                ArithmeticInstruction::AddX {
                    size,
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ops::arithmetic::exec_addx(self, size, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::SubX {
                    size,
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ops::arithmetic::exec_subx(self, size, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::NegX { size, dst } => {
                    ops::arithmetic::exec_negx(self, size, dst, memory)
                }
                ArithmeticInstruction::Chk { src, dst_reg } => {
                    ops::arithmetic::exec_chk(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::Cmp { size, src, dst_reg } => {
                    ops::arithmetic::exec_cmp(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::CmpA { size, src, dst_reg } => {
                    ops::arithmetic::exec_cmpa(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::CmpI { size, dst } => {
                    ops::arithmetic::exec_cmpi(self, size, dst, memory)
                }
                ArithmeticInstruction::CmpM { size, ax, ay } => {
                    ops::arithmetic::exec_cmpm(self, size, ax, ay, memory)
                }
                ArithmeticInstruction::Tst { size, dst } => {
                    ops::arithmetic::exec_tst(self, size, dst, memory)
                }
            },
            Instruction::Bits(bits_instr) => match bits_instr {
                BitsInstruction::And {
                    size,
                    src,
                    dst,
                    direction,
                } => ops::bits::exec_and(self, size, src, dst, direction, memory),
                BitsInstruction::AndI { size, dst } => {
                    ops::bits::exec_andi(self, size, dst, memory)
                }
                BitsInstruction::Or {
                    size,
                    src,
                    dst,
                    direction,
                } => ops::bits::exec_or(self, size, src, dst, direction, memory),
                BitsInstruction::OrI { size, dst } => ops::bits::exec_ori(self, size, dst, memory),
                BitsInstruction::Eor { size, src_reg, dst } => {
                    ops::bits::exec_eor(self, size, src_reg, dst, memory)
                }
                BitsInstruction::EorI { size, dst } => {
                    ops::bits::exec_eori(self, size, dst, memory)
                }
                BitsInstruction::Not { size, dst } => ops::bits::exec_not(self, size, dst, memory),
                BitsInstruction::Lsl { size, dst, count } => {
                    ops::bits::exec_shift(self, size, dst, count, true, false, memory)
                }
                BitsInstruction::Lsr { size, dst, count } => {
                    ops::bits::exec_shift(self, size, dst, count, false, false, memory)
                }
                BitsInstruction::Asl { size, dst, count } => {
                    ops::bits::exec_shift(self, size, dst, count, true, true, memory)
                }
                BitsInstruction::AslM { dst } => {
                    ops::bits::exec_shift_mem(self, dst, true, true, memory)
                }
                BitsInstruction::Asr { size, dst, count } => {
                    ops::bits::exec_shift(self, size, dst, count, false, true, memory)
                }
                BitsInstruction::AsrM { dst } => {
                    ops::bits::exec_shift_mem(self, dst, false, true, memory)
                }
                BitsInstruction::Rol { size, dst, count } => {
                    ops::bits::exec_rotate(self, size, dst, count, true, false, memory)
                }
                BitsInstruction::Ror { size, dst, count } => {
                    ops::bits::exec_rotate(self, size, dst, count, false, false, memory)
                }
                BitsInstruction::Roxl { size, dst, count } => {
                    ops::bits::exec_roxl(self, size, dst, count, memory)
                }
                BitsInstruction::Roxr { size, dst, count } => {
                    ops::bits::exec_roxr(self, size, dst, count, memory)
                }
                BitsInstruction::Btst { bit, dst } => ops::bits::exec_btst(self, bit, dst, memory),
                BitsInstruction::Bset { bit, dst } => ops::bits::exec_bset(self, bit, dst, memory),
                BitsInstruction::Bclr { bit, dst } => ops::bits::exec_bclr(self, bit, dst, memory),
                BitsInstruction::Bchg { bit, dst } => ops::bits::exec_bchg(self, bit, dst, memory),
                BitsInstruction::Tas { dst } => ops::bits::exec_tas(self, dst, memory),
            },
            Instruction::System(sys_instr) => match sys_instr {
                SystemInstruction::Bra { displacement } => {
                    ops::system::exec_bra(self, displacement, memory)
                }
                SystemInstruction::Bsr { displacement } => {
                    ops::system::exec_bsr(self, displacement, memory)
                }
                SystemInstruction::Bcc {
                    condition,
                    displacement,
                } => ops::system::exec_bcc(self, condition, displacement, memory),
                SystemInstruction::Scc { condition, dst } => {
                    ops::system::exec_scc(self, condition, dst, memory)
                }
                SystemInstruction::DBcc { condition, reg } => {
                    ops::system::exec_dbcc(self, condition, reg, memory)
                }
                SystemInstruction::Jmp { dst } => ops::system::exec_jmp(self, dst, memory),
                SystemInstruction::Jsr { dst } => ops::system::exec_jsr(self, dst, memory),
                SystemInstruction::Rts => ops::system::exec_rts(self, memory),
                SystemInstruction::Rte => ops::system::exec_rte(self, memory),
                SystemInstruction::Rtr => ops::system::exec_rtr(self, memory),
                SystemInstruction::Nop => 4,
                SystemInstruction::Reset => 132,
                SystemInstruction::Stop => ops::system::exec_stop(self, memory),
                SystemInstruction::MoveUsp { reg, to_usp } => {
                    ops::system::exec_move_usp(self, reg, to_usp, memory)
                }
                SystemInstruction::Trap { vector } => ops::system::exec_trap(self, vector, memory),
                SystemInstruction::TrapV => {
                    if self.get_flag(flags::OVERFLOW) {
                        self.process_exception(7, memory)
                    } else {
                        4
                    }
                }
                SystemInstruction::Link { reg } => {
                    let displacement = self.read_word(self.pc, memory) as i16;
                    self.pc = self.pc.wrapping_add(2);
                    ops::system::exec_link(self, reg, displacement, memory)
                }
                SystemInstruction::Unlk { reg } => ops::system::exec_unlk(self, reg, memory),
                SystemInstruction::MoveToSr { src } => {
                    ops::system::exec_move_to_sr(self, src, memory)
                }
                SystemInstruction::MoveFromSr { dst } => {
                    ops::system::exec_move_from_sr(self, dst, memory)
                }
                SystemInstruction::MoveToCcr { src } => {
                    ops::system::exec_move_to_ccr(self, src, memory)
                }
                SystemInstruction::AndiToCcr => ops::system::exec_andi_to_ccr(self, memory),
                SystemInstruction::AndiToSr => ops::system::exec_andi_to_sr(self, memory),
                SystemInstruction::OriToCcr => ops::system::exec_ori_to_ccr(self, memory),
                SystemInstruction::OriToSr => ops::system::exec_ori_to_sr(self, memory),
                SystemInstruction::EoriToCcr => ops::system::exec_eori_to_ccr(self, memory),
                SystemInstruction::EoriToSr => ops::system::exec_eori_to_sr(self, memory),
                SystemInstruction::Illegal => self.process_exception(4, memory),
                SystemInstruction::LineA { opcode: _ } => self.process_exception(10, memory),
                SystemInstruction::LineF { opcode: _ } => self.process_exception(11, memory),
                SystemInstruction::Unimplemented { opcode: _ } => {
                    self.process_exception(4, memory) // Illegal instruction
                }
            },
        }
    }

    fn check_interrupts<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        if self.pending_interrupt == 0 {
            return 0;
        }

        let current_mask = (self.sr & flags::INTERRUPT_MASK) >> 8;

        if self.pending_interrupt > current_mask as u8 || self.pending_interrupt == 7 {
            let level = self.pending_interrupt;
            self.acknowledge_interrupt(level);
            self.halted = false;

            let old_sr = self.sr;
            let mut new_sr = old_sr | flags::SUPERVISOR;
            new_sr &= !flags::TRACE;
            new_sr = (new_sr & !flags::INTERRUPT_MASK) | ((level as u16) << 8);

            self.set_sr(new_sr);
            self.push_long(self.pc, memory);
            self.push_word(old_sr, memory);

            let vector = 24 + level as u32;
            let vector_addr = vector * 4;
            self.pc = memory.read_long(vector_addr);

            return 44;
        }

        0
    }

    pub fn read_word<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u16 {
        if !addr.is_multiple_of(2) {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_word(addr)
    }

    pub fn read_long<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u32 {
        if !addr.is_multiple_of(2) {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_long(addr)
    }

    pub fn write_byte<M: MemoryInterface>(&mut self, addr: u32, val: u8, memory: &mut M) {
        memory.write_byte(addr, val);
        self.invalidate_cache_line(addr);
    }

    pub fn write_word<M: MemoryInterface>(&mut self, addr: u32, val: u16, memory: &mut M) {
        if !addr.is_multiple_of(2) {
            self.process_exception(3, memory);
            return;
        }
        memory.write_word(addr, val);
        self.invalidate_cache_line(addr);
    }

    pub fn write_long<M: MemoryInterface>(&mut self, addr: u32, val: u32, memory: &mut M) {
        if !addr.is_multiple_of(2) {
            self.process_exception(3, memory);
            return;
        }
        memory.write_long(addr, val);
        self.invalidate_cache_line(addr);
        self.invalidate_cache_line(addr.wrapping_add(2));
    }

    pub fn cpu_read_memory<M: MemoryInterface>(
        &mut self,
        addr: u32,
        size: Size,
        memory: &mut M,
    ) -> u32 {
        match size {
            Size::Byte => memory.read_byte(addr) as u32,
            Size::Word => self.read_word(addr, memory) as u32,
            Size::Long => self.read_long(addr, memory),
        }
    }

    pub fn cpu_write_memory<M: MemoryInterface>(
        &mut self,
        addr: u32,
        size: Size,
        val: u32,
        memory: &mut M,
    ) {
        match size {
            Size::Byte => self.write_byte(addr, val as u8, memory),
            Size::Word => self.write_word(addr, val as u16, memory),
            Size::Long => self.write_long(addr, val, memory),
        }
    }

    pub fn cpu_read_ea<M: MemoryInterface>(
        &mut self,
        ea: addressing::EffectiveAddress,
        size: Size,
        memory: &mut M,
    ) -> u32 {
        if let addressing::EffectiveAddress::Memory(addr) = ea {
            if size != Size::Byte && addr % 2 != 0 {
                self.process_exception(3, memory);
                return 0;
            }
        }
        addressing::read_ea(ea, size, &self.d, &self.a, memory)
    }

    pub fn cpu_write_ea<M: MemoryInterface>(
        &mut self,
        ea: addressing::EffectiveAddress,
        size: Size,
        val: u32,
        memory: &mut M,
    ) {
        match ea {
            addressing::EffectiveAddress::DataRegister(r) => {
                let reg = r as usize;
                match size {
                    Size::Byte => self.d[reg] = (self.d[reg] & 0xFFFFFF00) | (val & 0xFF),
                    Size::Word => self.d[reg] = (self.d[reg] & 0xFFFF0000) | (val & 0xFFFF),
                    Size::Long => self.d[reg] = val,
                }
            }
            addressing::EffectiveAddress::AddressRegister(r) => {
                let reg = r as usize;
                match size {
                    Size::Byte => self.a[reg] = (val as i8) as i32 as u32,
                    Size::Word => self.a[reg] = (val as i16) as i32 as u32,
                    Size::Long => self.a[reg] = val,
                }
            }
            addressing::EffectiveAddress::Memory(addr) => {
                if size != Size::Byte && addr % 2 != 0 {
                    self.process_exception(3, memory);
                    return;
                }
                self.cpu_write_memory(addr, size, val, memory);
            }
        }
    }

    pub fn fetch_bit_num<M: MemoryInterface>(&mut self, bit: BitSource, memory: &mut M) -> u8 {
        match bit {
            BitSource::Immediate => {
                let val = self.read_word(self.pc, memory) as u8;
                self.pc = self.pc.wrapping_add(2);
                val
            }
            BitSource::Register(r) => (self.d[r as usize] & 0xFF) as u8,
        }
    }

    pub fn resolve_bit_index(&self, bit_num: u8, is_memory: bool) -> u8 {
        if is_memory {
            bit_num % 8
        } else {
            bit_num % 32
        }
    }

    pub fn test_condition(&self, condition: Condition) -> bool {
        let n = self.get_flag(flags::NEGATIVE);
        let z = self.get_flag(flags::ZERO);
        let v = self.get_flag(flags::OVERFLOW);
        let c = self.get_flag(flags::CARRY);

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
            Condition::GreaterOrEqual => (n && v) || (!n && !v),
            Condition::LessThan => (n && !v) || (!n && v),
            Condition::GreaterThan => (n && v && !z) || (!n && !v && !z),
            Condition::LessOrEqual => z || (n && !v) || (!n && v),
        }
    }
}

#[cfg(test)]
mod tests_addressing;
#[cfg(test)]
mod tests_bug_fixes;
#[cfg(test)]
mod tests_cache;
#[cfg(test)]
mod tests_m68k_alu;
#[cfg(test)]
mod tests_m68k_bcd;
#[cfg(test)]
mod tests_m68k_bits;
#[cfg(test)]
mod tests_m68k_comprehensive;
#[cfg(test)]
mod tests_m68k_control;
#[cfg(test)]
mod tests_m68k_data;
#[cfg(test)]
mod tests_m68k_data_unit;
#[cfg(test)]
mod tests_m68k_extended;
#[cfg(test)]
mod tests_m68k_shift;
#[cfg(test)]
mod tests_m68k_torture;
#[cfg(test)]
mod tests_performance;
#[cfg(test)]
mod tests_security;
mod tests_decoder_shift;
