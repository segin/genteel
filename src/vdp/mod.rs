use crate::debugger::Debuggable;
use crate::vdp::render::RenderOps;
use serde::{Deserialize, Serialize};
use serde_json::Value;

// VDP Control Codes (bits 0-3)
pub const VRAM_READ: u8 = 0x00;
pub const VRAM_WRITE: u8 = 0x01;
pub const CRAM_WRITE: u8 = 0x03;
pub const VSRAM_READ: u8 = 0x04;
pub const VSRAM_WRITE: u8 = 0x05;
pub const CRAM_READ: u8 = 0x08;

// Register indices
pub const REG_MODE1: usize = 0;
pub const REG_MODE2: usize = 1;
pub const REG_PLANE_A: usize = 2;
pub const REG_WINDOW: usize = 3;
pub const REG_PLANE_B: usize = 4;
pub const REG_SPRITE_TABLE: usize = 5;
pub const REG_SPRITE_PATTERN: usize = 6;
pub const REG_BG_COLOR: usize = 7;
pub const REG_H_INT_COUNTER: usize = 10;
pub const REG_MODE3: usize = 11;
pub const REG_MODE4: usize = 12;
pub const REG_HSCROLL: usize = 13;
pub const REG_AUTO_INC: usize = 15;
pub const REG_PLANE_SIZE: usize = 16;
pub const REG_WINDOW_H_POS: usize = 17;
pub const REG_WINDOW_V_POS: usize = 18;
pub const REG_DMA_LEN_LO: usize = 19;
pub const REG_DMA_LEN_HI: usize = 20;
pub const REG_DMA_SRC_LO: usize = 21;
pub const REG_DMA_SRC_MID: usize = 22;
pub const REG_DMA_SRC_HI: usize = 23;

// Mode bits
pub const MODE1_HINT_ENABLE: u8 = 0x10;
pub const MODE2_V30_MODE: u8 = 0x08;
pub const MODE2_DMA_ENABLE: u8 = 0x10;
pub const MODE2_VINT_ENABLE: u8 = 0x20;
pub const MODE2_DISPLAY_ENABLE: u8 = 0x40;
pub const MODE4_H40_MODE: u8 = 0x81; // H40 mode check mask

// DMA Modes
pub const DMA_MODE_MASK: u8 = 0xC0;
pub const DMA_MODE_FILL: u8 = 0x80;
pub const DMA_MODE_COPY: u8 = 0xC0;

// Status bits
pub const STATUS_VBLANK: u16 = 0x0008;
pub const STATUS_VINT_PENDING: u16 = 0x0080;

pub const NUM_REGISTERS: usize = 24;

/// Genesis Video Display Processor (VDP)
#[derive(Debug, Serialize, Deserialize)]
pub struct Vdp {
    pub vram: [u8; 0x10000],
    pub cram: [u8; 128],
    pub vsram: [u8; 80],
    pub registers: [u8; NUM_REGISTERS],
    pub status: u16,
    pub control_pending: bool,
    pub control_code: u8,
    pub control_address: u16,
    pub dma_pending: bool,

    /// Cache of CRAM colors in RGB565 format for performance
    #[serde(skip)]
    pub cram_cache: [u16; 64],

    pub h_counter: u16,
    pub v_counter: u16,
    pub line_counter: u16,
    pub last_data_write: u16,
    pub v30_offset: u16,
    pub is_pal: bool,

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
        Self {
            vram: [0; 0x10000],
            cram: [0; 128],
            vsram: [0; 80],
            registers: [0; NUM_REGISTERS],
            status: 0x3400, // Initial status (FIFO empty, etc)
            control_pending: false,
            control_code: 0,
            control_address: 0,
            dma_pending: false,
            cram_cache: [0; 64],
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
                self.cram_cache[i] = Self::genesis_color_to_rgb565(val);
            }
        }
    }

    pub fn reset(&mut self) {
        self.registers.fill(0);
        self.status = 0x3400;
        self.control_pending = false;
        self.dma_pending = false;
        self.reconstruct_cram_cache();
    }

    pub fn set_pal(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    fn perform_dma_fill(&mut self, len: u32) {
        let fill_byte = (self.last_data_write >> 8) as u8;
        let mut addr = self.control_address;
        let inc = self.auto_increment() as u16;

        if inc == 1 {
            let start = addr as usize;
            let count = len as usize;
            let vram_len = self.vram.len();

            // Handle wrapping
            if start + count <= vram_len {
                self.vram[start..start + count].fill(fill_byte);
            } else {
                let first_part = vram_len - start;
                self.vram[start..vram_len].fill(fill_byte);
                let remaining = count - first_part;
                if remaining > 0 {
                    self.vram[0..remaining].fill(fill_byte);
                }
            }
            self.control_address = addr.wrapping_add(len as u16);
        } else if inc == 0 {
            if len > 0 {
                self.vram[addr as usize] = fill_byte;
            }
        } else {
            for _ in 0..len {
                self.vram[addr as usize] = fill_byte;
                addr = addr.wrapping_add(inc);
            }
            self.control_address = addr;
        }
    }

    fn perform_dma_fill(&mut self, len: u32) {
        let fill_byte = (self.last_data_write >> 8) as u8;
        let mut addr = self.control_address;
        let inc = self.auto_increment() as u16;

        if inc == 1 {
            let start = addr as usize;
            let count = len as usize;
            let vram_len = self.vram.len();

            // Handle wrapping
            if start + count <= vram_len {
                self.vram[start..start + count].fill(fill_byte);
            } else {
                let first_part = vram_len - start;
                self.vram[start..vram_len].fill(fill_byte);
                let remaining = count - first_part;
                if remaining > 0 {
                    self.vram[0..remaining].fill(fill_byte);
                }
            }
            self.control_address = addr.wrapping_add(len as u16);
        } else if inc == 0 {
            if len > 0 {
                self.vram[addr as usize] = fill_byte;
            }
        } else {
            for _ in 0..len {
                self.vram[addr as usize] = fill_byte;
                addr = addr.wrapping_add(inc);
            }
            self.control_address = addr;
        }
    }

    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        // Check for DMA Fill (Mode 2, code 1, bit 7 of source high set)
        if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
            && (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
            && self.dma_pending
        {
            let length = self.dma_length();

            // DMA Fill writes bytes. Length register specifies number of bytes.
            // If length is 0, it is treated as 0x10000 (64KB).
            let len = if length == 0 { 0x10000 } else { length };

            self.perform_dma_fill(len);
            self.dma_pending = false;
            return;
        }

        let addr = self.control_address;
        let code = self.control_code;

        match code & 0x0F {
            VRAM_WRITE => {
                let idx = addr as usize;
                if idx < self.vram.len() {
                    self.vram[idx] = (value >> 8) as u8;
                    self.vram[idx ^ 1] = (value & 0xFF) as u8;
                }
            }
            CRAM_WRITE => {
                let idx = (addr as usize / 2) & 0x3F;
                self.cram[idx * 2] = (value & 0xFF) as u8;
                self.cram[idx * 2 + 1] = (value >> 8) as u8;
                self.cram_cache[idx] = Self::genesis_color_to_rgb565(value);
            }
            VSRAM_WRITE => {
                let idx = (addr as usize) % 80;
                self.vsram[idx] = (value >> 8) as u8;
                if idx + 1 < 80 {
                    self.vsram[idx + 1] = (value & 0xFF) as u8;
                }
            }
            _ => {}
        }

        self.control_address = addr.wrapping_add(self.auto_increment() as u16);
    }

    pub fn read_data(&mut self) -> u16 {
        self.control_pending = false;
        let addr = self.control_address;
        let code = self.control_code;

        let val = match code & 0x0F {
            VRAM_READ => {
                let idx = addr as usize;
                if idx + 1 < self.vram.len() {
                    ((self.vram[idx] as u16) << 8) | (self.vram[idx + 1] as u16)
                } else {
                    0
                }
            }
            CRAM_READ => {
                let idx = (addr as usize) % 128;
                if idx + 1 < self.cram.len() {
                    ((self.cram[idx + 1] as u16) << 8) | (self.cram[idx] as u16)
                } else {
                    0
                }
            }
            VSRAM_READ => {
                let idx = (addr as usize) % 80;
                if idx + 1 < self.vsram.len() {
                    ((self.vsram[idx] as u16) << 8) | (self.vsram[idx + 1] as u16)
                } else {
                    0
                }
            }
            _ => 0,
        };

        self.control_address = addr.wrapping_add(self.auto_increment() as u16);
        val
    }

    pub fn write_control(&mut self, value: u16) {
        if !self.control_pending {
            // First word of command
            self.control_code = (self.control_code & 0xFC) | ((value >> 14) & 0x03) as u8;
            self.control_address = (self.control_address & 0xC000) | (value & 0x3FFF);
            self.control_pending = true;
        } else {
            // Second word of command
            self.control_code = (self.control_code & 0x03) | ((value >> 2) & 0x3C) as u8;
            self.control_address = (self.control_address & 0x3FFF) | ((value & 0x0003) << 14);
            self.control_pending = false;

            // Check if this was a register write
            if (value & 0xC000) == 0x8000 {
                let reg = ((value >> 8) & 0x1F) as usize;
                let val = (value & 0xFF) as u8;
                if reg < NUM_REGISTERS {
                    self.registers[reg] = val;
                }
            }

            // Check if DMA should be triggered (CD1 bit set in code)
            if (self.control_code & 0x20) != 0 && (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
            {
                self.dma_pending = true;
            }
        }
    }

    #[inline(always)]
    pub fn read_status(&mut self) -> u16 {
        // Reading the status register clears the write pending flag (resets the command state machine).
        self.control_pending = false;
        let res = self.status;
        // Reading status clears the VInt pending bit (Bit 7)
        self.status &= !STATUS_VINT_PENDING;
        res
    }

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

    pub fn dma_mode(&self) -> u8 {
        self.registers[REG_DMA_SRC_HI]
    }

    pub fn dma_source(&self) -> u32 {
        ((self.registers[REG_DMA_SRC_HI] as u32) << 17)
            | ((self.registers[REG_DMA_SRC_MID] as u32) << 9)
            | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    }

    pub fn dma_length(&self) -> u32 {
        ((self.registers[REG_DMA_LEN_HI] as u32) << 8) | (self.registers[REG_DMA_LEN_LO] as u32)
    }

    pub fn dma_source_transfer(&self) -> u32 {
        let hi = self.registers[REG_DMA_SRC_HI] as u32;
        let mid = self.registers[REG_DMA_SRC_MID] as u32;
        let lo = self.registers[REG_DMA_SRC_LO] as u32;

        if (hi & 0x40) != 0 {
            // RAM Transfer: bits 23-16 are forced to 1
            0xFF0000 | (mid << 9) | (lo << 1)
        } else {
            // ROM/Expansion Transfer: bit 7 is ignored, bits 6-0 are address
            ((hi & 0x3F) << 17) | (mid << 9) | (lo << 1)
        }
    }

    /// Check if DMA mode is 0 or 1 (68k Transfer)
    pub fn is_dma_transfer(&self) -> bool {
        (self.registers[REG_DMA_SRC_HI] & 0x80) == 0
    }

    pub fn is_control_pending(&self) -> bool {
        self.control_pending
    }

    pub fn display_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DISPLAY_ENABLE) != 0
    }

    pub fn vint_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_VINT_ENABLE) != 0
    }

    pub fn hint_enabled(&self) -> bool {
        (self.registers[REG_MODE1] & MODE1_HINT_ENABLE) != 0
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
        if (self.registers[REG_MODE2] & MODE2_V30_MODE) != 0 {
            240
        } else {
            224
        }
    }

    pub fn plane_a_address(&self) -> usize {
        // Bits 3-5 specify bits 13-15 of VRAM address
        ((self.registers[REG_PLANE_A] as usize) & 0x38) << 10
    }

    pub fn plane_b_address(&self) -> usize {
        // Bits 0-2 specify bits 13-15 of VRAM address
        ((self.registers[REG_PLANE_B] as usize) & 0x07) << 13
    }

    pub fn sprite_table_address(&self) -> usize {
        // Bits 0-6 specify bits 9-15 of VRAM address (H40 mode)
        // In H32 mode, bits 0-6 specify bits 10-16? No, bits 0-6 specify bits 9-15.
        ((self.registers[REG_SPRITE_TABLE] as usize) & 0x7F) << 9
    }

    pub fn hscroll_address(&self) -> usize {
        // Bits 0-5 specify bits 10-15 of VRAM address
        ((self.registers[REG_HSCROLL] as usize) & 0x3F) << 10
    }

    pub fn window_address(&self) -> usize {
        // Bits 1-5 specify bits 11-15 of VRAM address
        ((self.registers[REG_WINDOW] as usize) & 0x3E) << 10
    }

    fn is_window_area(&self, screen_x: u16, fetch_line: u16) -> bool {
        let win_h = self.registers[REG_WINDOW_H_POS];
        let win_v = self.registers[REG_WINDOW_V_POS];

        let cell_x = (screen_x >> 3) as u8;
        let cell_y = (fetch_line >> 3) as u8;

        let h_pos = win_h & 0x1F;
        let h_right = (win_h & 0x80) != 0;

        // In H40 mode, horizontal position is in units of 2 cells (16 pixels)
        let h_unit = if self.h40_mode() { h_pos << 1 } else { h_pos };

        let in_h = if h_right {
            cell_x >= h_unit
        } else {
            cell_x < h_unit
        };

        let v_pos = win_v & 0x1F;
        let v_down = (win_v & 0x80) != 0;
        let in_v = if v_down {
            cell_y >= v_pos
        } else {
            cell_y < v_pos
        };

        in_h || in_v
    }

    pub fn plane_size(&self) -> (usize, usize) {
        let w = match self.registers[REG_PLANE_SIZE] & 0x03 {
            0 => 32,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        let h = match (self.registers[REG_PLANE_SIZE] >> 4) & 0x03 {
            0 => 32,
            1 => 64,
            3 => 128,
            _ => 32,
        };
        (w, h)
    }

    pub fn execute_dma(&mut self) -> u32 {
        let length = self.dma_length();
        // If length is 0, it is treated as 0x10000 (64KB)
        let len = if length == 0 { 0x10000 } else { length };

        let mode = self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK;

        match mode {
            DMA_MODE_FILL => {
                self.perform_dma_fill(len);
            }
            DMA_MODE_COPY => {
                let mut source = (self.dma_source() & 0xFFFF) as u16;
                let mut dest = self.control_address;
                let inc = self.auto_increment() as u16;

                for _ in 0..len {
                    let val = self.vram[source as usize];
                    self.vram[dest as usize] = val;
                    source = source.wrapping_add(1);
                    dest = dest.wrapping_add(inc);
                }
                self.control_address = dest;
            }
            _ => {}
        }

        self.dma_pending = false;
        len
    }

    pub fn bg_color(&self) -> (u8, u8) {
        let bg_idx = self.registers[REG_BG_COLOR];
        let pal = (bg_idx >> 4) & 0x03;
        let color = bg_idx & 0x0F;
        (pal, color)
    }

    #[inline(always)]
    pub fn get_cram_color(&self, palette: u8, index: u8) -> u16 {
        let addr = ((palette as usize) * 16) + (index as usize);
        // SAFETY: palette is 2 bits (0-3), index is 4 bits (0-15).
        // Max addr is (3 * 16) + 15 = 63, which is within cram_cache bounds (64).
        unsafe { *self.cram_cache.get_unchecked(addr) }
    }
}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        let mut new_vdp: Vdp = match serde_json::from_value(state.clone()) {
            Ok(s) => s,
            Err(e) => {
                eprintln!("Error deserializing VDP state: {}", e);
                return;
            }
        };

        // Swap framebuffer to preserve allocation
        std::mem::swap(&mut self.framebuffer, &mut new_vdp.framebuffer);

        // Reconstruct CRAM cache
        new_vdp.reconstruct_cram_cache();

        *self = new_vdp;
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
mod tests_bulk_write;

#[cfg(test)]
mod tests_properties;

#[cfg(test)]
mod bench_render;

#[cfg(test)]
mod bench_dma;

#[cfg(test)]
mod test_repro_white_screen;
