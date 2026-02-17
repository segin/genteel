use super::{flags, Z80};
use crate::memory::{IoInterface, MemoryInterface};

pub trait CbOps {
    fn cb_rotate_shift(&mut self, val: u8, y: u8) -> u8;
    fn cb_bit(&mut self, val: u8, bit: u8);
    fn cb_res(&mut self, val: u8, bit: u8) -> u8;
    fn cb_set(&mut self, val: u8, bit: u8) -> u8;
}

impl<M: MemoryInterface, I: IoInterface> CbOps for Z80<M, I> {
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
}
