//! Sega Genesis Video Display Processor (VDP)
//!
//! The VDP is responsible for generating the video output. It contains:
//! - 64KB of VRAM for tile patterns and nametables
//! - 128 bytes of CRAM for the color palette (64 colors)
//! - 80 bytes of VSRAM for vertical scroll values
//! - 24 internal registers for configuration
//!
//! ## VDP Ports (Memory-Mapped)
//!
//! | Address          | Description                    |
//! |:-----------------|:-------------------------------|
//! | 0xC00000-0xC00003| Data Port (read/write VRAM)    |
//! | 0xC00004-0xC00007| Control Port (commands/status) |
//! | 0xC00008-0xC0000F| H/V Counter                    |
use crate::debugger::Debuggable;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod constants;
pub use constants::*;

pub mod dma;
pub use dma::DmaOps;

pub mod render;
pub use render::RenderOps;

fn default_cram_cache() -> [u16; 64] {
    [0; 64]
}

/// Video Display Processor (VDP)
#[derive(Debug, Serialize, Deserialize)]
pub struct Vdp {
    /// Video RAM (64KB) - stores tile patterns and nametables
    pub vram: Box<[u8]>,

    /// Color RAM (128 bytes) - 64 colors, 2 bytes each (9-bit color)
    pub cram: Box<[u8]>,

    /// Cached RGB565 colors for faster lookup
    #[serde(skip, default = "default_cram_cache")]
    pub cram_cache: [u16; 64],

    /// Vertical Scroll RAM (80 bytes) - 40 columns × 2 bytes
    pub vsram: Box<[u8]>,

    /// VDP Registers (24 registers, but only first 24 are meaningful)
    pub registers: [u8; NUM_REGISTERS],

    /// Control port state
    pub control_pending: bool,
    pub control_code: u8,
    pub control_address: u16,

    /// DMA state
    pub dma_pending: bool,

    /// Status register
    pub(crate) status: u16,

    /// Horizontal and vertical counters
    pub(crate) h_counter: u16,
    pub(crate) v_counter: u16,

    /// Internal line counter for HINT
    pub line_counter: u8,

    /// Last data value written (for VRAM fill DMA)
    pub last_data_write: u16,

    /// V30 offset for NTSC rolling effect
    pub v30_offset: u16,
    pub is_pal: bool,

    /// Framebuffer (320x240 RGB565)
    #[serde(skip)]
    pub framebuffer: Vec<u16>,
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Vdp {
    pub fn new() -> Self {
        Vdp {
            vram: vec![0; 0x10000].into_boxed_slice(),
            cram: vec![0; 128].into_boxed_slice(),
            cram_cache: [0; 64],
            vsram: vec![0; 80].into_boxed_slice(),
            registers: [0; NUM_REGISTERS],
            control_pending: false,
            control_code: 0,
            control_address: 0,
            dma_pending: false,
            status: 0x3600, // FIFO empty
            h_counter: 0,
            v_counter: 0,
            line_counter: 0,
            last_data_write: 0,
            v30_offset: 0,
            is_pal: false,
            framebuffer: vec![0; 320 * 240],
        }
    }

    /// Reconstruct cram_cache from cram
    pub fn reconstruct_cram_cache(&mut self) {
        for i in 0..64 {
            let addr = i * 2;
            if addr + 1 < self.cram.len() {
                let val = ((self.cram[addr + 1] as u16) << 8) | (self.cram[addr] as u16);

                // Extract 3-bit components (bits 1-3, 5-7, 9-11)
                let r3 = (val >> 1) & 0x07;
                let g3 = (val >> 5) & 0x07;
                let b3 = (val >> 9) & 0x07;

                // Scale to RGB565 using bit repetition
                let r5 = (r3 << 2) | (r3 >> 1);
                let g6 = (g3 << 3) | g3;
                let b5 = (b3 << 2) | (b3 >> 1);

                self.cram_cache[i] = (r5 << 11) | (g6 << 5) | b5;
            }
        }
    }

    /// Set the region (PAL=true, NTSC=false)
    pub fn set_region(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    pub fn write_data_bulk(&mut self, data: &[u8]) {
        self.control_pending = false;

        // Optimized VRAM write for standard increment
        if (self.control_code & 0x0F) == VRAM_WRITE && self.auto_increment() == 2 {
            let mut addr = self.control_address as usize;
            for chunk in data.chunks_exact(2) {
                if addr < 0x10000 {
                    // Optimization: When auto-increment is 2, address parity is preserved.
                    // This allows direct writing of big-endian chunks: chunk[0] -> even addr, chunk[1] -> odd addr.
                    self.vram[addr] = chunk[0];
                    self.vram[addr ^ 1] = chunk[1];
                }
                addr = (addr + 2) & 0xFFFF;
            }
            self.control_address = addr as u16;

            // Update last_data_write
            if data.len() >= 2 {
                let last_idx = data.len() - 2;
                self.last_data_write = ((data[last_idx] as u16) << 8) | (data[last_idx + 1] as u16);
            }
            return;
        }

        // Fallback
        for chunk in data.chunks_exact(2) {
            let val = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
            self.write_data(val);
        }
    }

    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        // DMA Fill (Mode 2) check
        if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
            && (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
            && self.dma_pending
        {
            let length = self.dma_length();
            let mut addr = self.control_address;
            let inc = self.auto_increment() as u16;
            let fill_byte = (value >> 8) as u8;

            // DMA Fill writes bytes. Length register specifies number of bytes.
            // If length is 0, it is treated as 0x10000 (64KB).
            let len = if length == 0 { 0x10000 } else { length };

            for _ in 0..len {
                // VRAM is byte-addressable in this emulator
                self.vram[addr as usize] = fill_byte;
                addr = addr.wrapping_add(inc);
            }
            self.control_address = addr;
            self.dma_pending = false;
            return;
        }

        match self.control_code & 0x0F {
            VRAM_WRITE => {
                // Write VRAM
                let addr = self.control_address as usize;
                if addr < 0x10000 {
                    self.vram[addr] = (value >> 8) as u8;
                    self.vram[addr ^ 1] = (value & 0xFF) as u8;
                }
            }
            CRAM_WRITE => {
                // Write CRAM
                let mut val = value;
                if (self.control_address & 0x01) != 0 {
                    val = val.rotate_left(8);
                }
                let addr = (self.control_address & 0x7E) as usize;

                self.cram_cache[addr >> 1] = Self::genesis_color_to_rgb565(val);

                self.cram[addr] = (val & 0xFF) as u8;
                self.cram[addr + 1] = (val >> 8) as u8;
            }
            VSRAM_WRITE => {
                // Write VSRAM
                let addr = (self.control_address & 0x7F) as usize;
                if addr < 80 {
                    self.vsram[addr] = (value >> 8) as u8;
                    self.vsram[addr + 1] = (value & 0xFF) as u8;
                }
            }
            _ => {}
        }

        self.control_address = self
            .control_address
            .wrapping_add(self.auto_increment() as u16);
    }

    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;
        match self.control_code & 0x0F {
            VRAM_READ => {
                // Read VRAM
                let addr = self.control_address as usize;
                let val = if addr < 0x10000 {
                    ((self.vram[addr] as u16) << 8) | (self.vram[addr ^ 1] as u16)
                } else {
                    0
                };
                self.control_address = self
                    .control_address
                    .wrapping_add(self.auto_increment() as u16);
                val
            }
            CRAM_READ => {
                // Read CRAM
                let addr = (self.control_address & 0x7F) as usize;
                ((self.cram[addr + 1] as u16) << 8) | (self.cram[addr] as u16)
            }
            VSRAM_READ => {
                // Read VSRAM
                let addr = (self.control_address & 0x7F) as usize;
                if addr < 80 {
                    ((self.vsram[addr] as u16) << 8) | (self.vsram[addr + 1] as u16)
                } else {
                    0
                }
            }
            _ => 0,
        }
    }

    pub fn write_control(&mut self, value: u16) {
        if self.control_pending {
            // Second word of command
            // CD5-CD2 are bits 7-4 of the second word.
            // When combined with CD1-CD0 from the first word, we get the 6-bit code.
            let high = ((value >> 4) & 0x0F) << 2;
            self.control_code = (self.control_code & 0x03) | high as u8;
            self.control_address = (self.control_address & 0x3FFF) | ((value & 0x03) << 14);
            self.control_pending = false;

            // DMA initiation check
            self.dma_pending = (self.control_code & 0x20) != 0
                && (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0;
        } else if (value & 0xC000) == 0x8000 {
            // Register write
            let reg = ((value >> 8) & 0x1F) as usize;
            let val = (value & 0xFF) as u8;
            if reg < NUM_REGISTERS {
                self.registers[reg] = val;
            }
        } else {
            // First word of command
            // CD1-CD0 are bits 15-14 of the first word.
            self.control_code = (value >> 14) as u8 & 0x03;
            self.control_address = value & 0x3FFF;
            self.control_pending = true;
        }
    }

    #[inline(always)]
    pub fn read_status(&mut self) -> u16 {
        // Reading the status register clears the write pending flag
        self.control_pending = false;
        let res = self.status;
        // Reading status clears the VInt pending bit (Bit 7)
        self.status &= !STATUS_VINT_PENDING;
        res
    }

    /// Read status without side effects (for debugging)
    pub fn peek_status(&self) -> u16 {
        self.status
    }

    /// Reset VDP state
    pub fn reset(&mut self) {
        *self = Self::new();
    }

    // Helper methods
    pub(crate) fn genesis_color_to_rgb565(val: u16) -> u16 {
        // Extract 3-bit components (bits 1-3, 5-7, 9-11)
        let r3 = (val >> 1) & 0x07;
        let g3 = (val >> 5) & 0x07;
        let b3 = (val >> 9) & 0x07;

        // Scale to RGB565 using bit repetition
        let r5 = (r3 << 2) | (r3 >> 1);
        let g6 = (g3 << 3) | g3;
        let b5 = (b3 << 2) | (b3 >> 1);

        (r5 << 11) | (g6 << 5) | b5
    }

    fn auto_increment(&self) -> u8 {
        self.registers[REG_AUTO_INC]
    }

    pub fn mode1(&self) -> u8 {
        self.registers[REG_MODE1]
    }

    pub fn mode2(&self) -> u8 {
        self.registers[REG_MODE2]
    }

    pub fn h40_mode(&self) -> bool {
        (self.registers[REG_MODE4] & MODE4_H40_MODE) == MODE4_H40_MODE
    }

    pub fn screen_width(&self) -> u16 {
        if self.h40_mode() {
            320
        } else {
            256
        }
    }

    pub fn screen_height(&self) -> u16 {
        if self.v30_mode() {
            240
        } else {
            224
        }
    }

    pub fn v30_mode(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_V30_MODE) != 0
    }

    pub fn display_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DISPLAY_ENABLE) != 0
    }

    pub fn dma_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
    }

    pub fn sprite_table_address(&self) -> u16 {
        ((self.registers[REG_SPRITE_TABLE] as u16) << 9) & 0xFE00
    }

    pub fn plane_a_address(&self) -> usize {
        ((self.registers[REG_PLANE_A] as usize) & 0x38) << 10
    }

    pub fn plane_b_address(&self) -> usize {
        ((self.registers[REG_PLANE_B] as usize) & 0x07) << 13
    }

    pub fn hscroll_address(&self) -> usize {
        ((self.registers[REG_HSCROLL] as usize) & 0x3F) << 10
    }

    pub fn plane_size(&self) -> (usize, usize) {
        let w = match self.registers[REG_PLANE_SIZE] & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128,
        };
        let h = match (self.registers[REG_PLANE_SIZE] >> 4) & 0x03 {
            0 => 32,
            1 => 64,
            _ => 128,
        };
        (w, h)
    }

    fn bg_color(&self) -> (u8, u8) {
        let bg_idx = self.registers[REG_BG_COLOR];
        let pal = (bg_idx >> 4) & 0x03;
        let color = bg_idx & 0x0F;
        (pal, color)
    }

    #[inline(always)]
    fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let addr = ((palette as usize) * 16) + (index as usize);
        unsafe { *self.cram_cache.get_unchecked(addr & 0x3F) }
    }

    // VDP State management

    pub fn step(&mut self, _cycles: u64) {}

    pub fn vblank_pending(&self) -> bool {
        (self.status & STATUS_VINT_PENDING) != 0
            && (self.registers[REG_MODE2] & MODE2_VINT_ENABLE) != 0
    }

    pub fn set_vblank(&mut self, active: bool) {
        if active {
            self.status |= STATUS_VBLANK;
            self.status |= STATUS_VINT_PENDING;
        } else {
            self.status &= !STATUS_VBLANK;
            // Note: STATUS_VINT_PENDING is only cleared by reading status or manual write
        }
    }

    pub fn trigger_vint(&mut self) {
        self.status |= STATUS_VINT_PENDING;
    }

    pub fn hblank_pending(&self) -> bool {
        (self.registers[REG_MODE1] & MODE1_HINT_ENABLE) != 0
    }

    pub fn update_v30_offset(&mut self) {}

    // Debugging helpers
    pub fn dump_vram(&self) -> Vec<u8> {
        self.vram.to_vec()
    }

    pub fn dump_cram(&self) -> Vec<u8> {
        self.cram.to_vec()
    }

    #[cfg(test)]
    pub fn is_control_pending(&self) -> bool {
        self.control_pending
    }

    #[cfg(test)]
    pub fn get_control_code(&self) -> u8 {
        self.control_code
    }

    #[cfg(test)]
    pub fn get_control_address(&self) -> u16 {
        self.control_address
    }

    #[cfg(test)]
    pub fn get_cram_color_pub(&self, palette: u8, index: u8) -> u16 {
        self.get_cram_color(palette, index)
    }

    pub fn read_hv_counter(&self) -> u16 {
        let h = (self.h_counter >> 1) as u8;
        let v = if self.v_counter > 0xFF {
            (self.v_counter - 0x100) as u8
        } else {
            self.v_counter as u8
        };
        ((v as u16) << 8) | (h as u16)
    }

    pub fn set_v_counter(&mut self, v: u16) {
        self.v_counter = v;
    }

    pub fn set_h_counter(&mut self, h: u16) {
        self.h_counter = h;
    }

    // === Rendering ===
    // Moved to render.rs




    pub fn write_vram_word(&mut self, addr: u16, value: u16) {
        let addr = addr as usize;
        if addr < 0x10000 {
            self.vram[addr] = (value >> 8) as u8;
            self.vram[addr ^ 1] = (value & 0xFF) as u8;
        }
    }
}

#[derive(Deserialize)]
struct VdpJsonState {
    status: Option<u16>,
    h_counter: Option<u16>,
    v_counter: Option<u16>,
    dma_pending: Option<bool>,
    registers: Option<Vec<u8>>,
    control_pending: Option<bool>,
    control_code: Option<u8>,
    control_address: Option<u16>,
    vram: Option<Vec<u8>>,
    cram: Option<Vec<u8>>,
    vsram: Option<Vec<u8>>,
    line_counter: Option<u8>,
    last_data_write: Option<u16>,
    v30_offset: Option<u16>,
    is_pal: Option<bool>,
}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        let json_state: VdpJsonState = match Deserialize::deserialize(state) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error deserializing VDP state: {}", e);
                return;
            }
        };

        if let Some(status) = json_state.status {
            self.status = status;
        }
        if let Some(h_counter) = json_state.h_counter {
            self.h_counter = h_counter;
        }
        if let Some(v_counter) = json_state.v_counter {
            self.v_counter = v_counter;
        }
        if let Some(dma_pending) = json_state.dma_pending {
            self.dma_pending = dma_pending;
        }

        if let Some(registers) = json_state.registers {
            for (i, val) in registers.iter().enumerate() {
                if i < 24 {
                    self.registers[i] = *val;
                }
            }
        }

        if let Some(pending) = json_state.control_pending {
            self.control_pending = pending;
        }
        if let Some(code) = json_state.control_code {
            self.control_code = code;
        }
        if let Some(address) = json_state.control_address {
            self.control_address = address;
        }

        if let Some(vram) = json_state.vram {
            for (i, val) in vram.iter().enumerate() {
                if i < self.vram.len() {
                    self.vram[i] = *val;
                }
            }
        }

        if let Some(cram) = json_state.cram {
            for (i, val) in cram.iter().enumerate() {
                if i < self.cram.len() {
                    self.cram[i] = *val;
                }
            }
            // Reconstruct CRAM Cache
            for i in 0..64 {
                let addr = i * 2;
                if addr + 1 < self.cram.len() {
                    let val = ((self.cram[addr + 1] as u16) << 8) | (self.cram[addr] as u16);

                    // Extract 3-bit components (bits 1-3, 5-7, 9-11)
                    let r3 = (val >> 1) & 0x07;
                    let g3 = (val >> 5) & 0x07;
                    let b3 = (val >> 9) & 0x07;

                    // Scale to RGB565 using bit repetition
                    let r5 = (r3 << 2) | (r3 >> 1);
                    let g6 = (g3 << 3) | g3;
                    let b5 = (b3 << 2) | (b3 >> 1);

                    self.cram_cache[i] = ((r5 as u16) << 11) | ((g6 as u16) << 5) | (b5 as u16);
                }
            }
        }

        if let Some(vsram) = json_state.vsram {
            for (i, val) in vsram.iter().enumerate() {
                if i < self.vsram.len() {
                    self.vsram[i] = *val;
                }
            }
        }

        if let Some(line_counter) = json_state.line_counter {
            self.line_counter = line_counter;
        }
        if let Some(last_data_write) = json_state.last_data_write {
            self.last_data_write = last_data_write;
        }
        if let Some(v30_offset) = json_state.v30_offset {
            self.v30_offset = v30_offset;
        }
        if let Some(is_pal) = json_state.is_pal {
            self.is_pal = is_pal;
        }
    }
}

#[cfg(test)]
mod tests_render;

#[cfg(test)]
mod tests_dma;

#[cfg(test)]
mod tests_dma_helpers;

#[cfg(test)]
mod test_command;

#[cfg(test)]
mod tests_control;

#[cfg(test)]
mod tests_properties;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_vdp_debuggable() {
        let mut vdp = Vdp::new();
        // Set some non-default state
        vdp.status = 0x1234;
        vdp.h_counter = 0x56;
        vdp.v_counter = 0x78;
        vdp.dma_pending = true;
        for i in 0..24 {
            vdp.registers[i] = (i + 1) as u8;
        }
        vdp.control_pending = true;
        vdp.control_code = 0x0F;
        vdp.control_address = 0x3FFF;

        // Memory state
        vdp.vram[0] = 0xAA;
        vdp.vram[0xFFFF] = 0xBB;
        vdp.cram[0] = 0xCC;
        vdp.cram[127] = 0xDD;
        vdp.vsram[0] = 0xEE;
        vdp.vsram[79] = 0xFF;

        vdp.line_counter = 42;
        vdp.last_data_write = 0xCAFE;
        vdp.v_counter = 0x78;
        vdp.v30_offset = 123;
        vdp.is_pal = true;

        // Set a color in CRAM to verify cache reconstruction
        // Color 1: White (0x0EEE) -> stored as low: 0xEE, high: 0x0E
        vdp.cram[2] = 0xEE;
        vdp.cram[3] = 0x0E;

        // Serialize
        let state = vdp.read_state();

        // Reset VDP
        let mut vdp2 = Vdp::new();

        // Deserialize
        vdp2.write_state(&state);

        // Verify basic registers
        assert_eq!(vdp2.status, 0x1234);
        assert_eq!(vdp2.h_counter, 0x56);
        assert_eq!(vdp2.v_counter, 0x78);
        assert_eq!(vdp2.dma_pending, true);
        assert_eq!(vdp2.registers[0], 1);
        assert_eq!(vdp2.registers[23], 24);
        assert_eq!(vdp2.control_pending, true);
        assert_eq!(vdp2.control_code, 0x0F);
        assert_eq!(vdp2.control_address, 0x3FFF);

        // Verify Memory
        assert_eq!(vdp2.vram[0], 0xAA);
        assert_eq!(vdp2.vram[0xFFFF], 0xBB);
        assert_eq!(vdp2.cram[0], 0xCC);
        assert_eq!(vdp2.cram[127], 0xDD);
        assert_eq!(vdp2.vsram[0], 0xEE);
        assert_eq!(vdp2.vsram[79], 0xFF);

        // Verify internal state
        assert_eq!(vdp2.line_counter, 42);
        assert_eq!(vdp2.last_data_write, 0xCAFE);
        assert_eq!(vdp2.v_counter, 0x78);
        assert_eq!(vdp2.v30_offset, 123);
        assert_eq!(vdp2.is_pal, true);

        // Verify CRAM Cache
        assert_eq!(vdp2.cram_cache[1], 0xFFFF);
    }

    #[test]
    fn test_genesis_color_to_rgb565() {
        // Test White (0x0EEE) -> 0xFFFF
        assert_eq!(Vdp::genesis_color_to_rgb565(0x0EEE), 0xFFFF);
        // Test Black (0x0000) -> 0x0000
        assert_eq!(Vdp::genesis_color_to_rgb565(0x0000), 0x0000);
        // Test Red (0x000E) -> R=7, G=0, B=0 -> R5=31, G6=0, B5=0 -> 0xF800
        assert_eq!(Vdp::genesis_color_to_rgb565(0x000E), 0xF800);
        // Test Green (0x00E0) -> R=0, G=7, B=0 -> R5=0, G6=63, B5=0 -> 0x07E0
        assert_eq!(Vdp::genesis_color_to_rgb565(0x00E0), 0x07E0);
        // Test Blue (0x0E00) -> R=0, G=0, B=7 -> R5=0, G6=0, B5=31 -> 0x001F
        assert_eq!(Vdp::genesis_color_to_rgb565(0x0E00), 0x001F);
    }
}

#[cfg(test)]
mod tests_bulk_write;

#[cfg(test)]
mod bench_render;
#[cfg(test)]
mod test_repro_white_screen;
