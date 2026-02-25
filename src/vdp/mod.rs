use crate::debugger::Debuggable;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod constants;
pub use constants::*;

pub mod dma;
pub use dma::DmaOps;

pub mod render;
pub use render::RenderOps;

pub mod big_array_vram {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    pub fn serialize<S>(data: &[u8; 0x10000], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(data)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 0x10000], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor;

        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = [u8; 0x10000];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of length 65536")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0u8; 0x10000];
                for i in 0..0x10000 {
                    arr[i] = seq.next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(0x10000, ArrayVisitor)
    }
}

pub mod big_array_cram {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    pub fn serialize<S>(data: &[u8; 128], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(data)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 128], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor;

        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = [u8; 128];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of length 128")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0u8; 128];
                for i in 0..128 {
                    arr[i] = seq.next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(128, ArrayVisitor)
    }
}

pub mod big_array_vsram {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde::de::{self, SeqAccess, Visitor};
    use std::fmt;

    pub fn serialize<S>(data: &[u8; 80], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.collect_seq(data)
    }

    pub fn deserialize<'de, D>(deserializer: D) -> Result<[u8; 80], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor;

        impl<'de> Visitor<'de> for ArrayVisitor {
            type Value = [u8; 80];

            fn expecting(&self, formatter: &mut fmt::Formatter) -> fmt::Result {
                formatter.write_str("an array of length 80")
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<Self::Value, A::Error>
            where
                A: SeqAccess<'de>,
            {
                let mut arr = [0u8; 80];
                for i in 0..80 {
                    arr[i] = seq.next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(80, ArrayVisitor)
    }
}

mod serde_arrays {
    use serde::{Deserialize, Deserializer, Serialize, Serializer};
    use serde::ser::SerializeTuple;

    pub fn serialize<S, const N: usize>(data: &[u8; N], serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        let mut s = serializer.serialize_tuple(N)?;
        for item in data {
            s.serialize_element(item)?;
        }
        s.end()
    }

    pub fn deserialize<'de, D, const N: usize>(deserializer: D) -> Result<[u8; N], D::Error>
    where
        D: Deserializer<'de>,
    {
        struct ArrayVisitor<const N: usize>;

        impl<'de, const N: usize> serde::de::Visitor<'de> for ArrayVisitor<N> {
            type Value = [u8; N];

            fn expecting(&self, formatter: &mut std::fmt::Formatter) -> fmt::Result {
                formatter.write_fmt(format_args!("an array of size {}", N))
            }

            fn visit_seq<A>(self, mut seq: A) -> Result<[u8; N], A::Error>
            where
                A: serde::de::SeqAccess<'de>,
            {
                let mut arr = [0u8; N];
                for i in 0..N {
                    arr[i] = seq.next_element()?
                        .ok_or_else(|| serde::de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(N, ArrayVisitor)
    }
}

fn default_vram() -> [u8; 0x10000] {
    [0; 0x10000]
}

fn default_cram() -> [u8; 128] {
    [0; 128]
}

fn default_vsram() -> [u8; 80] {
    [0; 80]
}

fn default_cram_cache() -> [u16; 64] {
    [0; 64]
}

fn default_framebuffer() -> Vec<u16> {
    vec![0; 320 * 240]
}

/// Genesis Video Display Processor (VDP)
#[derive(Debug, Serialize, Deserialize)]
pub struct Vdp {
    #[serde(with = "big_array_vram", default = "default_vram")]
    pub vram: [u8; 0x10000],
    #[serde(with = "big_array_cram", default = "default_cram")]
    pub cram: [u8; 128],
    #[serde(with = "big_array_vsram", default = "default_vsram")]
    pub vsram: [u8; 80],
    pub registers: [u8; NUM_REGISTERS],
    pub status: u16,
    pub control_pending: bool,
    pub control_code: u8,
    pub control_address: u16,
    pub dma_pending: bool,

    /// Cache of CRAM colors in RGB565 format for performance
    #[serde(skip, default = "default_cram_cache")]
    pub cram_cache: [u16; 64],

    pub h_counter: u16,
    pub v_counter: u16,
    pub line_counter: u16,
    pub last_data_write: u16,
    pub v30_offset: u16,
    pub is_pal: bool,

    #[serde(skip, default = "default_framebuffer")]
    pub framebuffer: Vec<u16>,
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Vdp {
    pub fn new() -> Self {
        let mut vdp = Self {
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
        };
        vdp.reset();
        vdp
    }

    /// Reconstruct cram_cache from cram
    pub fn reconstruct_cram_cache(&mut self) {
        for i in 0..64 {
            let addr = i * 2;
            if addr + 1 < self.cram.len() {
                let val = ((self.cram[addr + 1] as u16) << 8) | (self.cram[addr] as u16);
                // Use helper to avoid duplication
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

    pub fn write_data(&mut self, value: u16) {
        self.control_pending = false;
        self.last_data_write = value;

        // Check for DMA Fill (Mode 2, code 1, bit 7 of source high set)
        if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
            && (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
            && self.dma_pending
        {
            self.execute_dma();
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
        if self.control_pending {
            // Second word of command
            self.control_code = (self.control_code & 0x03) | ((value >> 2) & 0x3C) as u8;
            self.control_address = (self.control_address & 0x3FFF) | ((value & 0x0003) << 14);
            self.control_pending = false;

            // Check if DMA should be triggered (CD5 bit set in code)
            if (self.control_code & 0x20) != 0 && (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0 {
                self.dma_pending = true;
            }
        } else {
            // Check if this is a register write (Bits 15,14 = 10)
            if (value & 0xC000) == 0x8000 {
                let reg = ((value >> 8) & 0x1F) as usize;
                let val = (value & 0xFF) as u8;
                if reg < NUM_REGISTERS {
                    self.registers[reg] = val;
                }
                return;
            }

            // First word of command
            self.control_code = (self.control_code & 0xFC) | ((value >> 14) & 0x03) as u8;
            self.control_address = (self.control_address & 0xC000) | (value & 0x3FFF);
            self.control_pending = true;
        }
    }

    #[inline(always)]
    pub fn read_status(&mut self) -> u16 {
        // Reading the status register clears the write pending flag (resets the command state machine).
        self.control_pending = false;
        let mut res = self.status;
        if self.dma_pending {
            res |= 0x0002; // DMA Busy
        }
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

    pub fn auto_increment(&self) -> u8 {
        self.registers[REG_AUTO_INC]
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
        ((self.registers[REG_SPRITE_TABLE] as usize) & 0x7F) << 9
    }

    pub fn hscroll_address(&self) -> usize {
        // Bits 0-5 specify bits 10-15 of VRAM address
        ((self.registers[REG_HSCROLL] as usize) & 0x3F) << 10
    }

    pub fn write_vram_word(&mut self, addr: u16, value: u16) {
        let addr = addr as usize;
        if addr < 0x10000 {
            self.vram[addr] = (value >> 8) as u8;
            self.vram[addr ^ 1] = (value & 0xFF) as u8;
        }
    }

    pub fn set_vblank(&mut self, active: bool) {
        if active {
            self.status |= STATUS_VBLANK;
            self.status |= STATUS_VINT_PENDING;
        } else {
            self.status &= !STATUS_VBLANK;
        }
    }

    pub fn trigger_vint(&mut self) {
        self.status |= STATUS_VINT_PENDING;
    }

    pub fn vblank_pending(&self) -> bool {
        (self.status & STATUS_VINT_PENDING) != 0 && self.vint_enabled()
    }

    pub fn hblank_pending(&self) -> bool {
        self.hint_enabled()
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

    pub(crate) fn plane_size(&self) -> (usize, usize) {
        let val = self.registers[REG_PLANE_SIZE];
        let w = match (val >> 4) & 0x03 {
            0x00 => 32,
            0x01 => 64,
            0x03 => 128,
            _ => 32,
        };
        let h = match val & 0x03 {
            0x00 => 32,
            0x01 => 64,
            0x03 => 128,
            _ => 32,
        };
        (w, h)
    }

    pub(crate) fn window_address(&self) -> usize {
        ((self.registers[REG_WINDOW] as usize) & 0x3E) << 11
    }

    pub(crate) fn is_window_area(&self, x: u16, y: u16) -> bool {
        let h_pos = self.registers[REG_WINDOW_H_POS];
        let v_pos = self.registers[REG_WINDOW_V_POS];

        let h_point = (h_pos as u16 & 0x1F) * 16;
        let v_point = (v_pos as u16 & 0x1F) * 8;

        let h_dir = (h_pos & 0x80) != 0;
        let v_dir = (v_pos & 0x80) != 0;

        let in_h_window = if h_dir {
            x >= h_point
        } else {
            x < h_point
        };

        let in_v_window = if v_dir {
            y >= v_point
        } else {
            y < v_point
        };

        in_h_window || in_v_window
    }

    pub fn set_region(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    pub fn mode1(&self) -> u8 {
        self.registers[REG_MODE1]
    }

    pub fn mode2(&self) -> u8 {
        self.registers[REG_MODE2]
    }

    pub fn dma_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
    }

    pub fn update_v30_offset(&mut self) {
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

#[cfg(test)]
mod tests_draw_row_refactor;
