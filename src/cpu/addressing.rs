//! M68k effective-address resolution.
//!
//! Address calculation consumes extension words (each costing one program
//! fetch), spends the internal cycles the hardware spends (2 clocks for an
//! index or a predecrement) and applies postincrement/predecrement to the
//! address register. It never faults: only the operand access itself can
//! raise an address error.

use super::decoder::{AddressingMode, Size};
use super::{Cpu, Exec};
use crate::memory::MemoryInterface;

/// A resolved effective address.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EffectiveAddress {
    /// Data register (register number)
    DataRegister(u8),
    /// Address register (register number)
    AddressRegister(u8),
    /// Memory address
    Memory(u32),
    /// Immediate operand value, already fetched from the instruction stream
    Immediate(u32),
}

/// Brief extension word for indexed addressing modes
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

/// Postincrement/predecrement step: A7 always moves by at least 2 to keep
/// the stack word aligned.
#[inline]
pub fn address_step(reg: u8, size: Size) -> u32 {
    match size {
        Size::Byte if reg == 7 => 2,
        _ => size.bytes(),
    }
}

impl Cpu {
    /// Value of a brief-extension index (sign-extended when word sized)
    /// plus its 8-bit displacement.
    #[inline]
    pub(crate) fn index_offset(&self, ext_word: u16) -> u32 {
        let ext = IndexExtension::from_word(ext_word);
        let raw = if ext.is_address_reg {
            self.a[ext.reg as usize]
        } else {
            self.d[ext.reg as usize]
        };
        let index = if ext.is_long {
            raw
        } else {
            raw as u16 as i16 as i32 as u32
        };
        index.wrapping_add(ext.displacement as i32 as u32)
    }

    /// Resolve an effective address (standard timing).
    pub(crate) fn calc_ea<M: MemoryInterface>(
        &mut self,
        mode: AddressingMode,
        size: Size,
        memory: &mut M,
    ) -> EffectiveAddress {
        self.calc_ea_with(mode, size, memory, true)
    }

    /// Resolve an effective address; `predec_delay` selects whether a
    /// predecrement spends its 2 internal clocks (MOVE and MOVEM destinations
    /// do not).
    pub(crate) fn calc_ea_with<M: MemoryInterface>(
        &mut self,
        mode: AddressingMode,
        size: Size,
        memory: &mut M,
        predec_delay: bool,
    ) -> EffectiveAddress {
        match mode {
            AddressingMode::DataRegister(r) => EffectiveAddress::DataRegister(r),
            AddressingMode::AddressRegister(r) => EffectiveAddress::AddressRegister(r),
            AddressingMode::AddressIndirect(r) => EffectiveAddress::Memory(self.a[r as usize]),
            AddressingMode::AddressPostIncrement(r) => {
                let addr = self.a[r as usize];
                self.a[r as usize] = addr.wrapping_add(address_step(r, size));
                EffectiveAddress::Memory(addr)
            }
            AddressingMode::AddressPreDecrement(r) => {
                if predec_delay {
                    self.idle(2);
                }
                let addr = self.a[r as usize].wrapping_sub(address_step(r, size));
                self.a[r as usize] = addr;
                EffectiveAddress::Memory(addr)
            }
            AddressingMode::AddressDisplacement(r) => {
                let disp = self.ext_word(memory) as i16 as i32 as u32;
                EffectiveAddress::Memory(self.a[r as usize].wrapping_add(disp))
            }
            AddressingMode::AddressIndex(r) => {
                self.idle(2);
                let ext = self.ext_word(memory);
                let base = self.a[r as usize];
                EffectiveAddress::Memory(base.wrapping_add(self.index_offset(ext)))
            }
            AddressingMode::AbsoluteShort => {
                EffectiveAddress::Memory(self.ext_word(memory) as i16 as i32 as u32)
            }
            AddressingMode::AbsoluteLong => EffectiveAddress::Memory(self.ext_long(memory)),
            AddressingMode::PcDisplacement => {
                let base = self.pc;
                let disp = self.ext_word(memory) as i16 as i32 as u32;
                EffectiveAddress::Memory(base.wrapping_add(disp))
            }
            AddressingMode::PcIndex => {
                self.idle(2);
                let base = self.pc;
                let ext = self.ext_word(memory);
                EffectiveAddress::Memory(base.wrapping_add(self.index_offset(ext)))
            }
            AddressingMode::Immediate => {
                EffectiveAddress::Immediate(self.fetch_immediate(size, memory))
            }
        }
    }

    /// Fetch an immediate operand of `size` from the instruction stream.
    pub(crate) fn fetch_immediate<M: MemoryInterface>(
        &mut self,
        size: Size,
        memory: &mut M,
    ) -> u32 {
        match size {
            Size::Byte => (self.ext_word(memory) & 0xFF) as u32,
            Size::Word => self.ext_word(memory) as u32,
            Size::Long => self.ext_long(memory),
        }
    }

    /// Resolve a control-mode effective address without consuming the
    /// prefetch refills (JMP/JSR/LEA-style timing is handled by callers).
    /// Returns the address and the number of extension words consumed.
    pub(crate) fn control_address<M: MemoryInterface>(
        &mut self,
        mode: AddressingMode,
        memory: &mut M,
    ) -> u32 {
        match mode {
            AddressingMode::AddressIndirect(r) => self.a[r as usize],
            AddressingMode::AddressDisplacement(r) => {
                let disp = self.ext_word_deferred(memory) as i16 as i32 as u32;
                self.a[r as usize].wrapping_add(disp)
            }
            AddressingMode::AddressIndex(r) => {
                let ext = self.ext_word_deferred(memory);
                self.a[r as usize].wrapping_add(self.index_offset(ext))
            }
            AddressingMode::AbsoluteShort => self.ext_word_deferred(memory) as i16 as i32 as u32,
            AddressingMode::AbsoluteLong => {
                let hi = self.ext_word(memory) as u32;
                let lo = self.ext_word_deferred(memory) as u32;
                (hi << 16) | lo
            }
            AddressingMode::PcDisplacement => {
                let base = self.pc;
                let disp = self.ext_word_deferred(memory) as i16 as i32 as u32;
                base.wrapping_add(disp)
            }
            AddressingMode::PcIndex => {
                let base = self.pc;
                let ext = self.ext_word_deferred(memory);
                base.wrapping_add(self.index_offset(ext))
            }
            /* Not a control mode; the decoder rejects these encodings. */
            _ => 0,
        }
    }

    /// Read an operand from a resolved effective address.
    #[inline]
    pub(crate) fn read_operand<M: MemoryInterface>(
        &mut self,
        ea: EffectiveAddress,
        size: Size,
        memory: &mut M,
    ) -> Exec<u32> {
        match ea {
            EffectiveAddress::DataRegister(r) => Ok(self.d[r as usize] & size.mask()),
            EffectiveAddress::AddressRegister(r) => Ok(self.a[r as usize] & size.mask()),
            EffectiveAddress::Memory(addr) => self.read(memory, addr, size),
            EffectiveAddress::Immediate(v) => Ok(v & size.mask()),
        }
    }

    /// Write an operand to a resolved effective address.
    #[inline]
    pub(crate) fn write_operand<M: MemoryInterface>(
        &mut self,
        ea: EffectiveAddress,
        size: Size,
        value: u32,
        memory: &mut M,
    ) -> Exec {
        match ea {
            EffectiveAddress::DataRegister(r) => {
                let r = r as usize;
                self.d[r] = size.apply(self.d[r], value);
                Ok(())
            }
            EffectiveAddress::AddressRegister(r) => {
                /* Address registers are always written as a whole long. */
                self.a[r as usize] = match size {
                    Size::Byte => value as u8 as i8 as i32 as u32,
                    Size::Word => value as u16 as i16 as i32 as u32,
                    Size::Long => value,
                };
                Ok(())
            }
            EffectiveAddress::Memory(addr) => self.write(memory, addr, size, value),
            /* Immediate is never a destination; the decoder rejects it. */
            EffectiveAddress::Immediate(_) => Ok(()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_index_extension_parse() {
        let ext = IndexExtension::from_word(0x000A);
        assert!(!ext.is_address_reg);
        assert_eq!(ext.reg, 0);
        assert!(!ext.is_long);
        assert_eq!(ext.displacement, 10);

        let ext = IndexExtension::from_word(0xB8FB);
        assert!(ext.is_address_reg);
        assert_eq!(ext.reg, 3);
        assert!(ext.is_long);
        assert_eq!(ext.displacement, -5);
    }

    #[test]
    fn test_address_step_keeps_stack_aligned() {
        assert_eq!(address_step(7, Size::Byte), 2);
        assert_eq!(address_step(6, Size::Byte), 1);
        assert_eq!(address_step(7, Size::Word), 2);
        assert_eq!(address_step(0, Size::Long), 4);
    }
}
