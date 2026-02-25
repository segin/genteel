use crate::memory::MemoryInterface;
use serde::{Deserialize, Serialize};

pub mod addressing;
pub mod decoder;
pub mod instructions;
pub mod ops;

pub use addressing::EffectiveAddress;
pub use decoder::{Condition, Size, decode};
use instructions::{
    ArithmeticInstruction, BitSource, BitsInstruction, DataInstruction, DecodeCacheEntry, Instruction,
    SystemInstruction,
};

const CACHE_ROM_LIMIT: u32 = 0x400000; // 4MB ROM
const CACHE_MASK: u32 = 0x1FFFFF; // 2M entries

pub struct Cpu {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
    pub usp: u32,
    pub ssp: u32,
    pub halted: bool,
    pub pending_interrupt: u8,
    pub interrupt_pending_mask: u8,
    pub pending_exception: bool,
    pub cycles: u64,
    pub decode_cache: Box<[DecodeCacheEntry]>,
}

pub mod flags {
    pub const CARRY: u16 = 0x0001;
    pub const OVERFLOW: u16 = 0x0002;
    pub const ZERO: u16 = 0x0004;
    pub const NEGATIVE: u16 = 0x0008;
    pub const EXTEND: u16 = 0x0010;
    pub const INTERRUPT_MASK: u16 = 0x0700;
    pub const MASTER_STATE: u16 = 0x1000;
    pub const SUPERVISOR: u16 = 0x2000;
    pub const TRACE: u16 = 0x8000;
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CpuState {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
    pub halted: bool,
    pub pending_interrupt: u8,
}

impl Cpu {
    pub fn new<M: MemoryInterface>(memory: &mut M) -> Self {
        let ssp = memory.read_long(0);
        let pc = memory.read_long(4);
        let cache_size = (CACHE_MASK + 1) as usize;
        let cache = vec![DecodeCacheEntry::default(); cache_size].into_boxed_slice();

        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc,
            sr: 0x2700,
            usp: 0,
            ssp,
            halted: false,
            pending_interrupt: 0,
            interrupt_pending_mask: 0,
            pending_exception: false,
            cycles: 0,
            decode_cache: cache,
        };
        cpu.a[7] = ssp;
        cpu
    }

    pub fn reset<M: MemoryInterface>(&mut self, memory: &mut M) {
        self.ssp = memory.read_long(0);
        self.pc = memory.read_long(4);
        self.sr = 0x2700;
        self.a[7] = self.ssp;
        self.halted = false;
        self.pending_interrupt = 0;
        self.interrupt_pending_mask = 0;
        self.pending_exception = false;
        self.invalidate_cache();
    }

    pub fn cpu_read_ea<M: MemoryInterface>(&mut self, ea: EffectiveAddress, size: Size, memory: &mut M) -> u32 {
        if let EffectiveAddress::Memory(addr) = ea {
            if (size == Size::Word || size == Size::Long) && addr % 2 != 0 {
                self.process_exception(3, memory);
                return 0;
            }
        }
        addressing::read_ea(ea, size, &self.d, &self.a, memory)
    }

    pub fn cpu_write_ea<M: MemoryInterface>(&mut self, ea: EffectiveAddress, size: Size, value: u32, memory: &mut M) {
        if let EffectiveAddress::Memory(addr) = ea {
            if (size == Size::Word || size == Size::Long) && addr % 2 != 0 {
                self.process_exception(3, memory);
                return;
            }
        }
        addressing::write_ea(ea, size, value, &mut self.d, &mut self.a, memory)
    }

    pub fn cpu_read_memory<M: MemoryInterface>(&mut self, addr: u32, size: Size, memory: &mut M) -> u32 {
        if (size == Size::Word || size == Size::Long) && addr % 2 != 0 {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_size(addr, size)
    }

    pub fn cpu_write_memory<M: MemoryInterface>(&mut self, addr: u32, size: Size, value: u32, memory: &mut M) {
        if (size == Size::Word || size == Size::Long) && addr % 2 != 0 {
            self.process_exception(3, memory);
            return;
        }
        memory.write_size(addr, value, size)
    }

    fn check_interrupts<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        let mask = (self.sr & flags::INTERRUPT_MASK) >> 8;
        if self.pending_interrupt > mask as u8 {
            let level = self.pending_interrupt;
            let vector = 24 + level as u32;
            let cycles = self.process_exception(vector, memory);
            self.sr = (self.sr & !flags::INTERRUPT_MASK) | ((level as u16) << 8);
            self.acknowledge_interrupt(level);
            return 44;
        }
        0
    }

    pub fn invalidate_cache(&mut self) {
        self.decode_cache.fill(DecodeCacheEntry::default());
    }

    pub fn request_interrupt(&mut self, level: u8) {
        if level > 0 && level <= 7 {
            self.interrupt_pending_mask |= 1 << level;
            self.update_pending_interrupt();
        }
    }

    pub fn get_state(&self) -> CpuState {
        CpuState {
            d: self.d,
            a: self.a,
            pc: self.pc,
            sr: self.sr,
            halted: self.halted,
            pending_interrupt: self.pending_interrupt,
        }
    }

    pub fn set_state(&mut self, state: CpuState) {
        self.d = state.d;
        self.a = state.a;
        self.pc = state.pc;
        self.sr = state.sr;
        self.halted = state.halted;
        self.pending_interrupt = state.pending_interrupt;
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

    pub fn read_word<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u16 {
        if addr % 2 != 0 {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_word(addr)
    }

    pub fn read_long<M: MemoryInterface>(&mut self, addr: u32, memory: &mut M) -> u32 {
        if addr % 2 != 0 {
            self.process_exception(3, memory);
            return 0;
        }
        memory.read_long(addr)
    }

    pub fn write_word<M: MemoryInterface>(&mut self, addr: u32, val: u16, memory: &mut M) {
        if addr % 2 != 0 {
            self.process_exception(3, memory);
            return;
        }
        memory.write_word(addr, val);
    }

    pub fn write_long<M: MemoryInterface>(&mut self, addr: u32, val: u32, memory: &mut M) {
        if addr % 2 != 0 {
            self.process_exception(3, memory);
            return;
        }
        memory.write_long(addr, val);
    }

    pub fn write_byte<M: MemoryInterface>(&mut self, addr: u32, val: u8, memory: &mut M) {
        memory.write_byte(addr, val);
    }

    pub fn test_condition(&self, condition: Condition) -> bool {
        self.check_condition(condition)
    }

    pub fn fetch_bit_num<M: MemoryInterface>(&mut self, source: BitSource, memory: &mut M) -> u32 {
        match source {
            BitSource::Immediate => {
                let word = self.read_instruction_word(self.pc, memory);
                self.pc = self.pc.wrapping_add(2);
                (word & 0xFF) as u32
            }
            BitSource::Register(reg) => self.d[reg as usize],
        }
    }

    pub fn resolve_bit_index(&self, bit_num: u32, is_memory: bool) -> u32 {
        if is_memory {
            bit_num % 8
        } else {
            bit_num % 32
        }
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

        let instruction = match self.fetch_next_instruction(memory) {
            Some(instr) => instr,
            None => {
                // Address Error during fetch
                self.cycles += 34;
                return 34;
            }
        };

        let cycles = self.execute(instruction, memory);
        self.cycles += cycles as u64;
        cycles
    }

    fn fetch_next_instruction<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
    ) -> Option<Instruction> {
        let pc = self.pc;

        // Optimized instruction fetch with cache
        if pc < CACHE_ROM_LIMIT {
            // ROM/Cartridge space - Cacheable
            // Index: (PC / 2) & CACHE_MASK. Maps 0-128KB repeating or just lower bits.
            // Since we check entry.pc == pc, aliasing is handled safely.
            let cache_index = ((pc >> 1) & CACHE_MASK) as usize;

            // Try to read from cache safely
            // If the cache has been resized to be smaller than CACHE_SIZE, get() returns None
            // and we fall back to uncached fetch, preventing out-of-bounds access.
            if let Some(entry) = self.decode_cache.get(cache_index).copied() {
                if entry.pc == pc {
                    // Cache Hit
                    self.pc = pc.wrapping_add(2);
                    return Some(entry.instruction);
                }

                // Cache Miss
                let opcode = self.read_instruction_word(pc, memory);
                if self.pending_exception {
                    return None;
                }

                self.pc = self.pc.wrapping_add(2);
                let instruction = decode(opcode);

                // Update Cache
                // We know the index is valid because get() succeeded earlier,
                // but we use get_mut() for safety in case of concurrent modification (unlikely here)
                // or weird edge cases.
                if let Some(entry_mut) = self.decode_cache.get_mut(cache_index) {
                    *entry_mut = DecodeCacheEntry { pc, instruction };
                }
                return Some(instruction);
            }
        }

        // Uncached (RAM, I/O, etc.) or Cache index out of bounds
        let opcode = self.read_instruction_word(pc, memory);
        if self.pending_exception {
            return None;
        }

        self.pc = self.pc.wrapping_add(2);
        Some(decode(opcode))
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
                    ops::data::exec_moveq(self, dst_reg, data as u8)
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
                BitsInstruction::AndToCcr => ops::system::exec_andi_to_ccr(self, memory),
                BitsInstruction::AndToSr => ops::system::exec_andi_to_sr(self, memory),
                BitsInstruction::Or {
                    size,
                    src,
                    dst,
                    direction,
                } => ops::bits::exec_or(self, size, src, dst, direction, memory),
                BitsInstruction::OrI { size, dst } => ops::bits::exec_ori(self, size, dst, memory),
                BitsInstruction::OrToCcr => ops::system::exec_ori_to_ccr(self, memory),
                BitsInstruction::OrToSr => ops::system::exec_ori_to_sr(self, memory),
                BitsInstruction::Eor { size, src_reg, dst } => {
                    ops::bits::exec_eor(self, size, src_reg, dst, memory)
                }
                BitsInstruction::EorI { size, dst } => {
                    ops::bits::exec_eori(self, size, dst, memory)
                }
                BitsInstruction::EorToCcr => ops::system::exec_eori_to_ccr(self, memory),
                BitsInstruction::EorToSr => ops::system::exec_eori_to_sr(self, memory),
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
                SystemInstruction::Reset => ops::system::exec_reset(self, memory),
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

    pub fn check_condition(&self, cond: Condition) -> bool {
        let z = self.get_flag(flags::ZERO);
        let c = self.get_flag(flags::CARRY);
        let n = self.get_flag(flags::NEGATIVE);
        let v = self.get_flag(flags::OVERFLOW);

        match cond {
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
mod bench_decoder;
#[cfg(test)]
mod tests_addressing;
#[cfg(test)]
mod tests_bug_fixes;
#[cfg(test)]
mod tests_cache;
#[cfg(test)]
mod tests_decoder_shift;
#[cfg(test)]
mod tests_interrupts;
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
mod tests_m68k_exhaustive;
#[cfg(test)]
mod tests_m68k_extended;
#[cfg(test)]
mod tests_m68k_movep;
#[cfg(test)]
mod tests_m68k_shift;
#[cfg(test)]
mod tests_m68k_torture;
#[cfg(test)]
mod tests_performance;
#[cfg(test)]
mod tests_security;
