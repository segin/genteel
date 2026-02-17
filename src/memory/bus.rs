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
use serde_json::{json, Value};

// Memory Map Constants
const ROM_START: u32 = 0x000000;
const ROM_END: u32 = 0x3FFFFF;
const Z80_RAM_START: u32 = 0xA00000;
const Z80_RAM_END: u32 = 0xA01FFF;
const YM2612_START: u32 = 0xA04000;
const YM2612_END: u32 = 0xA04003;
const Z80_BANK_REG_START: u32 = 0xA06000;
const Z80_BANK_REG_END: u32 = 0xA060FF;
const Z80_LEGACY_START: u32 = 0xA02000;
const Z80_LEGACY_END: u32 = 0xA0FFFF;
const IO_START: u32 = 0xA10000;
const IO_END: u32 = 0xA1001F;
const Z80_BUS_REQ_START: u32 = 0xA11100;
const Z80_BUS_REQ_END: u32 = 0xA11101;
const Z80_RESET_START: u32 = 0xA11200;
const Z80_RESET_END: u32 = 0xA11201;
const VDP_DATA_START: u32 = 0xC00000;
const VDP_DATA_END: u32 = 0xC00003;
const VDP_CTRL_START: u32 = 0xC00004;
const VDP_CTRL_END: u32 = 0xC00007;
const VDP_HV_START: u32 = 0xC00008;
const VDP_HV_END: u32 = 0xC0000F;
const PSG_START: u32 = 0xC00010;
const PSG_END: u32 = 0xC00011; // Write-only
const WORK_RAM_START: u32 = 0xE00000;
const WORK_RAM_END: u32 = 0xFFFFFF;

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

    /// Audio synchronization
    pub audio_accumulator: f32,
    pub audio_buffer: Vec<i16>,
    pub sample_rate: u32,
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

    // --- Private Helper Methods for Read ---

    #[inline]
    fn read_rom_byte(&self, addr: u32) -> u8 {
        let rom_addr = addr as usize;
        if rom_addr < self.rom.len() {
            self.rom[rom_addr]
        } else {
            0xFF // Unmapped ROM area
        }
    }

    #[inline]
    fn read_z80_ram_byte(&self, addr: u32) -> u8 {
        if self.z80_bus_request {
            self.z80_ram[(addr & 0x1FFF) as usize]
        } else {
            0xFF
        }
    }

    #[inline]
    fn read_ym2612_byte(&mut self, addr: u32) -> u8 {
        self.apu.fm.read((addr & 3) as u8)
    }

    #[inline]
    fn read_io_byte(&mut self, addr: u32) -> u8 {
        self.io.read(addr)
    }

    #[inline]
    fn read_z80_bus_req_byte(&self) -> u8 {
        if self.z80_bus_request {
            0x00
        } else {
            0x01
        }
    }

    #[inline]
    fn read_z80_reset_byte(&self) -> u8 {
        if self.z80_reset {
            0x00
        } else {
            0x01
        }
    }

    #[inline]
    fn read_vdp_byte(&mut self, addr: u32) -> u8 {
        match addr {
            VDP_DATA_START..=VDP_DATA_END => (self.vdp.read_data() >> 8) as u8,
            VDP_CTRL_START..=0xC00005 => (self.vdp.read_status() >> 8) as u8,
            0xC00006..=VDP_CTRL_END => (self.vdp.read_status() & 0xFF) as u8,
            VDP_HV_START..=VDP_HV_END => (self.vdp.read_hv_counter() >> 8) as u8,
            _ => 0xFF,
        }
    }

    #[inline]
    fn read_work_ram_byte(&self, addr: u32) -> u8 {
        self.work_ram[(addr & 0xFFFF) as usize]
    }

    /// Read a byte from the memory map
    pub fn read_byte(&mut self, address: u32) -> u8 {
        let addr = address & 0xFFFFFF; // 24-bit address bus

        match addr {
            // ROM: 0x000000-0x3FFFFF
            ROM_START..=ROM_END => self.read_rom_byte(addr),

            // Z80 Address Space: 0xA00000-0xA0FFFF (Z80 RAM part)
            Z80_RAM_START..=Z80_RAM_END => self.read_z80_ram_byte(addr),

            // YM2612 from 68k: 0xA04000-0xA04003
            YM2612_START..=YM2612_END => self.read_ym2612_byte(addr),

            // Z80 area bank registers and other hardware
            Z80_LEGACY_START..=Z80_LEGACY_END => {
                // Includes Z80_BANK_REG_START..=Z80_BANK_REG_END
                0xFF
            }

            // I/O Ports: 0xA10000-0xA1001F
            IO_START..=IO_END => self.read_io_byte(addr),

            // Z80 Bus Request: 0xA11100
            Z80_BUS_REQ_START..=Z80_BUS_REQ_END => self.read_z80_bus_req_byte(),

            // Z80 Reset: 0xA11200
            Z80_RESET_START..=Z80_RESET_END => self.read_z80_reset_byte(),

            // VDP Ports: 0xC00000-0xC0000F
            VDP_DATA_START..=VDP_HV_END => self.read_vdp_byte(addr),

            // PSG: 0xC00010-0xC00011 (write-only, reads return FF)
            PSG_START..=PSG_END => 0xFF,

            // Reserved VDP area
            0xC00012..=0xC0001F => 0xFF,

            // Work RAM: 0xE00000-0xFFFFFF (64KB mirrored)
            WORK_RAM_START..=WORK_RAM_END => self.read_work_ram_byte(addr),

            // Unmapped regions
            _ => 0xFF,
        }
    }

    // --- Private Helper Methods for Write ---

    fn write_z80_ram_byte(&mut self, addr: u32, value: u8) {
        if self.z80_bus_request {
            self.z80_ram[(addr & 0x1FFF) as usize] = value;
        }
    }

    fn write_ym2612_byte(&mut self, addr: u32, value: u8) {
        let port = (addr & 2) >> 1;
        let is_data = (addr & 1) != 0;
        if is_data {
            self.apu.fm.write_data(port as u8, value);
        } else {
            self.apu.fm.write_address(port as u8, value);
        }
    }

    fn write_z80_bank_reg_byte(&mut self, _addr: u32, value: u8) {
        // Update bank register (LSB shifts in)
        let bit = (value as u32 & 1) << (self.z80_bank_bit + 15);
        let mask = 1 << (self.z80_bank_bit + 15);
        self.z80_bank_addr = (self.z80_bank_addr & !mask) | bit;
        self.z80_bank_bit = (self.z80_bank_bit + 1) % 9;
    }

    fn write_io_byte(&mut self, addr: u32, value: u8) {
        self.io.write(addr, value);
    }

    fn write_z80_bus_req_byte(&mut self, value: u8) {
        self.z80_bus_request = (value & 0x01) != 0;
    }

    fn write_z80_reset_byte(&mut self, value: u8) {
        self.z80_reset = (value & 0x01) == 0;
        if self.z80_reset {
            self.z80_bank_bit = 0; // Hardware resets shift pointer
        }
    }

    fn write_vdp_byte(&mut self, _addr: u32, _value: u8) {
        // VDP byte writes are generally ignored or handled as word writes with duplicated bytes
        // For now, we follow the original behavior of doing nothing.
    }

    fn write_psg_byte(&mut self, value: u8) {
        self.apu.psg.write(value);
    }

    fn write_work_ram_byte(&mut self, addr: u32, value: u8) {
        let ram_addr = addr & 0xFFFF;
        self.work_ram[ram_addr as usize] = value;
    }

    /// Write a byte to the memory map
    pub fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address & 0xFFFFFF;

        match addr {
            // ROM is read-only (writes are ignored)
            ROM_START..=ROM_END => {}

            // Z80 RAM
            Z80_RAM_START..=Z80_RAM_END => self.write_z80_ram_byte(addr, value),

            // YM2612 FM Chip: 0xA04000-0xA04003
            YM2612_START..=YM2612_END => self.write_ym2612_byte(addr, value),

            // Z80 area bank registers
            Z80_BANK_REG_START..=Z80_BANK_REG_END => self.write_z80_bank_reg_byte(addr, value),

            // I/O Ports
            IO_START..=IO_END => self.write_io_byte(addr, value),

            // Z80 Bus Request
            Z80_BUS_REQ_START..=Z80_BUS_REQ_END => {
                if addr == Z80_BUS_REQ_START {
                    self.write_z80_bus_req_byte(value);
                }
            }

            // Z80 Reset
            Z80_RESET_START..=Z80_RESET_END => {
                if addr == Z80_RESET_START {
                    self.write_z80_reset_byte(value);
                }
            }

            // VDP Ports
            VDP_DATA_START..=VDP_HV_END => self.write_vdp_byte(addr, value),

            // PSG: 0xC00011
            0xC00011 => self.write_psg_byte(value),

            // Work RAM
            WORK_RAM_START..=WORK_RAM_END => self.write_work_ram_byte(addr, value),

            // Unmapped regions (writes ignored)
            _ => {}
        }
    }

    /// Sync audio generation with CPU cycles
    pub fn sync_audio(
        &mut self,
        m68k_cycles: u32,
        z80: &mut crate::z80::Z80<crate::memory::Z80Bus, crate::memory::Z80Bus>,
        z80_cycle_debt: &mut f32,
    ) {
        const Z80_CYCLES_PER_M68K_CYCLE: f32 = 3.58 / 7.67;
        let z80_can_run = !self.z80_reset && !self.z80_bus_request;

        // M68k Clock = 7,670,453 Hz
        let cycles_per_sample = 7670453.0 / (self.sample_rate as f32);

        self.audio_accumulator += m68k_cycles as f32;

        while self.audio_accumulator >= cycles_per_sample {
            self.audio_accumulator -= cycles_per_sample;

            // Catch up Z80 before generating sample
            if z80_can_run {
                *z80_cycle_debt += cycles_per_sample * Z80_CYCLES_PER_M68K_CYCLE;
                while *z80_cycle_debt >= 1.0 {
                    let z80_cycles = z80.step();
                    *z80_cycle_debt -= z80_cycles as f32;
                }
            }

            let (l, r) = self.apu.step();

            // Limit buffer size to ~20 frames
            if self.audio_buffer.len() < 32768 {
                self.audio_buffer.push(l);
                self.audio_buffer.push(r);
            }
        }
    }

    /// Read a word (16-bit, big-endian) from the memory map
    #[inline]
    pub fn read_word(&mut self, address: u32) -> u16 {
        let addr = address & 0xFFFFFF;

        // ROM Fast Path
        if addr <= ROM_END {
            let idx = addr as usize;
            if idx + 1 < self.rom.len() {
                let high = self.rom[idx];
                let low = self.rom[idx + 1];
                return byte_utils::join_u16(high, low);
            } else if idx < self.rom.len() {
                // Partial read at end of ROM
                let high = self.rom[idx];
                let low = 0xFF; // Unmapped
                return byte_utils::join_u16(high, low);
            } else {
                return 0xFFFF; // Unmapped
            }
        }

        // VDP Data Port (Word access)
        if (VDP_DATA_START..=VDP_DATA_END).contains(&addr) {
            return self.vdp.read_data();
        }
        // VDP Control Port / Status
        if (VDP_CTRL_START..=VDP_CTRL_END).contains(&addr) {
            return self.vdp.read_status();
        }
        // VDP H/V Counter
        if (VDP_HV_START..=VDP_HV_END).contains(&addr) {
            return self.vdp.read_hv_counter();
        }

        // Optimize Work RAM access (0xE00000-0xFFFFFF, 64KB mirrored)
        if addr >= WORK_RAM_START {
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

        // VDP Data Port
        if (VDP_DATA_START..=VDP_DATA_END).contains(&addr) {
            self.vdp.write_data(value);
            return;
        }
        // VDP Control Port
        if (VDP_CTRL_START..=VDP_CTRL_END).contains(&addr) {
            self.vdp.write_control(value);
            if self.vdp.dma_pending {
                if self.vdp.is_dma_transfer() {
                    self.run_dma();
                } else {
                    self.vdp.execute_dma();
                }
            }
            return;
        }

        // Optimize Work RAM access
        if addr >= WORK_RAM_START {
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
        if addr <= ROM_END {
            let idx = addr as usize;
            if idx + 3 < self.rom.len() {
                let b0 = self.rom[idx];
                let b1 = self.rom[idx + 1];
                let b2 = self.rom[idx + 2];
                let b3 = self.rom[idx + 3];
                return byte_utils::join_u32(b0, b1, b2, b3);
            }
        }

        // Optimize Work RAM access
        if addr >= WORK_RAM_START {
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

        let b0 = self.read_byte(address);
        let b1 = self.read_byte(address.wrapping_add(1));
        let b2 = self.read_byte(address.wrapping_add(2));
        let b3 = self.read_byte(address.wrapping_add(3));
        byte_utils::join_u32(b0, b1, b2, b3)
    }

    /// Write a long word (32-bit, big-endian) to the memory map
    pub fn write_long(&mut self, address: u32, value: u32) {
        let addr = address & 0xFFFFFF;

        // Optimize Work RAM access
        if addr >= WORK_RAM_START {
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
        json!({
            "z80_bus_request": self.z80_bus_request,
            "z80_reset": self.z80_reset,
            "z80_bank_addr": self.z80_bank_addr,
            "vdp": self.vdp.read_state(),
            "io": self.io.read_state(),
            "apu": self.apu.read_state(),
        })
    }

    fn write_state(&mut self, state: &Value) {
        if let Some(req) = state["z80_bus_request"].as_bool() {
            self.z80_bus_request = req;
        }
        if let Some(reset) = state["z80_reset"].as_bool() {
            self.z80_reset = reset;
        }
        if let Some(bank) = state["z80_bank_addr"].as_u64() {
            self.z80_bank_addr = bank as u32;
        }
        self.vdp.write_state(&state["vdp"]);
        self.io.write_state(&state["io"]);
        self.apu.write_state(&state["apu"]);
    }
}
