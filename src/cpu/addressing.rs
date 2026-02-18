//! M68k Addressing Mode Resolution
//!
//! This module provides utilities for resolving effective addresses
//! and reading/writing operands for all M68k addressing modes.

use super::decoder::{AddressingMode, Size};
use crate::memory::MemoryInterface;

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
pub fn calculate_ea<M: MemoryInterface>(
    mode: AddressingMode,
    size: Size,
    d: &mut [u32; 8],
    a: &mut [u32; 8],
    pc: &mut u32,
    memory: &mut M,
) -> (EffectiveAddress, u32) {
    match mode {
        AddressingMode::DataRegister(reg) => (EffectiveAddress::DataRegister(reg), 0),
        AddressingMode::AddressRegister(reg) => (EffectiveAddress::AddressRegister(reg), 0),
        AddressingMode::AddressIndirect(reg) => (EffectiveAddress::Memory(a[reg as usize]), 4),
        AddressingMode::AddressPostIncrement(reg) => {
            let addr = a[reg as usize];
            let increment = match size {
                Size::Byte => {
                    if reg == 7 {
                        2
                    } else {
                        1
                    }
                }
                Size::Word => 2,
                Size::Long => 4,
            };
            a[reg as usize] = addr.wrapping_add(increment);
            (EffectiveAddress::Memory(addr), 4)
        }
        AddressingMode::AddressPreDecrement(reg) => {
            let decrement = match size {
                Size::Byte => {
                    if reg == 7 {
                        2
                    } else {
                        1
                    }
                }
                Size::Word => 2,
                Size::Long => 4,
            };
            let addr = a[reg as usize].wrapping_sub(decrement);
            a[reg as usize] = addr;
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
            let mut value_addr = *pc;
            let ext_words = match size {
                Size::Byte | Size::Word => {
                    if size == Size::Byte {
                        value_addr = value_addr.wrapping_add(1);
                    }
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
pub fn read_ea<M: MemoryInterface>(
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
pub fn write_ea<M: MemoryInterface>(
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

    #[derive(Debug, Default)]
    struct MockMemory {
        pub data: std::collections::HashMap<u32, u8>,
        pub reads: std::cell::RefCell<Vec<(u32, Size)>>,
    }

    impl MockMemory {
        fn new() -> Self {
            Self::default()
        }

        fn set_word(&mut self, addr: u32, val: u16) {
            self.data.insert(addr, (val >> 8) as u8);
            self.data.insert(addr + 1, (val & 0xFF) as u8);
        }

        fn set_long(&mut self, addr: u32, val: u32) {
            self.set_word(addr, (val >> 16) as u16);
            self.set_word(addr + 2, (val & 0xFFFF) as u16);
        }

        fn get_reads(&self) -> Vec<(u32, Size)> {
            self.reads.borrow().clone()
        }
    }

    impl MemoryInterface for MockMemory {
        fn read_byte(&mut self, address: u32) -> u8 {
            self.reads.borrow_mut().push((address, Size::Byte));
            *self.data.get(&address).unwrap_or(&0)
        }

        fn write_byte(&mut self, _address: u32, _value: u8) {}

        fn read_word(&mut self, address: u32) -> u16 {
            self.reads.borrow_mut().push((address, Size::Word));
            let high = *self.data.get(&address).unwrap_or(&0) as u16;
            let low = *self.data.get(&(address + 1)).unwrap_or(&0) as u16;
            (high << 8) | low
        }

        fn write_word(&mut self, _address: u32, _value: u16) {}

        fn read_long(&mut self, address: u32) -> u32 {
            self.reads.borrow_mut().push((address, Size::Long));
            let b0 = *self.data.get(&address).unwrap_or(&0) as u32;
            let b1 = *self.data.get(&(address + 1)).unwrap_or(&0) as u32;
            let b2 = *self.data.get(&(address + 2)).unwrap_or(&0) as u32;
            let b3 = *self.data.get(&(address + 3)).unwrap_or(&0) as u32;
            (b0 << 24) | (b1 << 16) | (b2 << 8) | b3
        }

        fn write_long(&mut self, _address: u32, _value: u32) {}
    }

    #[test]
    fn test_calculate_ea_register_direct() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        let mut pc = 0x1000;

        // Data Register
        let (ea, cycles) = calculate_ea(
            AddressingMode::DataRegister(3),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        assert!(matches!(ea, EffectiveAddress::DataRegister(3)));
        assert_eq!(cycles, 0);
        assert_eq!(mock.get_reads().len(), 0);

        // Address Register
        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressRegister(5),
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        assert!(matches!(ea, EffectiveAddress::AddressRegister(5)));
        assert_eq!(cycles, 0);
        assert_eq!(mock.get_reads().len(), 0);
    }

    #[test]
    fn test_calculate_ea_indirect() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        a[2] = 0x2000;
        let mut pc = 0x1000;

        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressIndirect(2),
            Size::Long,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );

        match ea {
            EffectiveAddress::Memory(addr) => assert_eq!(addr, 0x2000),
            _ => panic!("Expected Memory EA"),
        }
        assert_eq!(cycles, 4);
        assert_eq!(mock.get_reads().len(), 0);
    }

    #[test]
    fn test_calculate_ea_post_increment() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        a[0] = 0x1000;
        let mut pc = 0x100;

        // Byte size (inc by 1)
        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressPostIncrement(0),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1000);
        } else {
            panic!("Wrong EA");
        }
        assert_eq!(a[0], 0x1001);
        assert_eq!(cycles, 4);

        // Stack Pointer (A7) Byte size special case (inc by 2)
        a[7] = 0x2000;
        let (ea, _) = calculate_ea(
            AddressingMode::AddressPostIncrement(7),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x2000);
        }
        assert_eq!(a[7], 0x2002);
    }

    #[test]
    fn test_calculate_ea_pre_decrement() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        a[0] = 0x1000;
        let mut pc = 0x100;

        // Byte size (dec by 1)
        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressPreDecrement(0),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x0FFF);
        } else {
            panic!("Wrong EA");
        }
        assert_eq!(a[0], 0x0FFF);
        assert_eq!(cycles, 6);

        // Stack Pointer (A7) Byte size special case (dec by 2)
        a[7] = 0x2000;
        let (ea, _) = calculate_ea(
            AddressingMode::AddressPreDecrement(7),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1FFE);
        }
        assert_eq!(a[7], 0x1FFE);
    }

    #[test]
    fn test_calculate_ea_displacement() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        a[0] = 0x1000;
        let mut pc = 0x200;

        // Extension word at PC 0x200: 0x0010 (+16)
        mock.set_word(0x200, 0x0010);

        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressDisplacement(0),
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );

        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1010);
        } else {
            panic!("Wrong EA");
        }

        assert_eq!(pc, 0x202);
        assert_eq!(cycles, 8);

        // Verify read
        let reads = mock.get_reads();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0], (0x200, Size::Word));
    }

    #[test]
    fn test_calculate_ea_index() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        a[0] = 0x1000;
        d[1] = 0x20; // Index
        let mut pc = 0x200;

        // Extension word at PC 0x200: D1.L, Disp=4
        // 0 (Dn) 001 (Reg=1) 1 (Long) 000 (Scale) 00000100 (Disp=4) -> 0x1804
        mock.set_word(0x200, 0x1804);

        let (ea, cycles) = calculate_ea(
            AddressingMode::AddressIndex(0),
            Size::Byte,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );

        // Addr = Base(0x1000) + Index(0x20) + Disp(4) = 0x1024
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x1024);
        } else {
            panic!("Wrong EA");
        }

        assert_eq!(pc, 0x202);
        assert_eq!(cycles, 10);
        // Verify read
        let reads = mock.get_reads();
        assert_eq!(reads.len(), 1);
        assert_eq!(reads[0], (0x200, Size::Word));
    }

    #[test]
    fn test_calculate_ea_absolute() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        let mut pc = 0x200;

        // Short: 0x8000 -> Sign extended 0xFFFF8000
        mock.set_word(0x200, 0x8000);
        let (ea, cycles) = calculate_ea(
            AddressingMode::AbsoluteShort,
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0xFFFF8000);
        }
        assert_eq!(pc, 0x202);
        assert_eq!(cycles, 8);

        // Long: 0x00FF0000
        mock.set_long(0x202, 0x00FF0000);
        let (ea, cycles) = calculate_ea(
            AddressingMode::AbsoluteLong,
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x00FF0000);
        }
        assert_eq!(pc, 0x206);
        assert_eq!(cycles, 12);
    }

    #[test]
    fn test_calculate_ea_pc_relative() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        let mut pc = 0x200;

        // PC Displacement: d16(PC)
        // At 0x200, read 0x0010. Base PC is 0x200. Target = 0x210.
        mock.set_word(0x200, 0x0010);

        let (ea, cycles) = calculate_ea(
            AddressingMode::PcDisplacement,
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x210);
        }
        assert_eq!(pc, 0x202);
        assert_eq!(cycles, 8);
    }

    #[test]
    fn test_calculate_ea_immediate() {
        let mut mock = MockMemory::new();
        let mut d = [0u32; 8];
        let mut a = [0u32; 8];
        let mut pc = 0x200;

        // Immediate Byte/Word (advances by 2 bytes)
        // Value at 0x200
        mock.set_word(0x200, 0x1234);

        let (ea, cycles) = calculate_ea(
            AddressingMode::Immediate,
            Size::Word,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        // Returns address where value is stored
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x200);
        }
        assert_eq!(pc, 0x202);
        assert_eq!(cycles, 4);

        // Immediate Long (advances by 4 bytes)
        let (ea, cycles) = calculate_ea(
            AddressingMode::Immediate,
            Size::Long,
            &mut d,
            &mut a,
            &mut pc,
            &mut mock,
        );
        if let EffectiveAddress::Memory(addr) = ea {
            assert_eq!(addr, 0x202);
        }
        assert_eq!(pc, 0x206);
        assert_eq!(cycles, 8);
    }
}
