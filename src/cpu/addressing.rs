//! M68k Addressing Mode Resolution
//!
//! This module provides utilities for resolving effective addresses
//! and reading/writing operands for all M68k addressing modes.

use super::decoder::{AddressingMode, Size};
use crate::memory::{Memory, MemoryInterface};

/// Result of resolving an effective address
#[derive(Debug, Clone, Copy)]
pub enum EffectiveAddress {
    /// Data register (register number)
    DataRegister(u8),
    /// Address register (register number)
    AddressRegister(u8),
    /// Memory address
    Memory(u32),
}

/// Extension word for indexed addressing modes
#[derive(Debug, Clone, Copy)]
pub struct IndexExtension {
    /// Index register is address register (true) or data register (false)
    pub is_address_reg: bool,
    /// Index register number (0-7)
    pub reg: u8,
    /// Index size: true = long, false = word
    pub is_long: bool,
    /// 8-bit signed displacement
    pub displacement: i8,
}

impl IndexExtension {
    /// Parse from extension word
    pub fn from_word(word: u16) -> Self {
        Self {
            is_address_reg: (word >> 15) & 1 != 0,
            reg: ((word >> 12) & 0x07) as u8,
            is_long: (word >> 11) & 1 != 0,
            displacement: (word & 0xFF) as i8,
        }
    }
}

/// Operand value with size information
#[derive(Debug, Clone, Copy)]
pub struct Operand {
    pub value: u32,
    pub size: Size,
}

impl Operand {
    /// Create a new operand
    pub fn new(value: u32, size: Size) -> Self {
        Self { value, size }
    }

    /// Get the value masked to the appropriate size
    pub fn masked_value(&self) -> u32 {
        match self.size {
            Size::Byte => self.value & 0xFF,
            Size::Word => self.value & 0xFFFF,
            Size::Long => self.value,
        }
    }

    /// Sign-extend the value to 32 bits
    pub fn sign_extended(&self) -> i32 {
        match self.size {
            Size::Byte => (self.value as i8) as i32,
            Size::Word => (self.value as i16) as i32,
            Size::Long => self.value as i32,
        }
    }
}

/// Calculate effective address for an addressing mode
///
/// Returns the effective address and updates PC if extension words are read.
/// Also returns the number of cycles used.
pub fn calculate_ea<M: MemoryInterface + ?Sized>(
    mode: AddressingMode,
    size: Size,
    d: &[u32; 8],
    a: &[u32; 8],
    pc: &mut u32,
    memory: &mut M,
) -> (EffectiveAddress, u32) {
    match mode {
        AddressingMode::DataRegister(reg) => {
            (EffectiveAddress::DataRegister(reg), 0)
        }
        AddressingMode::AddressRegister(reg) => {
            (EffectiveAddress::AddressRegister(reg), 0)
        }
        AddressingMode::AddressIndirect(reg) => {
            (EffectiveAddress::Memory(a[reg as usize]), 4)
        }
        AddressingMode::AddressPostIncrement(reg) => {
            // Post-increment: address is used, then incremented
            let addr = a[reg as usize];
            // Note: caller must handle the increment after use
            (EffectiveAddress::Memory(addr), 4)
        }
        AddressingMode::AddressPreDecrement(reg) => {
            // Pre-decrement: address is decremented, then used
            // Note: caller must handle the decrement before use
            let decrement = match size {
                Size::Byte => {
                    // Special case: A7 always stays word-aligned
                    if reg == 7 { 2 } else { 1 }
                }
                Size::Word => 2,
                Size::Long => 4,
            };
            let addr = a[reg as usize].wrapping_sub(decrement);
            (EffectiveAddress::Memory(addr), 6)
        }
        AddressingMode::AddressDisplacement(reg) => {
            let displacement = memory.read_word(*pc) as i16;
            *pc = pc.wrapping_add(2);
            let addr = (a[reg as usize] as i32).wrapping_add(displacement as i32) as u32;
            (EffectiveAddress::Memory(addr), 8)
        }
        AddressingMode::AddressIndex(reg) => {
            let ext_word = memory.read_word(*pc);
            *pc = pc.wrapping_add(2);
            let ext = IndexExtension::from_word(ext_word);

            let base = a[reg as usize];
            let index = if ext.is_address_reg {
                a[ext.reg as usize]
            } else {
                d[ext.reg as usize]
            };
            let index = if ext.is_long {
                index as i32
            } else {
                (index as i16) as i32
            };

            let addr = (base as i32)
                .wrapping_add(index)
                .wrapping_add(ext.displacement as i32) as u32;
            (EffectiveAddress::Memory(addr), 10)
        }
        AddressingMode::AbsoluteShort => {
            let addr = memory.read_word(*pc) as i16 as i32 as u32;
            *pc = pc.wrapping_add(2);
            (EffectiveAddress::Memory(addr), 8)
        }
        AddressingMode::AbsoluteLong => {
            let addr = memory.read_long(*pc);
            *pc = pc.wrapping_add(4);
            (EffectiveAddress::Memory(addr), 12)
        }
        AddressingMode::PcDisplacement => {
            let base_pc = *pc;
            let displacement = memory.read_word(*pc) as i16;
            *pc = pc.wrapping_add(2);
            let addr = (base_pc as i32).wrapping_add(displacement as i32) as u32;
            (EffectiveAddress::Memory(addr), 8)
        }
        AddressingMode::PcIndex => {
            let base_pc = *pc;
            let ext_word = memory.read_word(*pc);
            *pc = pc.wrapping_add(2);
            let ext = IndexExtension::from_word(ext_word);

            let index = if ext.is_address_reg {
                a[ext.reg as usize]
            } else {
                d[ext.reg as usize]
            };
            let index = if ext.is_long {
                index as i32
            } else {
                (index as i16) as i32
            };

            let addr = (base_pc as i32)
                .wrapping_add(index)
                .wrapping_add(ext.displacement as i32) as u32;
            (EffectiveAddress::Memory(addr), 10)
        }
        AddressingMode::Immediate => {
            let value_addr = *pc;
            let ext_words = match size {
                Size::Byte | Size::Word => {
                    *pc = pc.wrapping_add(2);
                    1
                }
                Size::Long => {
                    *pc = pc.wrapping_add(4);
                    2
                }
            };
            (EffectiveAddress::Memory(value_addr), 4 * ext_words)
        }
    }
}

/// Read a value from an effective address
pub fn read_ea<M: MemoryInterface + ?Sized>(
    ea: EffectiveAddress,
    size: Size,
    d: &[u32; 8],
    a: &[u32; 8],
    memory: &mut M,
) -> u32 {
    match ea {
        EffectiveAddress::DataRegister(reg) => {
            let val = d[reg as usize];
            match size {
                Size::Byte => val & 0xFF,
                Size::Word => val & 0xFFFF,
                Size::Long => val,
            }
        }
        EffectiveAddress::AddressRegister(reg) => {
            let val = a[reg as usize];
            match size {
                Size::Byte => val & 0xFF,
                Size::Word => val & 0xFFFF,
                Size::Long => val,
            }
        }
        EffectiveAddress::Memory(addr) => match size {
            Size::Byte => memory.read_byte(addr) as u32,
            Size::Word => memory.read_word(addr) as u32,
            Size::Long => memory.read_long(addr),
        },
    }
}

/// Write a value to an effective address
pub fn write_ea<M: MemoryInterface + ?Sized>(
    ea: EffectiveAddress,
    size: Size,
    value: u32,
    d: &mut [u32; 8],
    a: &mut [u32; 8],
    memory: &mut M,
) {
    match ea {
        EffectiveAddress::DataRegister(reg) => {
            let reg = reg as usize;
            match size {
                Size::Byte => d[reg] = (d[reg] & 0xFFFFFF00) | (value & 0xFF),
                Size::Word => d[reg] = (d[reg] & 0xFFFF0000) | (value & 0xFFFF),
                Size::Long => d[reg] = value,
            }
        }
        EffectiveAddress::AddressRegister(reg) => {
            let reg = reg as usize;
            // Address register writes are always 32-bit (sign-extended for word)
            match size {
                Size::Byte => a[reg] = (value as i8) as i32 as u32,
                Size::Word => a[reg] = (value as i16) as i32 as u32,
                Size::Long => a[reg] = value,
            }
        }
        EffectiveAddress::Memory(addr) => match size {
            Size::Byte => memory.write_byte(addr, value as u8),
            Size::Word => memory.write_word(addr, value as u16),
            Size::Long => memory.write_long(addr, value),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operand_masked_value() {
        let op = Operand::new(0x12345678, Size::Byte);
        assert_eq!(op.masked_value(), 0x78);

        let op = Operand::new(0x12345678, Size::Word);
        assert_eq!(op.masked_value(), 0x5678);

        let op = Operand::new(0x12345678, Size::Long);
        assert_eq!(op.masked_value(), 0x12345678);
    }

    #[test]
    fn test_operand_sign_extended() {
        let op = Operand::new(0x80, Size::Byte);
        assert_eq!(op.sign_extended(), -128);

        let op = Operand::new(0x8000, Size::Word);
        assert_eq!(op.sign_extended(), -32768);

        let op = Operand::new(0x80000000, Size::Long);
        assert_eq!(op.sign_extended(), i32::MIN);
    }

    #[test]
    fn test_index_extension_parse() {
        // D0.W, displacement = 10
        let ext = IndexExtension::from_word(0x000A);
        assert!(!ext.is_address_reg);
        assert_eq!(ext.reg, 0);
        assert!(!ext.is_long);
        assert_eq!(ext.displacement, 10);

        // A3.L, displacement = -5
        let ext = IndexExtension::from_word(0xB8FB);
        assert!(ext.is_address_reg);
        assert_eq!(ext.reg, 3);
        assert!(ext.is_long);
        assert_eq!(ext.displacement, -5);
    }
}
