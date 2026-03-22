//! Z80 Bus Adapter for Genesis Sound System
//!
//! Routes Z80 memory accesses to the appropriate Genesis components:
//! - 0000h-1FFFh: Z80 Sound RAM (8KB)
//! - 4000h-4003h: YM2612 FM Chip
//! - 6000h: Bank Register (sets 32KB window into 68k memory)
//! - 7F11h: SN76489 PSG
//! - 8000h-FFFFh: Banked 68k Memory (32KB window)

use super::bus::Bus;
use super::{byte_utils, IoInterface, MemoryInterface, SharedBus};
use serde::{Deserialize, Serialize};

/// Z80 Bus adapter that routes memory accesses to Genesis components
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Z80Bus {
    /// Reference to the main Genesis bus.
    bus: SharedBus,
}

impl Z80Bus {
    /// Create a new Z80 bus adapter
    pub fn new(bus: SharedBus) -> Self {
        Self { bus }
    }
}

impl Default for Z80Bus {
    fn default() -> Self {
        Self {
            bus: SharedBus::default(),
        }
    }
}

impl Z80Bus {
    /// The value written becomes the upper bits of the 68k address
    pub fn set_bank(&mut self, value: u8) {
        self.bus.bus.borrow_mut().write_byte(0xA06000, value);
    }

    /// Reset bank register to 0
    pub fn reset_bank(&mut self) {
        let mut bus = self.bus.bus.borrow_mut();
        bus.z80_bank_addr = 0;
    }

    /// Internal helper to read byte from Bus (deduplicated logic)
    fn read_byte_from_bus(bus: &mut Bus, address: u32) -> u8 {
        let addr = address as u16;
        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => bus.z80_ram[addr as usize],

            // Mirror of Z80 RAM: 2000h-3FFFh
            0x2000..=0x3FFF => bus.z80_ram[(addr & 0x1FFF) as usize],

            // YM2612: 4000h-4003h
            0x4000..=0x4003 => bus.apu.fm.read((addr & 3) as u8),

            // FM Mirror or PSG/Bank area
            0x4004..=0x5FFF => 0xFF,

            // Bank register area: 6000h (write-only)
            0x6000..=0x7FFF => 0xFF,

            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let bank_addr = bus.z80_bank_addr;
                let effective_addr = bank_addr | ((addr as u32) & 0x7FFF);
                bus.read_byte(effective_addr)
            }
        }
    }

    /// Internal helper to write byte to Bus (deduplicated logic)
    fn write_byte_to_bus(bus: &mut Bus, address: u32, value: u8) {
        let addr = address as u16;
        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => {
                bus.z80_ram[addr as usize] = value;
            }

            // Mirror of Z80 RAM: 2000h-3FFFh
            0x2000..=0x3FFF => {
                bus.z80_ram[(addr & 0x1FFF) as usize] = value;
            }

            // YM2612: 4000h-4003h
            0x4000..=0x4003 => {
                let port = (addr & 2) >> 1;
                let is_data = (addr & 1) != 0;
                if is_data {
                    bus.apu.fm.write_data(port as u8, value);
                } else {
                    bus.apu.fm.write_address(port as u8, value);
                }
            }

            // Mirror of FM chip or Reserved: 4004h-5FFFh
            0x4004..=0x5FFF => {}

            // Bank register: 6000h
            0x6000..=0x60FF => {
                // Delegate to bus logic for bank register update
                bus.write_byte(0xA06000, value);
            }

            // Reserved / PSG area
            0x6100..=0x7F10 => {}
            0x7F11 => {
                bus.apu.psg.write(value);
            }
            0x7F12..=0x7FFF => {}

            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let bank_addr = bus.z80_bank_addr;
                let effective_addr = bank_addr | ((addr as u32) & 0x7FFF);
                bus.write_byte(effective_addr, value);
            }
        }
    }
}

impl MemoryInterface for Z80Bus {
    fn read_byte(&mut self, address: u32) -> u8 {
        Self::read_byte_from_bus(&mut self.bus.bus.borrow_mut(), address)
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        Self::write_byte_to_bus(&mut self.bus.bus.borrow_mut(), address, value)
    }

    fn read_word(&mut self, address: u32) -> u16 {
        let hi = self.read_byte(address);
        let lo = self.read_byte(address.wrapping_add(1));
        byte_utils::join_u16(hi, lo)
    }

    fn write_word(&mut self, address: u32, value: u16) {
        let (high, low) = byte_utils::split_u16(value);
        self.write_byte(address, high);
        self.write_byte(address.wrapping_add(1), low);
    }

    fn read_long(&mut self, address: u32) -> u32 {
        let hi = self.read_word(address);
        let lo = self.read_word(address.wrapping_add(2));
        byte_utils::join_u32_words(hi, lo)
    }

    fn write_long(&mut self, address: u32, value: u32) {
        let (high, low) = byte_utils::split_u32_to_words(value);
        self.write_word(address, high);
        self.write_word(address.wrapping_add(2), low);
    }

    fn read_size(&mut self, address: u32, size: crate::cpu::decoder::Size) -> u32 {
        match size {
            crate::cpu::decoder::Size::Byte => self.read_byte(address) as u32,
            crate::cpu::decoder::Size::Word => self.read_word(address) as u32,
            crate::cpu::decoder::Size::Long => self.read_long(address),
        }
    }

    fn write_size(&mut self, address: u32, value: u32, size: crate::cpu::decoder::Size) {
        match size {
            crate::cpu::decoder::Size::Byte => self.write_byte(address, value as u8),
            crate::cpu::decoder::Size::Word => self.write_word(address, value as u16),
            crate::cpu::decoder::Size::Long => self.write_long(address, value),
        }
    }
}

impl IoInterface for Z80Bus {
    fn read_port(&mut self, _port: u16) -> u8 {
        // On a real Sega Genesis, the Z80 I/O space is not connected to any internal hardware.
        // Any IN instruction will result in 0xFF (due to bus pull-ups).
        0xFF
    }

    fn write_port(&mut self, _port: u16, _value: u8) {
        // On a real Sega Genesis, the Z80 I/O space is not connected to any internal hardware.
        // Any OUT instruction is effectively a no-op.
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::memory::bus::Bus;
    use std::cell::RefCell;
    use std::rc::Rc;

    fn create_test_z80_bus() -> Z80Bus {
        let bus = Rc::new(RefCell::new(Bus::new()));
        Z80Bus::new(SharedBus::new(bus))
    }

    #[test]
    fn test_z80_ram_read_write() {
        let mut z80_bus = create_test_z80_bus();

        z80_bus.write_byte(0x0000, 0x42);
        assert_eq!(z80_bus.read_byte(0x0000), 0x42);

        z80_bus.write_byte(0x1FFF, 0xAB);
        assert_eq!(z80_bus.read_byte(0x1FFF), 0xAB);
    }

    #[test]
    fn test_bank_register() {
        let mut z80_bus = create_test_z80_bus();

        // Initially bank is 0
        assert_eq!(z80_bus.bus.bus.borrow().z80_bank_addr, 0);

        // Write to bank register (bit-by-bit shifting)
        z80_bus.write_byte(0x6000, 0x01); // Shift in 1

        // Note: bank register implementation in Bus handles the bit shifting logic
        // We just verify it changed
        assert_ne!(z80_bus.bus.bus.borrow().z80_bank_addr, 0);
    }

    #[test]
    fn test_reserved_reads_ff() {
        let mut z80_bus = create_test_z80_bus();

        // Z80 RAM is mirrored at 0x2000-0x3FFF, so reading 0x2000 reads 0x0000 (initially 0)
        assert_eq!(z80_bus.read_byte(0x2000), 0x00);
        assert_eq!(z80_bus.read_byte(0x3FFF), 0x00);

        // Reserved areas (like PSG read) should return 0xFF
        assert_eq!(z80_bus.read_byte(0x4004), 0xFF); // FM Mirror
        assert_eq!(z80_bus.read_byte(0x6000), 0xFF); // Bank register is write-only
        assert_eq!(z80_bus.read_byte(0x7F11), 0xFF); // PSG is write-only
    }

    #[test]
    fn test_z80_io_ports() {
        let mut z80_bus = create_test_z80_bus();

        // All I/O port reads should return 0xFF on Genesis
        assert_eq!(z80_bus.read_port(0x0000), 0xFF);
        assert_eq!(z80_bus.read_port(0x007F), 0xFF);
        assert_eq!(z80_bus.read_port(0xFFFF), 0xFF);

        // Writes should not panic
        z80_bus.write_port(0x0000, 0x42);
        z80_bus.write_port(0xFFFF, 0xAB);
    }

    #[test]
    fn test_set_reset_bank() {
        let mut z80_bus = create_test_z80_bus();

        // Initial state
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 0);
        }

        // set_bank(1) -> inserts 1 at bit 8 of the 9-bit bank register (bit 23 of address)
        z80_bus.set_bank(1);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 1 << 23);
        }

        // set_bank(0) -> right shifts, inserts 0 at bit 8 (bit 23 of addr), 1 is now at bit 7 (bit 22 of addr)
        z80_bus.set_bank(0);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 1 << 22);
        }

        // set_bank(1) -> right shifts, inserts 1 at bit 8 (bit 23 of addr)
        z80_bus.set_bank(1);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, (1 << 23) | (1 << 21));
        }

        // Reset
        z80_bus.reset_bank();
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 0);
        }
    }

    #[test]
    fn test_set_bank_wraparound() {
        let mut z80_bus = create_test_z80_bus();

        // 9 successive writes to fill the 9-bit bank register
        for _ in 0..9 {
            z80_bus.set_bank(1);
        }

        {
            let bus = z80_bus.bus.bus.borrow();
            // All 9 bits (15..=23) should be set
            let expected_addr = 0x1FF << 15;
            assert_eq!(bus.z80_bank_addr, expected_addr);
        }

        // 10th write, right shifts and inserts 0 at bit 23
        z80_bus.set_bank(0);

        {
            let bus = z80_bus.bus.bus.borrow();
            // Bit 23 should be cleared, bits 15..=22 should remain set
            // The bit that was at bit 15 is shifted out
            let expected_addr = 0xFF << 15; // 0x0FF
            assert_eq!(bus.z80_bank_addr, expected_addr);
        }
    }

    #[test]
    fn test_set_bank_ignore_upper_bits() {
        let mut z80_bus = create_test_z80_bus();

        // Writing 0xFF should only set the LSB to 1
        z80_bus.set_bank(0xFF);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 1 << 23);
        }

        // Writing 0xFE should only set the LSB to 0
        z80_bus.set_bank(0xFE);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, 1 << 22);
        }

        // Writing 0x81 should set the LSB to 1
        z80_bus.set_bank(0x81);
        {
            let bus = z80_bus.bus.bus.borrow();
            assert_eq!(bus.z80_bank_addr, (1 << 23) | (1 << 21));
        }
    }

    #[test]
    fn test_set_bank_via_memory_interface_wraparound() {
        let mut z80_bus = create_test_z80_bus();

        // Ensure writing to 0x6000 directly works exactly like set_bank
        for _ in 0..9 {
            z80_bus.write_byte(0x6000, 1);
        }

        {
            let bus = z80_bus.bus.bus.borrow();
            let expected_addr = 0x1FF << 15;
            assert_eq!(bus.z80_bank_addr, expected_addr);
        }

        z80_bus.write_byte(0x60FF, 0); // Testing an address within the 6000-60FF range

        {
            let bus = z80_bus.bus.bus.borrow();
            // Expected address after right shifting and putting 0 at top bit
            let expected_addr = 0xFF << 15;
            assert_eq!(bus.z80_bank_addr, expected_addr);
        }
    }
}
