//! Sega Genesis/Mega Drive Memory Bus
//!
//! This module implements the full memory map of the Genesis, routing
//! reads and writes to the appropriate components.
//!
//! ## Genesis Memory Map
//!
//! | Address Range      | Size   | Description                    |
//! |:-------------------|:-------|:-------------------------------|
//! | 0x000000-0x3FFFFF  | 4 MB   | ROM/Cartridge                  |
//! | 0x400000-0x7FFFFF  | 4 MB   | Expansion (unused/reserved)    |
//! | 0x800000-0x9FFFFF  | 2 MB   | Reserved                       |
//! | 0xA00000-0xA0FFFF  | 64 KB  | Z80 Address Space              |
//! | 0xA10000-0xA1001F  | 32 B   | I/O Ports                      |
//! | 0xA10020-0xA10FFF  | ~4 KB  | Reserved                       |
//! | 0xA11000-0xA110FF  | 256 B  | Z80 Bus Control / Expansion    |
//! | 0xA11100-0xA11101  | 2 B    | Z80 Bus Request                |
//! | 0xA11200-0xA11201  | 2 B    | Z80 Reset                      |
//! | 0xB00000-0xBFFFFF  | 1 MB   | Reserved                       |
//! | 0xC00000-0xC0001F  | 32 B   | VDP Ports                      |
//! | 0xE00000-0xFFFFFF  | 2 MB   | Work RAM (64KB mirrored)       |

use super::MemoryInterface;
use crate::vdp::Vdp;
use crate::io::Io;

/// Sega Genesis Memory Bus
///
/// Routes memory accesses to the appropriate component based on address.
#[derive(Debug)]
pub struct Bus {
    /// ROM data (up to 4MB)
    pub rom: Vec<u8>,

    /// Work RAM (64KB at 0xFF0000-0xFFFFFF, mirrored in 0xE00000-0xFFFFFF)
    pub work_ram: [u8; 0x10000],

    /// Z80 RAM (8KB at 0xA00000-0xA01FFF)
    pub z80_ram: [u8; 0x2000],

    /// VDP Port access
    pub vdp: Vdp,

    /// I/O ports (A10000-A1001F)
    pub io: Io,

    /// Z80 bus control registers
    pub z80_bus_request: bool,
    pub z80_reset: bool,

    /// TMSS (Trademark Security System) - lock/unlock state
    pub tmss_unlocked: bool,
}

impl Bus {
    /// Create a new empty bus
    pub fn new() -> Self {
        Self {
            rom: Vec::new(),
            work_ram: [0; 0x10000],
            z80_ram: [0; 0x2000],
            vdp: Vdp::new(),
            io: Io::new(),
            z80_bus_request: false,
            z80_reset: true, // Z80 starts in reset
            tmss_unlocked: false,
        }
    }

    /// Load a ROM into the bus
    pub fn load_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
        // Pad ROM to at least 512 bytes to ensure vector table exists
        if self.rom.len() < 512 {
            self.rom.resize(512, 0);
        }
    }

    /// Clear the ROM
    pub fn clear_rom(&mut self) {
        self.rom.clear();
    }

    /// Get ROM size
    pub fn rom_size(&self) -> usize {
        self.rom.len()
    }

    /// Read a byte from the memory map
    pub fn read_byte(&mut self, address: u32) -> u8 {
        let addr = address & 0xFFFFFF; // 24-bit address bus

        match addr {
            // ROM: 0x000000-0x3FFFFF
            0x000000..=0x3FFFFF => {
                let rom_addr = addr as usize;
                if rom_addr < self.rom.len() {
                    self.rom[rom_addr]
                } else {
                    0xFF // Unmapped ROM area
                }
            }

            // Z80 Address Space: 0xA00000-0xA0FFFF
            0xA00000..=0xA01FFF => {
                // Z80 RAM (8KB)
                self.z80_ram[(addr & 0x1FFF) as usize]
            }
            0xA02000..=0xA0FFFF => {
                // Z80 area bank registers and other hardware
                0xFF
            }

            // I/O Ports: 0xA10000-0xA1001F
            0xA10000..=0xA1001F => {
                self.io.read(addr)
            }

            // Z80 Bus Request: 0xA11100
            0xA11100..=0xA11101 => {
                if self.z80_bus_request { 0x00 } else { 0x01 }
            }

            // Z80 Reset: 0xA11200
            0xA11200..=0xA11201 => {
                if self.z80_reset { 0x00 } else { 0x01 }
            }

            // VDP Ports: 0xC00000-0xC0001F
            0xC00000..=0xC00003 => {
                // VDP data port
                (self.vdp.read_data() >> 8) as u8 // Placeholder: usually word-only
            }
            0xC00004..=0xC00005 => {
                // VDP status
                (self.vdp.read_status() >> 8) as u8
            }
            0xC00006..=0xC00007 => {
                (self.vdp.read_status() & 0xFF) as u8
            }
            0xC00008..=0xC0000F => {
                // HV counter
                (self.vdp.read_hv_counter() >> 8) as u8 // Just a stub for byte read
            }
            0xC00010..=0xC0001F => {
                // Reserved
                0xFF
            }

            // Work RAM: 0xE00000-0xFFFFFF (64KB mirrored)
            0xE00000..=0xFFFFFF => {
                self.work_ram[(addr & 0xFFFF) as usize]
            }

            // Unmapped regions
            _ => 0xFF,
        }
    }

    /// Write a byte to the memory map
    pub fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address & 0xFFFFFF;

        match addr {
            // ROM is read-only (writes are ignored)
            0x000000..=0x3FFFFF => {
                // Some mappers/SRAM use writes here, but basic implementation ignores
            }

            // Z80 RAM
            0xA00000..=0xA01FFF => {
                self.z80_ram[(addr & 0x1FFF) as usize] = value;
            }

            // I/O Ports
            0xA10000..=0xA1001F => {
                self.io.write(addr, value);
            }

            // Z80 Bus Request
            0xA11100..=0xA11101 => {
                self.z80_bus_request = (value & 0x01) != 0;
            }

            // Z80 Reset
            0xA11200..=0xA11201 => {
                self.z80_reset = (value & 0x01) == 0;
            }

            // VDP Ports
            0xC00000..=0xC00003 => {
                // VDP data port - placeholder (writes are usually words)
            }
            0xC00004..=0xC00007 => {
                // VDP control port - placeholder
            }

            // Work RAM
            0xE00000..=0xFFFFFF => {
                self.work_ram[(addr & 0xFFFF) as usize] = value;
            }

            // Unmapped regions (writes ignored)
            _ => {}
        }
    }

    /// Read a word (16-bit, big-endian) from the memory map
    pub fn read_word(&mut self, address: u32) -> u16 {
        let addr = address & 0xFFFFFF;
        
        // VDP Data Port (Word access)
        if addr >= 0xC00000 && addr <= 0xC00003 {
            return self.vdp.read_data();
        }
        // VDP Control Port / Status
        if addr >= 0xC00004 && addr <= 0xC00007 {
            return self.vdp.read_status();
        }
        // VDP H/V Counter
        if addr >= 0xC00008 && addr <= 0xC0000F {
            return self.vdp.read_hv_counter();
        }

        let high = self.read_byte(address) as u16;
        let low = self.read_byte(address.wrapping_add(1)) as u16;
        (high << 8) | low
    }

    /// Write a word (16-bit, big-endian) to the memory map
    pub fn write_word(&mut self, address: u32, value: u16) {
        let addr = address & 0xFFFFFF;

        // VDP Data Port
        if addr >= 0xC00000 && addr <= 0xC00003 {
            self.vdp.write_data(value);
            return;
        }
        // VDP Control Port
        if addr >= 0xC00004 && addr <= 0xC00007 {
            self.vdp.write_control(value);
            return;
        }

        self.write_byte(address, (value >> 8) as u8);
        self.write_byte(address.wrapping_add(1), value as u8);
    }

    /// Read a long word (32-bit, big-endian) from the memory map
    pub fn read_long(&mut self, address: u32) -> u32 {
        let high = self.read_word(address) as u32;
        let low = self.read_word(address.wrapping_add(2)) as u32;
        (high << 16) | low
    }

    /// Write a long word (32-bit, big-endian) to the memory map
    pub fn write_long(&mut self, address: u32, value: u32) {
        self.write_word(address, (value >> 16) as u16);
        self.write_word(address.wrapping_add(2), value as u16);
    }
}

impl MemoryInterface for Bus {
    fn read_byte(&mut self, address: u32) -> u8 {
        self.read_byte(address)
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        self.write_byte(address, value);
    }

    fn read_word(&mut self, address: u32) -> u16 {
        self.read_word(address)
    }

    fn write_word(&mut self, address: u32, value: u16) {
        self.write_word(address, value);
    }

    fn read_long(&mut self, address: u32) -> u32 {
        self.read_long(address)
    }

    fn write_long(&mut self, address: u32, value: u32) {
        self.write_long(address, value);
    }
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_rom_loading() {
        let mut bus = Bus::new();
        let rom_data = vec![0x00, 0x01, 0x02, 0x03, 0x04, 0x05];
        bus.load_rom(&rom_data);

        assert_eq!(bus.read_byte(0x000000), 0x00);
        assert_eq!(bus.read_byte(0x000001), 0x01);
        assert_eq!(bus.read_byte(0x000005), 0x05);
    }

    #[test]
    fn test_rom_read_word() {
        let mut bus = Bus::new();
        let rom_data = vec![0x12, 0x34, 0x56, 0x78];
        bus.load_rom(&rom_data);

        assert_eq!(bus.read_word(0x000000), 0x1234);
        assert_eq!(bus.read_word(0x000002), 0x5678);
    }

    #[test]
    fn test_rom_read_long() {
        let mut bus = Bus::new();
        let rom_data = vec![0x12, 0x34, 0x56, 0x78];
        bus.load_rom(&rom_data);

        assert_eq!(bus.read_long(0x000000), 0x12345678);
    }

    #[test]
    fn test_work_ram_read_write() {
        let mut bus = Bus::new();

        bus.write_byte(0xFF0000, 0x42);
        assert_eq!(bus.read_byte(0xFF0000), 0x42);

        bus.write_word(0xFF1000, 0xABCD);
        assert_eq!(bus.read_word(0xFF1000), 0xABCD);

        bus.write_long(0xFF2000, 0x12345678);
        assert_eq!(bus.read_long(0xFF2000), 0x12345678);
    }

    #[test]
    fn test_work_ram_mirroring() {
        let mut bus = Bus::new();

        // Write to 0xFF0000
        bus.write_byte(0xFF0000, 0x42);

        // Should be readable at mirrored addresses in 0xE00000-0xFFFFFF range
        assert_eq!(bus.read_byte(0xEF0000), 0x42);
        assert_eq!(bus.read_byte(0xFF0000), 0x42);
    }

    #[test]
    fn test_z80_ram() {
        let mut bus = Bus::new();

        bus.write_byte(0xA00000, 0x55);
        assert_eq!(bus.read_byte(0xA00000), 0x55);

        bus.write_byte(0xA01FFF, 0xAA);
        assert_eq!(bus.read_byte(0xA01FFF), 0xAA);
    }

    #[test]
    fn test_io_ports() {
        let mut bus = Bus::new();

        bus.write_byte(0xA10001, 0x40);
        assert_eq!(bus.read_byte(0xA10001), 0x40);
    }

    #[test]
    fn test_z80_bus_control() {
        let mut bus = Bus::new();

        // Request Z80 bus
        bus.write_byte(0xA11100, 0x01);
        assert!(bus.z80_bus_request);

        // Release Z80 bus
        bus.write_byte(0xA11100, 0x00);
        assert!(!bus.z80_bus_request);
    }

    #[test]
    fn test_unmapped_returns_ff() {
        let mut bus = Bus::new();

        // Unmapped ROM area
        assert_eq!(bus.read_byte(0x100000), 0xFF);

        // Reserved area
        assert_eq!(bus.read_byte(0x800000), 0xFF);
    }
}
