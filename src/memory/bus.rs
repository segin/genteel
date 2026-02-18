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

use super::byte_utils;
use super::MemoryInterface;
use crate::apu::Apu;
use crate::debugger::Debuggable;
use crate::io::Io;
use crate::vdp::Vdp;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Sega Genesis Memory Bus
///
/// Routes memory accesses to the appropriate component based on address.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bus {
    /// ROM data (up to 4MB)
    #[serde(skip)]
    pub rom: Vec<u8>,

    /// Work RAM (64KB at 0xFF0000-0xFFFFFF, mirrored in 0xE00000-0xFFFFFF)
    pub work_ram: Box<[u8]>,

    /// Z80 RAM (8KB at 0xA00000-0xA01FFF)
    pub z80_ram: Box<[u8]>,

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

    /// Audio synchronization
    pub audio_accumulator: f32,
    #[serde(skip)]
    pub audio_buffer: Vec<i16>,
    pub sample_rate: u32,
}

impl Bus {
    /// Create a new empty bus
    pub fn new() -> Self {
        Self {
            rom: Vec::new(),
            work_ram: vec![0; 0x10000].into_boxed_slice(),
            z80_ram: vec![0; 0x2000].into_boxed_slice(),
            vdp: Vdp::new(),
            io: Io::new(),
            apu: Apu::new(),
            z80_bus_request: false,
            z80_reset: true, // Z80 starts in reset
            z80_bank_addr: 0,
            z80_bank_bit: 0,
            tmss_unlocked: false,
            audio_accumulator: 0.0,
            audio_buffer: Vec::with_capacity(2048),
            sample_rate: 44100,
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
            0x000000..=0x3FFFFF => self.read_rom(addr),
            0xA00000..=0xA0FFFF => self.read_z80_area(addr),
            0xA10000..=0xA1FFFF => self.read_io_area(addr),
            0xC00000..=0xC0FFFF => self.read_vdp_area(addr),
            0xE00000..=0xFFFFFF => self.read_ram(addr),
            _ => 0xFF,
        }
    }

    /// Write a byte to the memory map
    pub fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address & 0xFFFFFF;

        match addr {
            0x000000..=0x3FFFFF => {} // ROM is read-only
            0xA00000..=0xA0FFFF => self.write_z80_area(addr, value),
            0xA10000..=0xA1FFFF => self.write_io_area(addr, value),
            0xC00000..=0xC0FFFF => self.write_vdp_area(addr, value),
            0xE00000..=0xFFFFFF => self.write_ram(addr, value),
            _ => {}
        }
    }

    fn read_rom(&self, addr: u32) -> u8 {
        let rom_addr = addr as usize;
        if rom_addr < self.rom.len() {
            self.rom[rom_addr]
        } else {
            0xFF // Unmapped ROM area
        }
    }

    fn read_z80_area(&mut self, addr: u32) -> u8 {
        match addr {
            // Z80 RAM (8KB)
            0xA00000..=0xA01FFF => {
                if self.z80_bus_request {
                    self.z80_ram[(addr & 0x1FFF) as usize]
                } else {
                    0xFF
                }
            }
            // YM2612
            0xA04000..=0xA04003 => self.apu.fm.read((addr & 3) as u8),
            _ => 0xFF,
        }
    }

    fn read_io_area(&mut self, addr: u32) -> u8 {
        match addr {
            0xA10000..=0xA1001F => self.io.read(addr),
            0xA11100..=0xA11101 => {
                if self.z80_bus_request {
                    0x00
                } else {
                    0x01
                }
            }
            0xA11200..=0xA11201 => {
                if self.z80_reset {
                    0x00
                } else {
                    0x01
                }
            }
            _ => 0xFF,
        }
    }

    fn read_vdp_area(&mut self, addr: u32) -> u8 {
        match addr {
            0xC00000..=0xC00003 => (self.vdp.read_data() >> 8) as u8,
            0xC00004..=0xC00007 => {
                let val = self.vdp.read_status();
                if (addr & 1) == 0 {
                    (val >> 8) as u8
                } else {
                    (val & 0xFF) as u8
                }
            }
            0xC00008..=0xC0000F => (self.vdp.read_hv_counter() >> 8) as u8,
            0xC00010..=0xC00011 => 0xFF,
            _ => 0xFF,
        }
    }

    fn read_ram(&self, addr: u32) -> u8 {
        self.work_ram[(addr & 0xFFFF) as usize]
    }

    fn write_z80_area(&mut self, addr: u32, value: u8) {
        match addr {
            0xA00000..=0xA01FFF => {
                if self.z80_bus_request {
                    self.z80_ram[(addr & 0x1FFF) as usize] = value;
                }
            }
            0xA04000..=0xA04003 => {
                let port = (addr & 2) >> 1;
                let is_data = (addr & 1) != 0;
                if is_data {
                    self.apu.fm.write_data(port as u8, value);
                } else {
                    self.apu.fm.write_address(port as u8, value);
                }
            }
            0xA06000..=0xA060FF => {
                let bit = (value as u32 & 1) << (self.z80_bank_bit + 15);
                let mask = 1 << (self.z80_bank_bit + 15);
                self.z80_bank_addr = (self.z80_bank_addr & !mask) | bit;
                self.z80_bank_bit = (self.z80_bank_bit + 1) % 9;
            }
            _ => {}
        }
    }

    fn write_io_area(&mut self, addr: u32, value: u8) {
        match addr {
            0xA10000..=0xA1001F => self.io.write(addr, value),
            0xA11100 => self.z80_bus_request = (value & 0x01) != 0,
            0xA11200 => {
                self.z80_reset = (value & 0x01) == 0;
                if self.z80_reset {
                    self.z80_bank_bit = 0;
                }
            }
            _ => {}
        }
    }

    fn write_vdp_area(&mut self, addr: u32, value: u8) {
        match addr {
            0xC00011 => self.apu.psg.write(value),
            _ => {}
        }
    }

    fn write_ram(&mut self, addr: u32, value: u8) {
        let ram_addr = addr & 0xFFFF;
        self.work_ram[ram_addr as usize] = value;
    }

    /// Read a word (16-bit, big-endian) from the memory map
    #[inline]
    pub fn read_word(&mut self, address: u32) -> u16 {
        let addr = address & 0xFFFFFF;

        // ROM Fast Path
        if addr <= 0x3FFFFF {
            let idx = addr as usize;
            if idx + 1 < self.rom.len() {
                // Verified safe: Use idiomatic from_be_bytes
                return u16::from_be_bytes(self.rom[idx..idx + 2].try_into().unwrap());
            } else if idx < self.rom.len() {
                // Partial read at end of ROM
                let high = self.rom[idx];
                let low = 0xFF; // Unmapped
                return byte_utils::join_u16(high, low);
            } else {
                return 0xFFFF; // Unmapped
            }
        }

        // VDP Ports
        if (0xC00000..=0xC0001F).contains(&addr) {
            let offset = addr & 0x1F;
            if offset < 4 {
                return self.vdp.read_data();
            }
            if offset < 8 {
                return self.vdp.read_status();
            }
            if offset < 0x10 {
                return self.vdp.read_hv_counter();
            }
            return 0xFFFF;
        }

        // Optimize Work RAM access (0xE00000-0xFFFFFF, 64KB mirrored)
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr < 0xFFFF {
                return byte_utils::join_u16(self.work_ram[r_addr], self.work_ram[r_addr + 1]);
            }
        }

        let high = self.read_byte(address);
        let low = self.read_byte(address.wrapping_add(1));
        byte_utils::join_u16(high, low)
    }

    /// Write a word (16-bit, big-endian) to the memory map
    pub fn write_word(&mut self, address: u32, value: u16) {
        let addr = address & 0xFFFFFF;

        // VDP Ports
        if (0xC00000..=0xC00007).contains(&addr) {
            if (addr & 0x1F) < 4 {
                self.vdp.write_data(value);
            } else {
                self.vdp.write_control(value);
                self.handle_dma();
            }
            return;
        }

        // Optimize Work RAM access
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr < 0xFFFF {
                let (high, low) = byte_utils::split_u16(value);
                self.work_ram[r_addr] = high;
                self.work_ram[r_addr + 1] = low;
                return;
            }
        }

        let (high, low) = byte_utils::split_u16(value);
        self.write_byte(address, high);
        self.write_byte(address.wrapping_add(1), low);
    }

    /// Read a long word (32-bit, big-endian) from the memory map
    #[inline]
    pub fn read_long(&mut self, address: u32) -> u32 {
        let addr = address & 0xFFFFFF;

        // ROM Fast Path
        if addr <= 0x3FFFFF {
            let idx = addr as usize;
            if idx + 3 < self.rom.len() {
                // Verified safe: Use idiomatic from_be_bytes
                return u32::from_be_bytes(self.rom[idx..idx + 4].try_into().unwrap());
            }
        }

        // VDP Data Port (Long access = 2 word reads)
        if addr == 0xC00000 {
            let high = self.vdp.read_data();
            let low = self.vdp.read_data();
            return ((high as u32) << 16) | (low as u32);
        }
        // VDP Control Port (Long access)
        if addr == 0xC00004 {
            let high = self.vdp.read_status();
            let low = self.vdp.read_status();
            return ((high as u32) << 16) | (low as u32);
        }

        // Optimize Work RAM access
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr <= 0xFFFC {
                return byte_utils::join_u32(
                    self.work_ram[r_addr],
                    self.work_ram[r_addr + 1],
                    self.work_ram[r_addr + 2],
                    self.work_ram[r_addr + 3],
                );
            }
        }

        // VDP Ports (Unaligned/Other)
        if (0xC00000..=0xC0001F).contains(&addr) {
            let offset = addr & 0x1F;
            // VDP H/V Counter
            if offset == 8 {
                let high = self.vdp.read_hv_counter();
                let low = self.vdp.read_hv_counter();
                return ((high as u32) << 16) | (low as u32);
            }
            // Unaligned/Other VDP Access
            let high = self.read_word(address);
            let low = self.read_word(address.wrapping_add(2));
            return ((high as u32) << 16) | (low as u32);
        }

        let b0 = self.read_byte(address);
        let b1 = self.read_byte(address.wrapping_add(1));
        let b2 = self.read_byte(address.wrapping_add(2));
        let b3 = self.read_byte(address.wrapping_add(3));
        byte_utils::join_u32(b0, b1, b2, b3)
    }

    /// Write a long word (32-bit, big-endian) to the memory map
    pub fn write_long(&mut self, address: u32, value: u32) {
        let addr = address & 0xFFFFFF;

        // VDP Data Port (Long access = 2 word writes)
        if (0xC00000..=0xC00003).contains(&addr) {
            let high = (value >> 16) as u16;
            let low = (value & 0xFFFF) as u16;
            self.vdp.write_data(high);
            self.vdp.write_data(low);
            return;
        }

        // VDP Control Port (Long access)
        if (0xC00004..=0xC00007).contains(&addr) {
            let (high, low) = byte_utils::split_u32_to_u16(value);
            self.vdp.write_control(high);
            self.handle_dma();
            self.vdp.write_control(low);
            self.handle_dma();
            return;
        }

        // Optimize Work RAM access
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr <= 0xFFFC {
                let (b0, b1, b2, b3) = byte_utils::split_u32(value);
                self.work_ram[r_addr] = b0;
                self.work_ram[r_addr + 1] = b1;
                self.work_ram[r_addr + 2] = b2;
                self.work_ram[r_addr + 3] = b3;
                return;
            }
        }

        let (b0, b1, b2, b3) = byte_utils::split_u32(value);
        self.write_byte(address, b0);
        self.write_byte(address.wrapping_add(1), b1);
        self.write_byte(address.wrapping_add(2), b2);
        self.write_byte(address.wrapping_add(3), b3);
    }

    fn handle_dma(&mut self) {
        if self.vdp.dma_pending {
            if self.vdp.is_dma_transfer() {
                self.run_dma();
            } else {
                self.vdp.execute_dma();
            }
        }
    }

    fn run_dma(&mut self) {
        if !self.vdp.is_dma_transfer() {
            return;
        }
        let length = self.vdp.dma_length() as usize;
        let source = self.vdp.dma_source_transfer();
        let _step = self.vdp.registers[15] as u16;
        for i in 0..length {
            let src_addr = source + (i * 2) as u32;
            let val = self.read_word(src_addr);
            self.vdp.write_data(val);
        }
        self.vdp.dma_pending = false;
    }
}

impl MemoryInterface for Bus {
    #[inline(always)]
    fn read_byte(&mut self, address: u32) -> u8 {
        self.read_byte(address)
    }
    #[inline(always)]
    fn write_byte(&mut self, address: u32, value: u8) {
        self.write_byte(address, value)
    }
    #[inline(always)]
    fn read_word(&mut self, address: u32) -> u16 {
        self.read_word(address)
    }
    #[inline(always)]
    fn write_word(&mut self, address: u32, value: u16) {
        self.write_word(address, value)
    }
    #[inline(always)]
    fn read_long(&mut self, address: u32) -> u32 {
        self.read_long(address)
    }
    #[inline(always)]
    fn write_long(&mut self, address: u32, value: u32) {
        self.write_long(address, value)
    }
}

impl Debuggable for Bus {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        if let Ok(mut new_bus) = serde_json::from_value::<Bus>(state.clone()) {
            new_bus.rom = std::mem::take(&mut self.rom);
            if new_bus.vdp.framebuffer.len() != 320 * 240 {
                new_bus.vdp.framebuffer.resize(320 * 240, 0);
            }
            new_bus.vdp.reconstruct_cram_cache();
            *self = new_bus;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_bus_state_serialization() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024) // 16MB
            .spawn(|| {
                let mut bus = Bus::new();

                // Modify state
                bus.z80_bus_request = true;
                bus.z80_reset = false;
                bus.z80_bank_addr = 0x12345;

                // Serialize
                let state_value = bus.read_state();

                // Create new bus
                let mut new_bus = Bus::new();

                // Deserialize
                new_bus.write_state(&state_value);

                // Assert equality
                assert_eq!(new_bus.z80_bus_request, true);
                assert_eq!(new_bus.z80_reset, false);
                assert_eq!(new_bus.z80_bank_addr, 0x12345);

                // Verify VDP/IO/APU keys exist in JSON
                assert!(state_value.get("vdp").is_some());
                assert!(state_value.get("io").is_some());
                assert!(state_value.get("apu").is_some());
            })
            .unwrap()
            .join()
            .unwrap();
    }
}
