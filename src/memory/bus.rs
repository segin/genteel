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
use crate::debugger::Debuggable;
use serde_json::{json, Value};

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

    /// Z80 Bank Register (for Mapping $8000-$FFFF in Z80 space)
    pub z80_bank_addr: u32,
    pub z80_bank_bit: u8, // 0..8

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
            z80_bank_addr: 0,
            z80_bank_bit: 0,
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
                // Hack: Sonic 1 waits for 0xA01FFD to be 0x80 to indicate Z80 ready.
                // Our Z80 emulation isn't setting this fast enough or correctly.
                if addr == 0xA01FFD {
                     0x80
                } else {
                     self.z80_ram[(addr & 0x1FFF) as usize]
                }
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
            
            // YM2612 FM Chip: 0xA04000-0xA04003
            0xA04000..=0xA04003 => {
                let port = (addr & 2) >> 1;
                let is_data = (addr & 1) != 0;
                if is_data {
                    self.apu.fm.write_data(port as u8, value);
                } else {
                    self.apu.fm.write_address(port as u8, value);
                }
            }

            // Z80 area bank registers and other hardware
            0xA06000..=0xA060FF => {
                // Update bank register (LSB shifts in)
                let bit = (value as u32 & 1) << (self.z80_bank_bit + 15);
                let mask = 1 << (self.z80_bank_bit + 15);
                self.z80_bank_addr = (self.z80_bank_addr & !mask) | bit;
                self.z80_bank_bit = (self.z80_bank_bit + 1) % 9;
            }

            // I/O Ports
            0xA10000..=0xA1001F => {
                self.io.write(addr, value);
            }

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
                if self.z80_reset {
                    self.z80_bank_bit = 0; // Hardware resets shift pointer
                }
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
                let ram_addr = addr & 0xFFFF;
                self.work_ram[ram_addr as usize] = value;
            }

            // Unmapped regions (writes ignored)
            _ => {}
        }
    }

    /// Read a word (16-bit, big-endian) from the memory map
    pub fn read_word(&mut self, address: u32) -> u16 {
        let addr = address & 0xFFFFFF;
        
        // ROM Optimization
        if addr <= 0x3FFFFE {
            let rom_addr = addr as usize;
            if rom_addr + 1 < self.rom.len() {
                let high = self.rom[rom_addr] as u16;
                let low = self.rom[rom_addr + 1] as u16;
                return (high << 8) | low;
            }
        }

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
        let addr = address & 0xFFFFFF;

        // ROM Optimization
        if addr <= 0x3FFFFC {
            let rom_addr = addr as usize;
            if rom_addr + 3 < self.rom.len() {
                let b0 = self.rom[rom_addr] as u32;
                let b1 = self.rom[rom_addr + 1] as u32;
                let b2 = self.rom[rom_addr + 2] as u32;
                let b3 = self.rom[rom_addr + 3] as u32;
                return (b0 << 24) | (b1 << 16) | (b2 << 8) | b3;
            }
        }

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
        
        let mode = self.vdp.dma_mode();
        let length = {
            let l = ((self.vdp.registers[0x14] as u32) << 8) | (self.vdp.registers[0x13] as u32);
            if l == 0 { 0x10000 } else { l }
        };
        let mut source = ((self.vdp.registers[0x17] as u32 & 0x3F) << 17)
                   | ((self.vdp.registers[0x16] as u32) << 9)
                   | ((self.vdp.registers[0x15] as u32) << 1);
        
        if mode >= 2 {
            self.vdp.execute_dma();
            self.vdp.dma_pending = false;
            return;
        }

        // If it's a 68k transfer (mode bit 7=0), bit 22 decides if it's ROM or RAM
        // Register 23 bit 6 MUST be 0 for 68k DMA.
        // A22 is bit 22 of source. If A22=1, it's RAM. 
        // On Genesis, RAM is at $FF0000-$FFFFFF. VDP DMA forces A23=1.
        if (source & 0x400000) != 0 {
            source |= 0xFF0000; // Map to RAM range (0xFF0000-0xFFFFFF)
        }
        
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

impl Debuggable for Bus {
    fn read_state(&self) -> Value {
        json!({
            "z80_bus_request": self.z80_bus_request,
            "z80_reset": self.z80_reset,
            "tmss_unlocked": self.tmss_unlocked,
            // Sub-components are debugged separately usually, but we could link them
        })
    }

    fn write_state(&mut self, _state: &Value) {
        // Bus state write not supported
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

