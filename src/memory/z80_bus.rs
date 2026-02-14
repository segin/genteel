//! Z80 Bus Adapter for Genesis Sound System
//!
//! Routes Z80 memory accesses to the appropriate Genesis components:
//! - 0000h-1FFFh: Z80 Sound RAM (8KB)
//! - 4000h-4003h: YM2612 FM Chip
//! - 6000h: Bank Register (sets 32KB window into 68k memory)
//! - 7F11h: SN76489 PSG
//! - 8000h-FFFFh: Banked 68k Memory (32KB window)

use super::{IoInterface, MemoryInterface};
use crate::memory::bus::Bus;

/// Z80 Bus adapter that routes memory accesses to Genesis components
#[derive(Debug)]
pub struct Z80Bus<'a> {
    /// Reference to the main Genesis bus
    pub bus: &'a mut Bus,
}

impl<'a> Z80Bus<'a> {
    /// Create a new Z80 bus adapter
    pub fn new(bus: &'a mut Bus) -> Self {
        Self { bus }
    }

    /// Set the bank register (called on write to $6000)
    /// The value written becomes the upper bits of the 68k address
    pub fn set_bank(&mut self, value: u8) {
        // Delegate to shared bus so 68k and Z80 see the same state
        self.bus.write_byte(0xA06000, value);
    }

    /// Reset bank register to 0
    pub fn reset_bank(&mut self) {
        self.bus.z80_bank_addr = 0;
        self.bus.z80_bank_bit = 0;
    }
}

impl<'a> MemoryInterface for Z80Bus<'a> {
    fn read_byte(&mut self, address: u32) -> u8 {
        let addr = address as u16;

        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => self.bus.z80_ram[addr as usize],

            // Mirror of Z80 RAM: 2000h-3FFFh
            0x2000..=0x3FFF => self.bus.z80_ram[(addr & 0x1FFF) as usize],

            // YM2612: 4000h-4003h
            0x4000..=0x4003 => self.bus.apu.fm.read((addr & 3) as u8),

            // FM Mirror or PSG/Bank area
            0x4004..=0x5FFF => 0xFF,

            // Bank register area: 6000h (write-only)
            0x6000..=0x7FFF => 0xFF,

            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let bank_addr = self.bus.z80_bank_addr;
                let effective_addr = bank_addr | ((addr as u32) & 0x7FFF);
                // Note: accessing 68k memory via Z80 bus is subject to bus arbitration
                // But here we just read it directly as we are the Z80 thread/step
                self.bus.read_byte(effective_addr)
            }
        }
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address as u16;

        match addr {
            // Z80 Sound RAM: 0000h-1FFFh
            0x0000..=0x1FFF => {
                self.bus.z80_ram[addr as usize] = value;
            }

            // Mirror of Z80 RAM: 2000h-3FFFh
            0x2000..=0x3FFF => {
                self.bus.z80_ram[(addr & 0x1FFF) as usize] = value;
            }

            // YM2612: 4000h-4003h
            0x4000..=0x4003 => {
                let port = (addr & 2) >> 1;
                let is_data = (addr & 1) != 0;
                if is_data {
                    self.bus.apu.fm.write_data(port as u8, value);
                } else {
                    self.bus.apu.fm.write_address(port as u8, value);
                }
            }

            // Mirror of FM chip or Reserved: 4004h-5FFFh
            0x4004..=0x5FFF => {}

            // Bank register: 6000h
            0x6000..=0x60FF => {
                self.set_bank(value);
            }

            // Reserved / PSG area
            0x6100..=0x7F10 => {}
            0x7F11 => {
                self.bus.apu.psg.write(value);
            }
            0x7F12..=0x7FFF => {}

            // Banked 68k memory: 8000h-FFFFh
            0x8000..=0xFFFF => {
                let bank_addr = self.bus.z80_bank_addr;
                let effective_addr = bank_addr | ((addr as u32) & 0x7FFF);
                self.bus.write_byte(effective_addr, value);
            }
        }
    }

    fn read_word(&mut self, address: u32) -> u16 {
        let hi = self.read_byte(address) as u16;
        let lo = self.read_byte(address.wrapping_add(1)) as u16;
        (hi << 8) | lo
    }

    fn write_word(&mut self, address: u32, value: u16) {
        self.write_byte(address, (value >> 8) as u8);
        self.write_byte(address.wrapping_add(1), value as u8);
    }

    fn read_long(&mut self, address: u32) -> u32 {
        let hi = self.read_word(address) as u32;
        let lo = self.read_word(address.wrapping_add(2)) as u32;
        (hi << 16) | lo
    }

    fn write_long(&mut self, address: u32, value: u32) {
        self.write_word(address, (value >> 16) as u16);
        self.write_word(address.wrapping_add(2), value as u16);
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

impl<'a> IoInterface for Z80Bus<'a> {
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

    #[test]
    fn test_z80_ram_read_write() {
        let mut bus = Bus::new();
        let mut z80_bus = Z80Bus::new(&mut bus);

        z80_bus.write_byte(0x0000, 0x42);
        assert_eq!(z80_bus.read_byte(0x0000), 0x42);

        z80_bus.write_byte(0x1FFF, 0xAB);
        assert_eq!(z80_bus.read_byte(0x1FFF), 0xAB);
    }

    #[test]
    fn test_bank_register() {
        let mut bus = Bus::new();
        let mut z80_bus = Z80Bus::new(&mut bus);

        // Initially bank is 0
        assert_eq!(z80_bus.bus.z80_bank_addr, 0);

        // Write to bank register (bit-by-bit shifting)
        z80_bus.write_byte(0x6000, 0x01); // Shift in 1

        assert_ne!(z80_bus.bus.z80_bank_addr, 0);
    }

    #[test]
    fn test_reserved_reads_ff() {
        let mut bus = Bus::new();
        let mut z80_bus = Z80Bus::new(&mut bus);

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
        let mut bus = Bus::new();
        let mut z80_bus = Z80Bus::new(&mut bus);

        // All I/O port reads should return 0xFF on Genesis
        assert_eq!(z80_bus.read_port(0x0000), 0xFF);
        assert_eq!(z80_bus.read_port(0x007F), 0xFF);
        assert_eq!(z80_bus.read_port(0xFFFF), 0xFF);

        // Writes should not panic
        z80_bus.write_port(0x0000, 0x42);
        z80_bus.write_port(0xFFFF, 0xAB);
    }
}
