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
use crate::apu::Apu;

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
    
    /// Audio Processing Unit (YM2612 + PSG)
    pub apu: Apu,

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
            apu: Apu::new(),
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
            // YM2612 from 68k: 0xA04000-0xA04003
            0xA04000..=0xA04003 => {
                self.apu.fm.read((addr & 3) as u8)
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
            // PSG: 0xC00010-0xC00011 (write-only, reads return FF)
            0xC00010..=0xC00011 => 0xFF,
            0xC00012..=0xC0001F => {
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
            
            // YM2612 from 68k: 0xA04000-0xA04003
            0xA04000..=0xA04003 => {
                let port = ((addr & 2) >> 1) as u8;  // 0 for 4000/4001, 1 for 4002/4003
                let is_data = (addr & 1) != 0;
                
                if is_data {
                    self.apu.fm.write_data(port, value);
                } else {
                    self.apu.fm.write_address(port, value);
                }
            }

            // I/O Ports
            0xA10000..=0xA1001F => {
                self.io.write(addr, value);
            }

            // Z80 Bus Request
            // Z80 Bus Request
            0xA11100 => {
                self.z80_bus_request = (value & 0x01) != 0;
            }
            0xA11101 => {
                // Ignore writes to lower byte of Z80 bus request
            }

            // Z80 Reset
            0xA11200 => {
                self.z80_reset = (value & 0x01) == 0;
            }
            0xA11201 => {}

            // VDP Ports
            0xC00000..=0xC00003 => {
                // VDP data port - placeholder (writes are usually words)
            }
            0xC00004..=0xC00007 => {
                // VDP control port - placeholder
            }
            // PSG: 0xC00011
            0xC00011 => {
                self.apu.psg.write(value);
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
            if self.vdp.dma_pending {
                self.run_dma();
            }
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
    // === DMA ===

    fn run_dma(&mut self) {
        // VDP registers:
        // 0x13/0x14: DMA Length (low/high)
        // 0x15/0x16/0x17: DMA Source (low/mid/high)
        
        // Mode check (Reg 23 bit 7, 6)
        // 00/01: 68k -> VDP (Supported)
        // 10: VRAM Copy (Should be internal to VDP)
        // 11: VRAM Fill (Should be internal)
        let mode = self.vdp.registers[0x17] >> 6;

        
        // Debug DMA
        // println!("DMA Check: Mode={} dma_enabled={} pending={}", mode, self.vdp.dma_enabled(), self.vdp.dma_pending);

        if (mode & 0x02) != 0 {
            // Not 68k transfer (VDP copy/fill)
            // Ideally VDP handles these itself, or we need another handler.
            // For now, assume VDP handles them or they are triggered differently.
            // Clear pending just in case.
            self.vdp.dma_pending = false;
            return;
        }

        let len_low = self.vdp.registers[0x13];
        let len_high = self.vdp.registers[0x14];
        let length = ((len_high as u32) << 8) | (len_low as u32);
        
        // Source address (word index in regs 21-23)
        // Reg 23 (A22-A17), Reg 22 (A16-A9), Reg 21 (A8-A1)
        let src_low = self.vdp.registers[0x15];
        let src_mid = self.vdp.registers[0x16];
        let src_high = self.vdp.registers[0x17] & 0x3F; // Mask mode bits? (bits 0-5 are address)

        let mut source = ((src_high as u32) << 17)
                       | ((src_mid as u32) << 9)
                       | ((src_low as u32) << 1);
        
        println!("DMA EXECUTE: Mode={} Length=0x{:X} Source=0x{:06X}", mode, length, source);
        
        // Transfer
        for _ in 0..length {
            let word = self.read_word(source);
            self.vdp.write_data(word);
            source = source.wrapping_add(2);
        }

        self.vdp.dma_pending = false;
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

        bus.write_byte(0xA10009, 0x40);
        assert_eq!(bus.read_byte(0xA10009), 0x40);
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

