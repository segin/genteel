//! Motorola 68000 core.
//!
//! Each instruction is modelled as the sequence of bus cycles the real chip
//! performs: program fetches ("prefetch", 4 clocks each), data reads and
//! writes (4 clocks per word) and internal idle cycles. Executing that
//! sequence in hardware order yields exact instruction timing and, when a
//! word or long access hits an odd address, lets the instruction abort at
//! exactly the point the hardware does, with the matching group 0 exception
//! frame.

use crate::memory::MemoryInterface;
use serde::{Deserialize, Serialize};
use std::sync::OnceLock;

pub mod addressing;
pub mod decoder;
pub mod instructions;
pub mod ops;

#[cfg(test)]
pub mod test_utils;

pub use addressing::EffectiveAddress;
pub use decoder::{decode, Condition, Size};
use instructions::{
    ArithmeticInstruction, BitsInstruction, DataInstruction, Instruction, SystemInstruction,
};

/// Bits of SR that exist on the 68000 (T, S, IPL2-0, X, N, Z, V, C).
pub const SR_MASK: u16 = 0xA71F;
/// Bits of the CCR that exist on the 68000 (X, N, Z, V, C).
pub const CCR_MASK: u16 = 0x001F;

/// Exception vector numbers used by the core.
pub mod vectors {
    pub const ADDRESS_ERROR: u32 = 3;
    pub const ILLEGAL_INSTRUCTION: u32 = 4;
    pub const ZERO_DIVIDE: u32 = 5;
    pub const CHK: u32 = 6;
    pub const TRAPV: u32 = 7;
    pub const PRIVILEGE_VIOLATION: u32 = 8;
    pub const TRACE: u32 = 9;
    pub const LINE_A: u32 = 10;
    pub const LINE_F: u32 = 11;
    pub const AUTOVECTOR_BASE: u32 = 24;
    pub const TRAP_BASE: u32 = 32;
}

/// An odd-address word/long access that aborts the current instruction.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct AddressFault {
    /// The full 32-bit address of the faulting access.
    pub addr: u32,
    /// True for a write cycle.
    pub write: bool,
    /// True for a program-space (instruction stream) access.
    pub program: bool,
}

/// Result of a step of instruction execution that may abort on a fault.
pub type Exec<T = ()> = Result<T, AddressFault>;

#[derive(Serialize, Deserialize)]
pub struct Cpu {
    pub d: [u32; 8],
    pub a: [u32; 8],
    pub pc: u32,
    pub sr: u16,
    pub usp: u32,
    pub ssp: u32,
    /// Halted after a double fault; only a reset leaves this state.
    pub halted: bool,
    /// Stopped by STOP; an accepted interrupt resumes execution.
    #[serde(default)]
    pub stopped: bool,
    pub pending_interrupt: u8,
    pub interrupt_pending_mask: u8,
    pub last_interrupt_level: u8,
    pub cycles: u64,

    /* Per-instruction transient state. */
    /// Opcode of the instruction being executed (the IR register).
    #[serde(skip)]
    pub(crate) ir: u16,
    /// Address of the opcode of the instruction being executed.
    #[serde(skip)]
    pub(crate) pc0: u32,
    /// Clock cycles consumed so far by the current instruction.
    #[serde(skip)]
    pub(crate) icycles: u32,
    /// Program fetches issued so far by the current instruction.
    #[serde(skip)]
    pub(crate) fetches: u32,
    /// Set once the current instruction has started exception processing.
    #[serde(skip)]
    pub(crate) exception_taken: bool,
    /// Override for the PC stacked by an address error (used where the
    /// hardware's program counter is not simply opcode + 2 * fetches).
    #[serde(skip)]
    pub(crate) fault_pc: Option<u32>,
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
    pub usp: u32,
    pub ssp: u32,
    pub halted: bool,
    pub stopped: bool,
    pub pending_interrupt: u8,
    pub interrupt_pending_mask: u8,
}

/// The process-wide opcode decode table (64K entries, built once).
fn decode_table() -> &'static [Instruction] {
    static TABLE: OnceLock<Box<[Instruction]>> = OnceLock::new();
    TABLE.get_or_init(|| (0..=0xFFFFu32).map(|op| decode(op as u16)).collect())
}

impl Cpu {
    pub fn new<M: MemoryInterface>(memory: &mut M) -> Self {
        // Build the shared decode table up front so the first step is cheap.
        let _ = decode_table();
        let ssp = memory.read_long(0);
        let pc = memory.read_long(4);
        let mut cpu = Self {
            d: [0; 8],
            a: [0; 8],
            pc,
            sr: 0x2700,
            usp: 0,
            ssp,
            halted: false,
            stopped: false,
            pending_interrupt: 0,
            interrupt_pending_mask: 0,
            last_interrupt_level: 0,
            cycles: 0,
            ir: 0,
            pc0: pc,
            icycles: 0,
            fetches: 0,
            exception_taken: false,
            fault_pc: None,
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
        self.stopped = false;
        self.pending_interrupt = 0;
        self.interrupt_pending_mask = 0;
        self.last_interrupt_level = 0;
    }

    pub fn request_interrupt(&mut self, level: u8) {
        if level > 0 && level <= 7 {
            self.interrupt_pending_mask |= 1 << level;
            self.update_pending_interrupt();
        }
    }

    pub fn cancel_interrupt(&mut self, level: u8) {
        if level > 0 && level <= 7 {
            self.interrupt_pending_mask &= !(1 << level);
            self.update_pending_interrupt();
        }
    }

    pub fn get_state(&self) -> CpuState {
        CpuState {
            d: self.d,
            a: self.a,
            pc: self.pc,
            sr: self.sr,
            usp: self.usp,
            ssp: self.ssp,
            halted: self.halted,
            stopped: self.stopped,
            pending_interrupt: self.pending_interrupt,
            interrupt_pending_mask: self.interrupt_pending_mask,
        }
    }

    pub fn set_state(&mut self, state: CpuState) {
        self.d = state.d;
        self.a = state.a;
        self.pc = state.pc;
        self.sr = state.sr;
        self.usp = state.usp;
        self.ssp = state.ssp;
        self.halted = state.halted;
        self.stopped = state.stopped;
        self.pending_interrupt = state.pending_interrupt;
        self.interrupt_pending_mask = state.interrupt_pending_mask;
    }

    /// Update pending_interrupt to the highest priority level in the mask.
    fn update_pending_interrupt(&mut self) {
        self.pending_interrupt = (1..=7u8)
            .rev()
            .find(|&l| self.interrupt_pending_mask & (1 << l) != 0)
            .unwrap_or(0);
    }

    /// Acknowledge an interrupt (called after processing).
    pub fn acknowledge_interrupt(&mut self, level: u8) {
        if level > 7 {
            return;
        }
        self.interrupt_pending_mask &= !(1 << level);
        self.update_pending_interrupt();
    }

    pub fn test_condition(&self, condition: Condition) -> bool {
        self.check_condition(condition)
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
        let val = val & size.mask();
        self.set_flag(flags::ZERO, val == 0);
        self.set_flag(flags::NEGATIVE, size.is_negative(val));
    }

    /// Set N and Z from `val`, clear V and C (the common logic-op result).
    pub(crate) fn set_logic_flags(&mut self, val: u32, size: Size) {
        self.update_nz_flags(val, size);
        self.sr &= !(flags::OVERFLOW | flags::CARRY);
    }

    pub fn add_with_flags(&self, src: u32, dst: u32, size: Size) -> (u32, bool, bool) {
        self.addx_with_flags(src, dst, 0, size)
    }

    pub fn sub_with_flags(&self, dst: u32, src: u32, size: Size) -> (u32, bool, bool) {
        self.subx_with_flags(dst, src, 0, size)
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

    /// Write SR, keeping only the implemented bits and switching stacks when
    /// the supervisor bit changes.
    pub fn set_sr(&mut self, val: u16) {
        let old_sr = self.sr;
        self.sr = val & SR_MASK;
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

    /// Write the CCR (low byte of SR), keeping only the implemented bits.
    pub fn set_ccr(&mut self, val: u16) {
        self.sr = (self.sr & 0xFF00) | (val & CCR_MASK);
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
            Condition::GreaterOrEqual => n == v,
            Condition::LessThan => n != v,
            Condition::GreaterThan => !z && (n == v),
            Condition::LessOrEqual => z || (n != v),
        }
    }

    /* ------------------------------------------------------------------ */
    /* Bus cycle primitives                                                */
    /* ------------------------------------------------------------------ */

    /// Internal (non-bus) clock cycles.
    #[inline]
    pub(crate) fn idle(&mut self, n: u32) {
        self.icycles += n;
    }

    /// One program fetch into the prefetch queue (4 clocks).
    #[inline]
    pub(crate) fn prefetch(&mut self) {
        self.icycles += 4;
        self.fetches += 1;
    }

    /// Consume the next extension word from the instruction stream; the
    /// queue refill costs one program fetch.
    #[inline]
    pub(crate) fn ext_word<M: MemoryInterface>(&mut self, memory: &mut M) -> u16 {
        let w = memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        self.prefetch();
        w
    }

    /// Consume the next extension word without refilling the queue yet (the
    /// caller issues the matching `prefetch` later).
    #[inline]
    pub(crate) fn ext_word_deferred<M: MemoryInterface>(&mut self, memory: &mut M) -> u16 {
        let w = memory.read_word(self.pc);
        self.pc = self.pc.wrapping_add(2);
        w
    }

    #[inline]
    pub(crate) fn ext_long<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        let hi = self.ext_word(memory) as u32;
        let lo = self.ext_word(memory) as u32;
        (hi << 16) | lo
    }

    #[inline]
    fn check_align(addr: u32, size: Size, write: bool) -> Exec {
        if size != Size::Byte && addr & 1 != 0 {
            Err(AddressFault {
                addr,
                write,
                program: false,
            })
        } else {
            Ok(())
        }
    }

    /// Data read of `size` at `addr`.
    #[inline]
    pub(crate) fn read<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
        addr: u32,
        size: Size,
    ) -> Exec<u32> {
        Self::check_align(addr, size, false)?;
        Ok(match size {
            Size::Byte => {
                self.icycles += 4;
                memory.read_byte(addr) as u32
            }
            Size::Word => {
                self.icycles += 4;
                memory.read_word(addr) as u32
            }
            Size::Long => {
                self.icycles += 8;
                memory.read_long(addr)
            }
        })
    }

    /// Data write of `size` at `addr`.
    #[inline]
    pub(crate) fn write<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
        addr: u32,
        size: Size,
        val: u32,
    ) -> Exec {
        Self::check_align(addr, size, true)?;
        match size {
            Size::Byte => {
                self.icycles += 4;
                memory.write_byte(addr, val as u8)
            }
            Size::Word => {
                self.icycles += 4;
                memory.write_word(addr, val as u16)
            }
            Size::Long => {
                self.icycles += 8;
                memory.write_long(addr, val)
            }
        }
        Ok(())
    }

    pub(crate) fn push_long<M: MemoryInterface>(&mut self, memory: &mut M, val: u32) -> Exec {
        let sp = self.a[7].wrapping_sub(4);
        self.write(memory, sp, Size::Long, val)?;
        self.a[7] = sp;
        Ok(())
    }

    pub(crate) fn push_word<M: MemoryInterface>(&mut self, memory: &mut M, val: u16) -> Exec {
        let sp = self.a[7].wrapping_sub(2);
        self.write(memory, sp, Size::Word, val as u32)?;
        self.a[7] = sp;
        Ok(())
    }

    pub(crate) fn pop_long<M: MemoryInterface>(&mut self, memory: &mut M) -> Exec<u32> {
        let v = self.read(memory, self.a[7], Size::Long)?;
        self.a[7] = self.a[7].wrapping_add(4);
        Ok(v)
    }

    pub(crate) fn pop_word<M: MemoryInterface>(&mut self, memory: &mut M) -> Exec<u16> {
        let v = self.read(memory, self.a[7], Size::Word)? as u16;
        self.a[7] = self.a[7].wrapping_add(2);
        Ok(v)
    }

    /// Load PC with a branch/jump target. An odd target faults on the
    /// program fetch from that address, before any fetch is made.
    #[inline]
    pub(crate) fn jump_to(&mut self, target: u32) -> Exec {
        if target & 1 != 0 {
            return Err(AddressFault {
                addr: target,
                write: false,
                program: true,
            });
        }
        self.pc = target;
        Ok(())
    }

    /// Jump and refill the two-word prefetch queue at the target.
    #[inline]
    pub(crate) fn jump_and_fill(&mut self, target: u32) -> Exec {
        self.jump_to(target)?;
        self.prefetch();
        self.prefetch();
        Ok(())
    }

    /* ------------------------------------------------------------------ */
    /* Exception processing                                                */
    /* ------------------------------------------------------------------ */

    /// Enter supervisor state for exception processing; returns the SR value
    /// to stack.
    fn enter_exception_state(&mut self) -> u16 {
        let old_sr = self.sr;
        if old_sr & flags::SUPERVISOR == 0 {
            self.usp = self.a[7];
            self.a[7] = self.ssp;
        }
        self.sr = (self.sr | flags::SUPERVISOR) & !flags::TRACE;
        self.exception_taken = true;
        old_sr
    }

    /// Fetch the handler address for `vector` and start executing it.
    fn vector_jump<M: MemoryInterface>(&mut self, memory: &mut M, vector: u32) -> Exec {
        let handler = self.read(memory, vector * 4, Size::Long)?;
        self.jump_to(handler)?;
        self.prefetch();
        self.idle(2);
        self.prefetch();
        Ok(())
    }

    /// Group 1/2 exception (trap, illegal, privilege, CHK, zero divide,
    /// trace, interrupt): stack PC and SR, then vector. Costs 30 clocks on
    /// top of whatever the caller has already spent.
    pub(crate) fn exception<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
        vector: u32,
        stacked_pc: u32,
    ) -> Exec {
        let old_sr = self.enter_exception_state();
        self.push_long(memory, stacked_pc)?;
        self.push_word(memory, old_sr)?;
        self.vector_jump(memory, vector)
    }

    /// Exception for an instruction that is not allowed to execute (illegal,
    /// Line A/F, privilege violation): the faulting opcode's address is
    /// stacked.
    pub(crate) fn exception_at_opcode<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
        vector: u32,
    ) -> Exec {
        self.idle(4);
        let pc = self.pc0;
        self.exception(memory, vector, pc)
    }

    /// Returns true when in supervisor state; otherwise takes the privilege
    /// violation exception and returns false.
    pub(crate) fn require_supervisor<M: MemoryInterface>(&mut self, memory: &mut M) -> Exec<bool> {
        if self.sr & flags::SUPERVISOR != 0 {
            Ok(true)
        } else {
            self.exception_at_opcode(memory, vectors::PRIVILEGE_VIOLATION)?;
            Ok(false)
        }
    }

    /// Group 0 (address error) exception: stack the 7-word frame.
    pub(crate) fn address_error<M: MemoryInterface>(
        &mut self,
        memory: &mut M,
        fault: AddressFault,
    ) {
        let supervisor = self.sr & flags::SUPERVISOR != 0;
        let fc: u16 = (if supervisor { 4 } else { 0 }) | (if fault.program { 2 } else { 1 });
        /* Status word: the undefined upper bits hold IR, then R/W (1 = read),
        I/N (set for a program fetch on this core) and the function code. */
        let status: u16 = (self.ir & 0xFFE0)
            | (if fault.write { 0 } else { 0x10 })
            | (if fault.program { 0x08 } else { 0 })
            | fc;
        /* The stacked PC is the hardware program counter at the fault: the
        opcode address advanced by one word per program fetch issued, or for
        a failed fetch from a branch target, that target minus 4. */
        let stacked_pc = if fault.program {
            fault.addr.wrapping_sub(4)
        } else {
            self.fault_pc
                .unwrap_or_else(|| self.pc0.wrapping_add(2 * self.fetches))
        };
        self.idle(4);
        let old_sr = self.enter_exception_state();
        let ir = self.ir;
        let result = (|| -> Exec {
            self.push_long(memory, stacked_pc)?;
            self.push_word(memory, old_sr)?;
            self.push_word(memory, ir)?;
            self.push_long(memory, fault.addr)?;
            self.push_word(memory, status)?;
            self.vector_jump(memory, vectors::ADDRESS_ERROR)
        })();
        if result.is_err() {
            /* A fault while stacking a group 0 frame is a double fault: the
            processor halts until reset. */
            log::error!("68000 double fault at PC={:08X}; halting", self.pc0);
            self.halted = true;
        }
    }

    /// Accept a pending interrupt if its level beats the mask. Returns true
    /// when interrupt processing was started.
    fn check_interrupts<M: MemoryInterface>(&mut self, memory: &mut M) -> bool {
        let mask = ((self.sr & flags::INTERRUPT_MASK) >> 8) as u8;
        let level = self.pending_interrupt;
        if level == 0 || (level <= mask && level != 7) {
            return false;
        }
        self.stopped = false;
        self.icycles = 0;
        self.fetches = 0;
        self.exception_taken = false;
        self.fault_pc = None;
        self.pc0 = self.pc;
        /* Interrupt acknowledge: 6 internal clocks plus the 4-clock IACK
        cycle and autovector wait states, then group 1 processing. */
        self.idle(14);
        let old_sr = self.enter_exception_state();
        let pc = self.pc;
        let result = (|| -> Exec {
            self.push_long(memory, pc)?;
            self.push_word(memory, old_sr)?;
            self.sr = (self.sr & !flags::INTERRUPT_MASK) | ((level as u16) << 8);
            self.vector_jump(memory, vectors::AUTOVECTOR_BASE + level as u32)
        })();
        if let Err(fault) = result {
            self.address_error(memory, fault);
        }
        self.acknowledge_interrupt(level);
        self.last_interrupt_level = level;
        true
    }

    /// Execute one instruction (or accept one interrupt) and return the
    /// number of clock cycles it took.
    pub fn step_instruction<M: MemoryInterface>(&mut self, memory: &mut M) -> u32 {
        self.last_interrupt_level = 0;

        if self.halted {
            /* Double-fault halt: only reset recovers; interrupts are ignored. */
            self.cycles += 4;
            return 4;
        }

        if self.check_interrupts(memory) {
            let total = self.icycles + memory.take_pending_stall_cycles();
            self.cycles += total as u64;
            return total;
        }

        if self.stopped {
            self.cycles += 4;
            return 4;
        }

        self.execute_one(memory);
        let total = self.icycles + memory.take_pending_stall_cycles();
        self.cycles += total as u64;
        total
    }

    fn execute_one<M: MemoryInterface>(&mut self, memory: &mut M) {
        self.icycles = 0;
        self.fetches = 0;
        self.exception_taken = false;
        self.fault_pc = None;
        self.pc0 = self.pc;
        let trace = self.sr & flags::TRACE != 0;

        if self.pc & 1 != 0 {
            let fault = AddressFault {
                addr: self.pc,
                write: false,
                program: true,
            };
            self.address_error(memory, fault);
            return;
        }

        let opcode = memory.read_word(self.pc);
        self.ir = opcode;
        self.pc = self.pc.wrapping_add(2);
        let instruction = decode_table()[opcode as usize];

        match self.execute(instruction, memory) {
            Ok(()) => {
                if trace && !self.exception_taken {
                    self.idle(4);
                    let pc = self.pc;
                    if let Err(fault) = self.exception(memory, vectors::TRACE, pc) {
                        self.address_error(memory, fault);
                    }
                }
            }
            Err(fault) => self.address_error(memory, fault),
        }
    }

    fn execute<M: MemoryInterface>(&mut self, instruction: Instruction, memory: &mut M) -> Exec {
        use ops::{arithmetic as ar, bits as bi, data as da, system as sy};
        match instruction {
            Instruction::Data(i) => match i {
                DataInstruction::Move { size, src, dst } => {
                    da::exec_move(self, size, src, dst, memory)
                }
                DataInstruction::MoveA { size, src, dst_reg } => {
                    da::exec_movea(self, size, src, dst_reg, memory)
                }
                DataInstruction::MoveQ { dst_reg, data } => da::exec_moveq(self, dst_reg, data),
                DataInstruction::Lea { src, dst_reg } => da::exec_lea(self, src, dst_reg, memory),
                DataInstruction::Pea { src } => da::exec_pea(self, src, memory),
                DataInstruction::Clr { size, dst } => ar::exec_clr(self, size, dst, memory),
                DataInstruction::Exg { rx, ry, mode } => da::exec_exg(self, rx, ry, mode),
                DataInstruction::Movep {
                    size,
                    reg,
                    an,
                    direction,
                } => da::exec_movep(self, size, reg, an, direction, memory),
                DataInstruction::Movem {
                    size,
                    direction,
                    ea,
                    ..
                } => da::exec_movem(self, size, direction, ea, memory),
                DataInstruction::Swap { reg } => da::exec_swap(self, reg),
                DataInstruction::Ext { size, reg } => da::exec_ext(self, size, reg),
            },
            Instruction::Arithmetic(i) => match i {
                ArithmeticInstruction::Add { size, src, dst, .. } => {
                    ar::exec_add_sub(self, size, src, dst, false, memory)
                }
                ArithmeticInstruction::Sub { size, src, dst, .. } => {
                    ar::exec_add_sub(self, size, src, dst, true, memory)
                }
                ArithmeticInstruction::AddA { size, src, dst_reg } => {
                    ar::exec_adda_suba(self, size, src, dst_reg, false, memory)
                }
                ArithmeticInstruction::SubA { size, src, dst_reg } => {
                    ar::exec_adda_suba(self, size, src, dst_reg, true, memory)
                }
                ArithmeticInstruction::AddI { size, dst } => {
                    ar::exec_addi_subi(self, size, dst, false, memory)
                }
                ArithmeticInstruction::SubI { size, dst } => {
                    ar::exec_addi_subi(self, size, dst, true, memory)
                }
                ArithmeticInstruction::AddQ { size, dst, data } => {
                    ar::exec_addq_subq(self, size, data, dst, false, memory)
                }
                ArithmeticInstruction::SubQ { size, dst, data } => {
                    ar::exec_addq_subq(self, size, data, dst, true, memory)
                }
                ArithmeticInstruction::MulU { src, dst_reg } => {
                    ar::exec_mulu(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::MulS { src, dst_reg } => {
                    ar::exec_muls(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::DivU { src, dst_reg } => {
                    ar::exec_divu(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::DivS { src, dst_reg } => {
                    ar::exec_divs(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::Neg { size, dst } => ar::exec_neg(self, size, dst, memory),
                ArithmeticInstruction::NegX { size, dst } => ar::exec_negx(self, size, dst, memory),
                ArithmeticInstruction::Abcd {
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ar::exec_abcd(self, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::Sbcd {
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ar::exec_sbcd(self, src_reg, dst_reg, memory_mode, memory),
                ArithmeticInstruction::Nbcd { dst } => ar::exec_nbcd(self, dst, memory),
                ArithmeticInstruction::AddX {
                    size,
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ar::exec_addx_subx(self, size, src_reg, dst_reg, memory_mode, false, memory),
                ArithmeticInstruction::SubX {
                    size,
                    src_reg,
                    dst_reg,
                    memory_mode,
                } => ar::exec_addx_subx(self, size, src_reg, dst_reg, memory_mode, true, memory),
                ArithmeticInstruction::Chk { src, dst_reg } => {
                    ar::exec_chk(self, src, dst_reg, memory)
                }
                ArithmeticInstruction::Cmp { size, src, dst_reg } => {
                    ar::exec_cmp(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::CmpA { size, src, dst_reg } => {
                    ar::exec_cmpa(self, size, src, dst_reg, memory)
                }
                ArithmeticInstruction::CmpI { size, dst } => ar::exec_cmpi(self, size, dst, memory),
                ArithmeticInstruction::CmpM { size, ax, ay } => {
                    ar::exec_cmpm(self, size, ax, ay, memory)
                }
                ArithmeticInstruction::Tst { size, dst } => ar::exec_tst(self, size, dst, memory),
            },
            Instruction::Bits(i) => match i {
                BitsInstruction::And { size, src, dst, .. } => {
                    bi::exec_logic(self, size, src, dst, bi::LogicOp::And, memory)
                }
                BitsInstruction::Or { size, src, dst, .. } => {
                    bi::exec_logic(self, size, src, dst, bi::LogicOp::Or, memory)
                }
                BitsInstruction::Eor { size, src_reg, dst } => bi::exec_logic(
                    self,
                    size,
                    instructions::AddressingMode::DataRegister(src_reg),
                    dst,
                    bi::LogicOp::Eor,
                    memory,
                ),
                BitsInstruction::AndI { size, dst } => {
                    bi::exec_logic_imm(self, size, dst, bi::LogicOp::And, memory)
                }
                BitsInstruction::OrI { size, dst } => {
                    bi::exec_logic_imm(self, size, dst, bi::LogicOp::Or, memory)
                }
                BitsInstruction::EorI { size, dst } => {
                    bi::exec_logic_imm(self, size, dst, bi::LogicOp::Eor, memory)
                }
                BitsInstruction::Not { size, dst } => bi::exec_not(self, size, dst, memory),
                BitsInstruction::Lsl { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Lsl, memory)
                }
                BitsInstruction::Lsr { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Lsr, memory)
                }
                BitsInstruction::Asl { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Asl, memory)
                }
                BitsInstruction::Asr { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Asr, memory)
                }
                BitsInstruction::AslM { dst } => bi::exec_shift(
                    self,
                    Size::Word,
                    dst,
                    instructions::ShiftCount::Immediate(1),
                    bi::ShiftOp::Asl,
                    memory,
                ),
                BitsInstruction::AsrM { dst } => bi::exec_shift(
                    self,
                    Size::Word,
                    dst,
                    instructions::ShiftCount::Immediate(1),
                    bi::ShiftOp::Asr,
                    memory,
                ),
                BitsInstruction::Rol { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Rol, memory)
                }
                BitsInstruction::Ror { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Ror, memory)
                }
                BitsInstruction::Roxl { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Roxl, memory)
                }
                BitsInstruction::Roxr { size, dst, count } => {
                    bi::exec_shift(self, size, dst, count, bi::ShiftOp::Roxr, memory)
                }
                BitsInstruction::Btst { bit, dst } => {
                    bi::exec_bit(self, bit, dst, bi::BitOp::Test, memory)
                }
                BitsInstruction::Bset { bit, dst } => {
                    bi::exec_bit(self, bit, dst, bi::BitOp::Set, memory)
                }
                BitsInstruction::Bclr { bit, dst } => {
                    bi::exec_bit(self, bit, dst, bi::BitOp::Clear, memory)
                }
                BitsInstruction::Bchg { bit, dst } => {
                    bi::exec_bit(self, bit, dst, bi::BitOp::Change, memory)
                }
                BitsInstruction::Tas { dst } => bi::exec_tas(self, dst, memory),
            },
            Instruction::System(i) => match i {
                SystemInstruction::Bra { displacement } => {
                    sy::exec_bcc(self, Condition::True, displacement, memory)
                }
                SystemInstruction::Bsr { displacement } => sy::exec_bsr(self, displacement, memory),
                SystemInstruction::Bcc {
                    condition,
                    displacement,
                } => sy::exec_bcc(self, condition, displacement, memory),
                SystemInstruction::Scc { condition, dst } => {
                    sy::exec_scc(self, condition, dst, memory)
                }
                SystemInstruction::DBcc { condition, reg } => {
                    sy::exec_dbcc(self, condition, reg, memory)
                }
                SystemInstruction::Jmp { dst } => sy::exec_jmp(self, dst, memory),
                SystemInstruction::Jsr { dst } => sy::exec_jsr(self, dst, memory),
                SystemInstruction::Rts => sy::exec_rts(self, memory),
                SystemInstruction::Rte => sy::exec_rte(self, memory),
                SystemInstruction::Rtr => sy::exec_rtr(self, memory),
                SystemInstruction::Nop => {
                    self.prefetch();
                    Ok(())
                }
                SystemInstruction::Reset => sy::exec_reset(self, memory),
                SystemInstruction::Stop => sy::exec_stop(self, memory),
                SystemInstruction::MoveUsp { reg, to_usp } => {
                    sy::exec_move_usp(self, reg, to_usp, memory)
                }
                SystemInstruction::Trap { vector } => sy::exec_trap(self, vector, memory),
                SystemInstruction::TrapV => sy::exec_trapv(self, memory),
                SystemInstruction::Link { reg } => sy::exec_link(self, reg, memory),
                SystemInstruction::Unlk { reg } => sy::exec_unlk(self, reg, memory),
                SystemInstruction::MoveToSr { src } => sy::exec_move_to_sr(self, src, memory),
                SystemInstruction::MoveFromSr { dst } => sy::exec_move_from_sr(self, dst, memory),
                SystemInstruction::MoveToCcr { src } => sy::exec_move_to_ccr(self, src, memory),
                SystemInstruction::AndiToCcr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::And, false, memory)
                }
                SystemInstruction::AndiToSr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::And, true, memory)
                }
                SystemInstruction::OriToCcr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::Or, false, memory)
                }
                SystemInstruction::OriToSr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::Or, true, memory)
                }
                SystemInstruction::EoriToCcr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::Eor, false, memory)
                }
                SystemInstruction::EoriToSr => {
                    sy::exec_logic_to_sr(self, bi::LogicOp::Eor, true, memory)
                }
                SystemInstruction::Illegal | SystemInstruction::Unimplemented { .. } => {
                    self.exception_at_opcode(memory, vectors::ILLEGAL_INSTRUCTION)
                }
                SystemInstruction::LineA { .. } => {
                    self.exception_at_opcode(memory, vectors::LINE_A)
                }
                SystemInstruction::LineF { .. } => {
                    self.exception_at_opcode(memory, vectors::LINE_F)
                }
            },
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
mod tests_harte;
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
mod tests_m68k_conformance;
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
#[cfg(test)]
mod tests_state;
#[cfg(test)]
mod tests_system;
