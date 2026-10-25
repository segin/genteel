//! Sega Genesis/Mega Drive Memory Bus
//!
//! This module implements the full memory map of the Genesis, routing
//! reads and writes to the appropriate components.
//!
//! ## Genesis Memory Map
//!
//! | Address Range      | Size   | Description                    |
//! |:-------------------|:-------|:-------------------------------|
//! | 0x000000-0x3FFFFF  | 4 MB   | ROM/Cartridge (SRAM, mappers)  |
//! | 0x400000-0x9FFFFF  | 6 MB   | Expansion; linear ROM > 4 MB   |
//! | 0xA00000-0xA0FFFF  | 64 KB  | Z80 Address Space              |
//! | 0xA10000-0xA1001F  | 32 B   | I/O Ports                      |
//! | 0xA11100-0xA11101  | 2 B    | Z80 Bus Request                |
//! | 0xA11200-0xA11201  | 2 B    | Z80 Reset                      |
//! | 0xA130F1-0xA130FF  | 16 B   | SRAM control / Sega mapper     |
//! | 0xA14000-0xA14003  | 4 B    | TMSS                           |
//! | 0xC00000-0xDFFFFF  | 2 MB   | VDP ports, mirrored where      |
//! |                    |        | `(addr & 0xE700E0) == 0xC00000`|
//! | 0xE00000-0xFFFFFF  | 2 MB   | Work RAM (64KB mirrored)       |

use super::byte_utils;
use super::cart::{SegaMapper, Sram, SEGA_MAPPER_THRESHOLD};
use super::everdrive::EverdriveMapper;
use super::MemoryInterface;
use crate::apu::Apu;
use crate::audio;
use crate::debugger::Debuggable;
use crate::io::{ConsoleRegion, Io};
use crate::vdp::Vdp;
use serde::{Deserialize, Deserializer, Serialize};
use serde_json::{json, Value};

/// 68k stall cycles charged for each data-port write that hits a full VDP FIFO.
/// Calibrated to roughly one FIFO drain slot in H40 (~16 MCLK / 7 MCLK-per-cycle).
const VDP_FIFO_STALL_CYCLES: u32 = 4;

/// Work RAM size (64 KiB).
pub const WORK_RAM_SIZE: usize = 0x10000;
/// Z80 sound RAM size (8 KiB).
pub const Z80_RAM_SIZE: usize = 0x2000;

/// Master clocks per 68k cycle.
const MCLK_PER_M68K_CYCLE: u32 = 7;

/// Deserialize a RAM buffer, rejecting any length other than `N` so a
/// malformed save state can't make later fixed-size indexing panic.
fn deserialize_ram<'de, D, const N: usize>(deserializer: D) -> Result<Box<[u8]>, D::Error>
where
    D: Deserializer<'de>,
{
    let ram = Box::<[u8]>::deserialize(deserializer)?;
    if ram.len() != N {
        return Err(serde::de::Error::invalid_length(
            ram.len(),
            &format!("a RAM buffer of {} bytes", N).as_str(),
        ));
    }
    Ok(ram)
}

/// True when a 68k address decodes to the VDP ports (`$C00000-$DFFFFF`
/// with A5-A7 and A16-A18 clear).
#[inline]
fn is_vdp_port(addr: u32) -> bool {
    addr & 0xE700E0 == 0xC00000
}

/// Read a byte from the cartridge window (`$000000-$3FFFFF`).
#[inline]
fn cart_read_byte(
    rom: &[u8],
    mapper: &SegaMapper,
    everdrive: &EverdriveMapper,
    sram: &Sram,
    addr: u32,
) -> u8 {
    if everdrive.enabled {
        return everdrive.read(addr);
    }
    if sram.contains(addr) {
        return sram.read_byte(addr);
    }
    rom.get(mapper.rom_offset(addr)).copied().unwrap_or(0xFF)
}

/// Read a word from the cartridge window, honouring the SSF/Sega mappers
/// and SRAM mapping.
#[inline]
fn cart_read_word(
    rom: &[u8],
    mapper: &SegaMapper,
    everdrive: &EverdriveMapper,
    sram: &Sram,
    addr: u32,
) -> u16 {
    // Fast path: plain linear ROM below any mapped SRAM.
    if !everdrive.enabled && !mapper.enabled && (!sram.mapped || addr + 1 < sram.base()) {
        let idx = addr as usize;
        if idx + 1 < rom.len() {
            return u16::from_be_bytes([rom[idx], rom[idx + 1]]);
        }
    }
    byte_utils::join_u16(
        cart_read_byte(rom, mapper, everdrive, sram, addr),
        cart_read_byte(rom, mapper, everdrive, sram, addr.wrapping_add(1)),
    )
}

/// Word fetched by a 68k->VDP DMA. The source sees the same cartridge
/// mapping as the CPU (mapper banks, mapped SRAM), plus work RAM.
#[inline]
fn dma_read_word(
    rom: &[u8],
    mapper: &SegaMapper,
    everdrive: &EverdriveMapper,
    sram: &Sram,
    work_ram: &[u8],
    addr: u32,
) -> u16 {
    let addr = addr & 0xFFFFFF;
    if addr <= 0x3FFFFF {
        cart_read_word(rom, mapper, everdrive, sram, addr)
    } else if addr <= 0x9FFFFF {
        let idx = addr as usize;
        byte_utils::join_u16(
            rom.get(idx).copied().unwrap_or(0xFF),
            rom.get(idx + 1).copied().unwrap_or(0xFF),
        )
    } else if addr >= 0xE00000 {
        let r_addr = (addr & 0xFFFF) as usize;
        byte_utils::join_u16(
            work_ram.get(r_addr).copied().unwrap_or(0xFF),
            work_ram.get((r_addr + 1) & 0xFFFF).copied().unwrap_or(0xFF),
        )
    } else {
        0xFFFF // Unmapped or unused DMA source
    }
}

/// Sega Genesis Memory Bus
///
/// Routes memory accesses to the appropriate component based on address.
#[derive(Debug, Serialize, Deserialize)]
pub struct Bus {
    /// ROM data
    #[serde(skip)]
    pub rom: Vec<u8>,

    /// EverDrive / SSF bank-switching mapper. Active only for `"SEGA SSF"` ROMs,
    /// where it exposes the `$000000-$3FFFFF` window as up to 4MB of banked RAM.
    #[serde(default)]
    pub everdrive: EverdriveMapper,

    /// Standard Sega bank mapper, active for ROMs larger than 4 MiB.
    #[serde(default)]
    pub rom_mapper: SegaMapper,

    /// Work RAM (64KB at 0xFF0000-0xFFFFFF, mirrored in 0xE00000-0xFFFFFF)
    #[serde(deserialize_with = "deserialize_ram::<_, WORK_RAM_SIZE>")]
    pub work_ram: Box<[u8]>,

    /// Z80 RAM (8KB at 0xA00000-0xA01FFF, mirrored at 0xA02000-0xA03FFF)
    #[serde(deserialize_with = "deserialize_ram::<_, Z80_RAM_SIZE>")]
    pub z80_ram: Box<[u8]>,

    /// Battery-backed cartridge RAM (serialized under its historical
    /// `sram*` keys).
    #[serde(flatten)]
    pub sram: Sram,

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

    /// The Z80 hung by accessing the Z80 window through its own bank; it
    /// stays halted until its reset line is asserted.
    #[serde(default)]
    pub z80_lockup: bool,

    /// TMSS (Trademark Security System) - lock/unlock state
    pub tmss_unlocked: bool,

    /// TMSS Register (stores the written bytes "SEGA")
    #[serde(skip)]
    pub tmss_register: [u8; 4],

    /// Audio synchronization
    pub audio_accumulator: f64,
    #[serde(skip)]
    pub audio_buffer: Vec<i16>,
    pub sample_rate: u32,

    /// 68k stall cycles owed by the VDP (e.g. for FIFO back-pressure).
    /// Drained by the CPU through `take_pending_stall_cycles`.
    #[serde(skip)]
    pub vdp_stall_cycles: u32,

    /// Region forced by configuration instead of the ROM header.
    #[serde(skip)]
    pub region_override: Option<ConsoleRegion>,

    /// Master clocks not yet converted to whole 68k cycles for I/O timing.
    #[serde(skip)]
    io_mclk_remainder: u32,
}

impl Default for Bus {
    fn default() -> Self {
        Self::new()
    }
}

impl Bus {
    /// Create a new empty bus
    pub fn new() -> Self {
        Self {
            rom: Vec::new(),
            everdrive: EverdriveMapper::new(),
            rom_mapper: SegaMapper::default(),
            work_ram: vec![0; WORK_RAM_SIZE].into_boxed_slice(),
            z80_ram: vec![0; Z80_RAM_SIZE].into_boxed_slice(),
            sram: Sram::new(),
            vdp: Vdp::new(),
            io: Io::new(),
            apu: Apu::new(),
            z80_bus_request: false,
            z80_reset: true, // Z80 starts in reset
            z80_bank_addr: 0,
            z80_lockup: false,
            tmss_unlocked: false,
            tmss_register: [0; 4],
            audio_accumulator: 0.0,
            audio_buffer: Vec::with_capacity(2048),
            sample_rate: audio::SAMPLE_RATE,
            vdp_stall_cycles: 0,
            region_override: None,
            io_mclk_remainder: 0,
        }
    }

    /// Install ROM bytes and activate the mapper the image needs. Shared by
    /// a fresh load and a save-state restore.
    fn install_rom(&mut self, data: &[u8]) {
        self.rom = data.to_vec();
        // Pad ROM to at least 512 bytes to ensure vector table exists
        if self.rom.len() < 512 {
            self.rom.resize(512, 0);
        }

        // EverDrive / SSF mapper: ROMs whose header system field is "SEGA SSF"
        // expose the cartridge window as banked, writable RAM.
        if EverdriveMapper::rom_uses_mapper(&self.rom) {
            self.everdrive.load_rom(&self.rom);
        } else {
            self.everdrive.disable();
        }
        self.rom_mapper.enabled = !self.everdrive.enabled && self.rom.len() > SEGA_MAPPER_THRESHOLD;
    }

    /// Load a ROM into the bus, configuring SRAM and the console region
    /// from its header.
    pub fn load_rom(&mut self, data: &[u8]) {
        self.install_rom(data);
        self.rom_mapper.reset();
        self.sram = Sram::configure_from_rom(&self.rom);
        self.apply_region();
    }

    /// Re-attach ROM bytes after a save state has been deserialized into
    /// this bus. Only state that is not serialized (the ROM image and mapper
    /// activation) is rebuilt; SRAM contents and mapping, mapper banks and
    /// the region come from the state.
    pub fn restore_rom(&mut self, data: &[u8]) {
        self.install_rom(data);
    }

    /// Force the console region (or return to header detection with `None`)
    /// and apply it to the version register and VDP timing.
    pub fn set_region_override(&mut self, region: Option<ConsoleRegion>) {
        self.region_override = region;
        self.apply_region();
    }

    /// The region in effect: the configured override, else the ROM header.
    pub fn region(&self) -> ConsoleRegion {
        self.region_override
            .unwrap_or_else(|| ConsoleRegion::from_header(&self.rom))
    }

    fn apply_region(&mut self) {
        let region = self.region();
        self.io.set_region(region);
        self.vdp.set_pal(region.is_pal());
    }

    /// Clear the ROM
    pub fn clear_rom(&mut self) {
        self.rom.clear();
    }

    /// Get ROM size
    pub fn rom_size(&self) -> usize {
        self.rom.len()
    }

    /// Check if DMA is currently active
    pub fn dma_active(&self) -> bool {
        use crate::vdp::DmaOps;
        self.vdp.command.dma_pending && self.vdp.is_dma_transfer()
    }

    /// True while the Z80 is free to execute: out of reset, not halted by a
    /// 68k bus request, and not hung by an illegal bank access.
    pub fn z80_can_run(&self) -> bool {
        !self.z80_reset && !self.z80_bus_request && !self.z80_lockup
    }

    /// Reset volatile state while keeping ROM, SRAM contents and sample_rate
    pub fn reset(&mut self) {
        self.work_ram.fill(0);
        self.z80_ram.fill(0);
        // SRAM is battery backed: keep its contents, re-derive its mapping.
        self.sram.reset();
        self.rom_mapper.reset();
        // Return the SSF mapper to its power-on bank mapping. The backing memory
        // (which holds the ROM image) persists across a soft reset, like PSRAM.
        self.everdrive.reset_registers();

        self.vdp.reset();
        self.vdp.vram.fill(0);
        self.vdp.cram.fill(0);
        self.vdp.vsram.fill(0);
        self.vdp.reconstruct_cram_cache();

        self.io.reset();
        self.apu.reset();

        self.z80_bus_request = false;
        self.z80_reset = true;
        self.z80_bank_addr = 0;
        self.z80_lockup = false;
        self.tmss_unlocked = false;
        self.tmss_register = [0; 4];
        self.audio_accumulator = 0.0;
        self.audio_buffer.clear();
    }

    #[inline]
    fn read_cart_byte(&self, addr: u32) -> u8 {
        cart_read_byte(
            &self.rom,
            &self.rom_mapper,
            &self.everdrive,
            &self.sram,
            addr,
        )
    }

    #[inline]
    fn read_cart_word(&self, addr: u32) -> u16 {
        cart_read_word(
            &self.rom,
            &self.rom_mapper,
            &self.everdrive,
            &self.sram,
            addr,
        )
    }

    /// Read a byte from the memory map
    pub fn read_byte(&mut self, address: u32) -> u8 {
        let addr = address & 0xFFFFFF; // 24-bit address bus

        match addr {
            0x000000..=0x3FFFFF => self.read_cart_byte(addr),
            // Linear ROM beyond 4 MiB (open bus for smaller images).
            0x400000..=0x9FFFFF => self.rom.get(addr as usize).copied().unwrap_or(0xFF),
            0xA00000..=0xA0FFFF => self.read_z80_area(addr),
            0xA10000..=0xA1FFFF => self.read_io_area(addr),
            0xC00000..=0xDFFFFF => self.read_vdp_byte(addr),
            0xE00000..=0xFFFFFF => self.work_ram[(addr & 0xFFFF) as usize],
            _ => 0xFF,
        }
    }

    /// Write a byte to the memory map
    pub fn write_byte(&mut self, address: u32, value: u8) {
        let addr = address & 0xFFFFFF;

        match addr {
            0x000000..=0x3FFFFF => {
                if self.everdrive.enabled {
                    self.everdrive.write(addr, value);
                } else if self.sram.contains(addr) {
                    self.sram.write_byte(addr, value);
                }
            }
            0xA00000..=0xA0FFFF => self.write_z80_area(addr, value),
            0xA10000..=0xA1FFFF => {
                if self.everdrive.enabled && EverdriveMapper::is_register(addr) {
                    // SSF mapper control/bank registers ($A130F0-$A130FF).
                    self.everdrive.write_register(addr, value);
                } else if addr == 0xA130F1 {
                    self.sram.write_control(value);
                } else if self.rom_mapper.enabled && SegaMapper::is_bank_register(addr) {
                    self.rom_mapper.write_register(addr, value);
                } else {
                    self.write_io_area(addr, value);
                }
            }
            0xC00000..=0xDFFFFF => self.write_vdp_byte(addr, value),
            0xE00000..=0xFFFFFF => self.work_ram[(addr & 0xFFFF) as usize] = value,
            _ => {}
        }
    }

    /// 68k byte read in the Z80 window. Z80 RAM (8 KiB, mirrored over
    /// `$A00000-$A03FFF`) needs the Z80 bus; the bank register and the VDP
    /// window at `$A07F00` are not readable.
    fn read_z80_area(&mut self, addr: u32) -> u8 {
        match (addr >> 13) & 3 {
            0 | 1 => {
                if self.z80_bus_request {
                    self.z80_ram[(addr & 0x1FFF) as usize]
                } else {
                    0xFF
                }
            }
            // YM2612, mirrored across A04000h-A05FFFh
            2 => self.apu.fm.read((addr & 3) as u8),
            _ => 0xFF,
        }
    }

    fn read_io_area(&mut self, addr: u32) -> u8 {
        match addr {
            0xA10000..=0xA1001F => self.io.read(addr),
            // BUSACK: 0 only once the Z80 has released the bus, which needs
            // the request asserted and the Z80 out of reset.
            0xA11100..=0xA11101 => {
                if self.z80_bus_request && !self.z80_reset {
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
            0xA14000..=0xA14003 => {
                if self.tmss_unlocked {
                    0x01
                } else {
                    0x00
                }
            }
            _ => 0xFF,
        }
    }

    fn read_vdp_byte(&mut self, addr: u32) -> u8 {
        if !is_vdp_port(addr) {
            return 0xFF;
        }
        let word = match addr & 0x1F {
            0x00..=0x03 => self.vdp.read_data(),
            0x04..=0x07 => self.vdp.read_status(),
            0x08..=0x0F => self.vdp.read_hv_counter(),
            _ => return 0xFF,
        };
        if addr & 1 == 0 {
            (word >> 8) as u8
        } else {
            word as u8
        }
    }

    fn read_vdp_word(&mut self, addr: u32) -> u16 {
        if !is_vdp_port(addr) {
            return 0xFFFF;
        }
        match addr & 0x1F {
            0x00..=0x03 => self.vdp.read_data(),
            0x04..=0x07 => self.vdp.read_status(),
            0x08..=0x0F => self.vdp.read_hv_counter(),
            _ => 0xFFFF,
        }
    }

    fn write_z80_area(&mut self, addr: u32, value: u8) {
        match (addr >> 13) & 3 {
            0 | 1 => {
                if self.z80_bus_request {
                    self.z80_ram[(addr & 0x1FFF) as usize] = value;
                }
            }
            // YM2612, mirrored across A04000h-A05FFFh
            2 => {
                let port = ((addr & 2) >> 1) as u8;
                if addr & 1 != 0 {
                    self.apu.fm.write_data(port, value);
                } else {
                    self.apu.fm.write_address(port, value);
                }
            }
            _ => {
                if (addr >> 8) & 0x7F == 0x60 {
                    let current_reg = self.z80_bank_addr >> 15;
                    let new_reg = (current_reg >> 1) | ((value as u32 & 1) << 8);
                    // Clear the top bits and assign to the 9-bit register (max 0x1FF)
                    self.z80_bank_addr = (new_reg & 0x1FF) << 15;
                }
            }
        }
    }

    fn write_io_area(&mut self, addr: u32, value: u8) {
        match addr {
            // Pad data/control writes never latch the HV counter: only a
            // light gun's TH input does (Genesis Plus GX `lightgun.c`).
            0xA10000..=0xA1001F => self.io.write(addr, value),
            0xA11100 => self.z80_bus_request = (value & 0x01) != 0,
            0xA11200 => {
                let was_reset = self.z80_reset;
                self.z80_reset = (value & 0x01) == 0;
                if self.z80_reset {
                    self.z80_bank_addr = 0;
                    self.z80_lockup = false;
                }
                /* ZRES also drives the YM2612 /IC line: the chip is reset
                when reset is asserted and again when it is released
                (Genesis Plus GX `gen_zreset_w`). */
                if self.z80_reset || was_reset {
                    self.apu.fm.reset();
                }
            }
            0xA14000..=0xA14003 => {
                // TMSS Unlock: Write 'SEGA' to $A14000
                let idx = (addr & 3) as usize;
                self.tmss_register[idx] = value;
                if self.tmss_register == *b"SEGA" {
                    self.tmss_unlocked = true;
                }
            }
            _ => {}
        }
    }

    /// Data-port write, charging the 68k a stall when the FIFO is full.
    #[inline]
    fn vdp_data_write(&mut self, value: u16) {
        if self.vdp.fifo_full {
            // FIFO back-pressure: the 68k stalls until a slot drains.
            self.vdp_stall_cycles = self.vdp_stall_cycles.saturating_add(VDP_FIFO_STALL_CYCLES);
        }
        self.vdp.write_data(value);
    }

    fn write_vdp_byte(&mut self, addr: u32, value: u8) {
        if !is_vdp_port(addr) {
            return;
        }
        // Byte writes to the data/control ports duplicate the byte.
        let val16 = u16::from_be_bytes([value, value]);
        match addr & 0x1F {
            0x00..=0x03 => self.vdp_data_write(val16),
            0x04..=0x07 => self.vdp.write_control(val16),
            // The PSG answers on the odd bytes of $C00010-$C00017.
            0x10..=0x17 if addr & 1 == 1 => self.apu.psg.write(value),
            _ => {}
        }
    }

    fn write_vdp_word(&mut self, addr: u32, value: u16) {
        if !is_vdp_port(addr) {
            return;
        }
        match addr & 0x1F {
            0x00..=0x03 => self.vdp_data_write(value),
            0x04..=0x07 => self.vdp.write_control(value),
            0x10..=0x17 => self.apu.psg.write(value as u8),
            _ => {}
        }
    }

    /// Read a word (16-bit, big-endian) from the memory map
    #[inline]
    pub fn read_word(&mut self, address: u32) -> u16 {
        let addr = address & 0xFFFFFF;

        if addr <= 0x3FFFFF {
            return self.read_cart_word(addr);
        }

        // Work RAM (0xE00000-0xFFFFFF, 64KB mirrored)
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr < 0xFFFF {
                return u16::from_be_bytes([self.work_ram[r_addr], self.work_ram[r_addr + 1]]);
            }
        }

        match addr {
            // The Z80 window is 8 bits wide: a word read returns the
            // addressed byte in both halves.
            0xA00000..=0xA0FFFF => {
                let b = self.read_z80_area(addr);
                u16::from_be_bytes([b, b])
            }
            0xC00000..=0xDFFFFF => self.read_vdp_word(addr),
            _ => byte_utils::join_u16(
                self.read_byte(address),
                self.read_byte(address.wrapping_add(1)),
            ),
        }
    }

    /// Write a word (16-bit, big-endian) to the memory map
    pub fn write_word(&mut self, address: u32, value: u16) {
        let addr = address & 0xFFFFFF;

        // Work RAM
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr < 0xFFFF {
                let (high, low) = byte_utils::split_u16(value);
                self.work_ram[r_addr] = high;
                self.work_ram[r_addr + 1] = low;
                return;
            }
        }

        match addr {
            // The Z80 window is 8 bits wide: only the high byte is written,
            // to the addressed location.
            0xA00000..=0xA0FFFF => self.write_z80_area(addr, (value >> 8) as u8),
            0xC00000..=0xDFFFFF => self.write_vdp_word(addr, value),
            _ => {
                let (high, low) = byte_utils::split_u16(value);
                self.write_byte(address, high);
                self.write_byte(address.wrapping_add(1), low);
            }
        }
    }

    /// Read a long word (32-bit, big-endian) from the memory map. Outside
    /// the ROM and work RAM fast paths it is two word accesses, as on the
    /// 68000's 16-bit bus.
    #[inline]
    pub fn read_long(&mut self, address: u32) -> u32 {
        let addr = address & 0xFFFFFF;

        // ROM fast path (plain linear ROM below any mapped SRAM)
        if addr <= 0x3FFFFF
            && !self.everdrive.enabled
            && !self.rom_mapper.enabled
            && (!self.sram.mapped || addr + 3 < self.sram.base())
        {
            let idx = addr as usize;
            if idx + 3 < self.rom.len() {
                return u32::from_be_bytes([
                    self.rom[idx],
                    self.rom[idx + 1],
                    self.rom[idx + 2],
                    self.rom[idx + 3],
                ]);
            }
        }

        // Work RAM
        if addr >= 0xE00000 {
            let r_addr = (addr & 0xFFFF) as usize;
            if r_addr <= 0xFFFC {
                return u32::from_be_bytes([
                    self.work_ram[r_addr],
                    self.work_ram[r_addr + 1],
                    self.work_ram[r_addr + 2],
                    self.work_ram[r_addr + 3],
                ]);
            }
        }

        let high = self.read_word(address);
        let low = self.read_word(address.wrapping_add(2));
        byte_utils::join_u32_words(high, low)
    }

    /// Write a long word (32-bit, big-endian) to the memory map, as two
    /// word accesses outside work RAM.
    pub fn write_long(&mut self, address: u32, value: u32) {
        let addr = address & 0xFFFFFF;

        // Work RAM
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

        let (high, low) = byte_utils::split_u32_to_words(value);
        self.write_word(address, high);
        self.write_word(address.wrapping_add(2), low);
    }

    /// The word a VDP 68k->VDP DMA would fetch from `addr`.
    pub fn dma_source_word(&self, addr: u32) -> u16 {
        dma_read_word(
            &self.rom,
            &self.rom_mapper,
            &self.everdrive,
            &self.sram,
            &self.work_ram,
            addr,
        )
    }

    /// Advance system state by N MCLK cycles.
    pub fn tick(&mut self, mclk: u32) {
        let rom = &self.rom;
        let mapper = &self.rom_mapper;
        let everdrive = &self.everdrive;
        let sram = &self.sram;
        let work_ram = &self.work_ram;

        self.vdp.tick(mclk, |addr| {
            dma_read_word(rom, mapper, everdrive, sram, work_ram, addr)
        });

        // I/O timing (6-button pad timeout) runs on whole 68k cycles.
        let total = self.io_mclk_remainder + mclk;
        self.io.tick(total / MCLK_PER_M68K_CYCLE);
        self.io_mclk_remainder = total % MCLK_PER_M68K_CYCLE;
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
    #[inline]
    fn take_pending_stall_cycles(&mut self) -> u32 {
        let cycles = self
            .vdp_stall_cycles
            .saturating_add(self.vdp.take_dma_stall_cycles());
        self.vdp_stall_cycles = 0;
        cycles
    }
    /// The Mega Drive bus ignores the write cycle of TAS (games such as
    /// Gargoyles and Ex-Mutants depend on the byte staying unchanged).
    #[inline]
    fn tas_write_back(&self) -> bool {
        false
    }
}

impl Debuggable for Bus {
    fn read_state(&self) -> Value {
        json!({
            "z80_bus_request": self.z80_bus_request,
            "z80_reset": self.z80_reset,
            "z80_bank_addr": self.z80_bank_addr,
            "tmss_unlocked": self.tmss_unlocked,
            "audio_accumulator": self.audio_accumulator,
            "sample_rate": self.sample_rate,
            "work_ram": self.work_ram,
            "z80_ram": self.z80_ram,
            "vdp": self.vdp.read_state(),
            "io": self.io.read_state(),
            "apu": self.apu.read_state(),
        })
    }

    fn write_state(&mut self, state: &Value) {
        // Reject the whole state if a RAM image has the wrong size: the
        // buffers are indexed with fixed masks.
        let ram_len_ok = |key: &str, len: usize| {
            state
                .get(key)
                .is_none_or(|v| v.as_array().is_some_and(|a| a.len() == len))
        };
        if !ram_len_ok("work_ram", WORK_RAM_SIZE) || !ram_len_ok("z80_ram", Z80_RAM_SIZE) {
            log::error!("Rejected bus state: work_ram or z80_ram has the wrong size");
            return;
        }

        if let Some(val) = state.get("z80_bus_request") {
            if let Some(b) = val.as_bool() {
                self.z80_bus_request = b;
            }
        }
        if let Some(val) = state.get("z80_reset") {
            if let Some(b) = val.as_bool() {
                self.z80_reset = b;
            }
        }
        if let Some(val) = state.get("z80_bank_addr") {
            if let Some(u) = val.as_u64() {
                self.z80_bank_addr = u as u32;
            }
        }
        if let Some(val) = state.get("tmss_unlocked") {
            if let Some(b) = val.as_bool() {
                self.tmss_unlocked = b;
                // Sync register state
                if b {
                    self.tmss_register = *b"SEGA";
                } else {
                    self.tmss_register = [0; 4];
                }
            }
        }
        if let Some(val) = state.get("audio_accumulator") {
            if let Some(f) = val.as_f64() {
                self.audio_accumulator = f;
            }
        }
        if let Some(val) = state.get("sample_rate") {
            if let Some(u) = val.as_u64() {
                self.sample_rate = u as u32;
            }
        }

        if let Some(val) = state.get("work_ram") {
            if let Ok(ram) = Box::<[u8]>::deserialize(val) {
                self.work_ram = ram;
            }
        }
        if let Some(val) = state.get("z80_ram") {
            if let Ok(ram) = Box::<[u8]>::deserialize(val) {
                self.z80_ram = ram;
            }
        }

        if let Some(val) = state.get("vdp") {
            self.vdp.write_state(val);
        }
        if let Some(val) = state.get("io") {
            self.io.write_state(val);
        }
        if let Some(val) = state.get("apu") {
            self.apu.write_state(val);
        }

        // Clear audio buffer as it's transient
        self.audio_buffer.clear();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Writing to the VDP data port while the FIFO is full charges 68k stall
    /// cycles via `take_pending_stall_cycles`.
    #[test]
    fn vdp_fifo_full_charges_stall_cycles() {
        let mut bus = Bus::new();
        // Configure the VDP so writes target VRAM.
        bus.vdp.command.code = crate::vdp::VRAM_WRITE;
        bus.vdp.command.address = 0;
        // Pretend the FIFO is already full.
        bus.vdp.fifo_full = true;
        // Push three writes through the byte path.
        bus.write_byte(0xC00000, 0x12);
        bus.write_byte(0xC00000, 0x34);
        bus.write_byte(0xC00000, 0x56);

        let stall = bus.take_pending_stall_cycles();
        assert_eq!(
            stall,
            3 * VDP_FIFO_STALL_CYCLES,
            "FIFO-full writes should accumulate stall cycles"
        );
        // Second drain should return 0.
        assert_eq!(bus.take_pending_stall_cycles(), 0);
    }

    /// Word-path writes follow the same stall path.
    #[test]
    fn vdp_fifo_full_word_write_charges_stall() {
        let mut bus = Bus::new();
        bus.vdp.command.code = crate::vdp::VRAM_WRITE;
        bus.vdp.command.address = 0;
        bus.vdp.fifo_full = true;
        bus.write_word(0xC00000, 0xABCD);
        assert_eq!(bus.take_pending_stall_cycles(), VDP_FIFO_STALL_CYCLES);
    }

    /// Writes that don't hit a full FIFO must not charge stalls.
    #[test]
    fn vdp_empty_fifo_no_stall() {
        let mut bus = Bus::new();
        bus.vdp.command.code = crate::vdp::VRAM_WRITE;
        bus.vdp.command.address = 0;
        // FIFO empty by default.
        bus.write_word(0xC00000, 0xABCD);
        assert_eq!(bus.take_pending_stall_cycles(), 0);
    }

    #[test]
    fn test_tmss_unlock_byte_writes() {
        let mut bus = Bus::new();
        assert!(!bus.tmss_unlocked);

        // Write 'S', 'E', 'G', 'A' byte by byte
        bus.write_byte(0xA14000, b'S');
        assert!(!bus.tmss_unlocked);

        bus.write_byte(0xA14001, b'E');
        assert!(!bus.tmss_unlocked);

        bus.write_byte(0xA14002, b'G');
        assert!(!bus.tmss_unlocked);

        bus.write_byte(0xA14003, b'A');
        // This should unlock the TMSS
        assert!(bus.tmss_unlocked);
    }

    #[test]
    fn test_tmss_unlock_long_write() {
        let mut bus = Bus::new();
        assert!(!bus.tmss_unlocked);

        // Write 'SEGA' as a long
        bus.write_long(0xA14000, 0x53454741); // "SEGA"
        assert!(bus.tmss_unlocked);
    }

    #[test]
    fn test_bus_debuggable_roundtrip() {
        std::thread::Builder::new()
            .stack_size(16 * 1024 * 1024) // 16MB
            .spawn(|| {
                let mut bus = Bus::new();

                // Modify scalar state
                bus.z80_bus_request = true;
                bus.z80_reset = false;
                bus.z80_bank_addr = 0x12345;
                bus.tmss_unlocked = true;
                bus.audio_accumulator = 1.234;
                bus.sample_rate = 48000;

                // Modify RAM
                bus.work_ram[0] = 0xAA;
                bus.work_ram[0xFFFF] = 0xBB;
                bus.z80_ram[0] = 0xCC;
                bus.z80_ram[0x1FFF] = 0xDD;

                // Modify sub-components
                bus.vdp.set_v_counter(100);
                if let Some(ctrl) = bus.io.controller(1) {
                    ctrl.a = true;
                }
                bus.apu.write_psg(0x90); // Set volume (test indirect effect)

                // Serialize
                let state_value = bus.read_state();

                // Verify structure
                assert!(state_value.get("vdp").is_some());
                assert!(state_value.get("io").is_some());
                assert!(state_value.get("apu").is_some());
                assert!(state_value.get("work_ram").is_some());

                // Create new bus
                let mut new_bus = Bus::new();

                // Deserialize
                new_bus.write_state(&state_value);

                // Assert scalar equality
                assert!(new_bus.z80_bus_request);
                assert!(!new_bus.z80_reset);
                assert_eq!(new_bus.z80_bank_addr, 0x12345);
                assert!(new_bus.tmss_unlocked);
                assert!((new_bus.audio_accumulator - 1.234).abs() < 1e-12);
                assert_eq!(new_bus.sample_rate, 48000);

                // Assert RAM equality
                assert_eq!(new_bus.work_ram[0], 0xAA);
                assert_eq!(new_bus.work_ram[0xFFFF], 0xBB);
                assert_eq!(new_bus.z80_ram[0], 0xCC);
                assert_eq!(new_bus.z80_ram[0x1FFF], 0xDD);

                // Assert sub-components
                assert_eq!(new_bus.vdp.read_hv_counter() >> 8, 100); // V counter
                assert!(new_bus.io.port1.state.a);
                assert_eq!(new_bus.apu.psg.tones[0].volume, 0); // 0x90 sets vol to 0
            })
            .unwrap()
            .join()
            .unwrap();
    }

    #[test]
    fn test_clear_rom() {
        let mut bus = Bus::new();
        assert_eq!(bus.rom_size(), 0);

        // Load dummy ROM
        let data = vec![0x4E, 0x71, 0x00, 0x00]; // NOP
        bus.load_rom(&data);

        // Bus::load_rom pads to at least 512 bytes
        assert!(bus.rom_size() >= 512);
        assert_eq!(bus.rom[0], 0x4E);

        // Clear ROM
        bus.clear_rom();
        assert_eq!(bus.rom_size(), 0);

        // Clear again (idempotency/safety check)
        bus.clear_rom();
        assert_eq!(bus.rom_size(), 0);
    }
}
