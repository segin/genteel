use crate::memory::MemoryInterface;
use serde::{Deserialize, Serialize};

pub mod addressing;
pub mod decoder;
pub mod instructions;
pub mod ops;

pub use decoder::{Condition, Cpu, Size};

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
            Condition::GreaterEqual => (n && v) || (!n && !v),
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
