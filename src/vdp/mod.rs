use crate::debugger::Debuggable;
use serde::{Deserialize, Serialize};
use serde_json::Value;

pub mod constants;
pub use constants::*;

pub mod dma;
pub use dma::DmaOps;

pub mod render;
pub use render::{RenderOps, SpriteAttributes, SpriteIterator};

pub mod big_array_vram {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};
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
                for (i, item) in arr.iter_mut().enumerate() {
                    *item = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(0x10000, ArrayVisitor)
    }
}

pub mod big_array_cram {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};
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
                for (i, item) in arr.iter_mut().enumerate() {
                    *item = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(128, ArrayVisitor)
    }
}

pub mod big_array_vsram {
    use serde::de::{self, SeqAccess, Visitor};
    use serde::{Deserializer, Serializer};
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
                for (i, item) in arr.iter_mut().enumerate() {
                    *item = seq
                        .next_element()?
                        .ok_or_else(|| de::Error::invalid_length(i, &self))?;
                }
                Ok(arr)
            }
        }

        deserializer.deserialize_tuple(80, ArrayVisitor)
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

fn default_sat() -> [u8; 0x400] {
    [0; 0x400]
}

fn default_rendered_scanlines() -> [bool; 240] {
    [false; 240]
}

fn default_latched_vsram() -> [u8; 80] {
    [0; 80]
}

/// VDP Command State Machine
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CommandState {
    pub pending: bool,
    pub code: u8,
    pub address: u16,
    pub dma_pending: bool,
    #[serde(default)]
    pub read_buffer: u16,
    #[serde(default)]
    pub cd4_flag: bool,
    #[serde(default)]
    pub dma_fill_first: bool,
}

/// VDP Write FIFO Entry
#[derive(Debug, Clone, Copy, Serialize, Deserialize)]
pub struct FifoEntry {
    pub address: u16,
    pub code: u8,
    pub value: u16,
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
    pub command: CommandState,

    /// Cache of CRAM colors in RGB565 format for performance
    #[serde(skip, default = "default_cram_cache")]
    pub cram_cache: [u16; 64],

    // Timing and Sequencer
    pub mclk_line_clocks: u32,
    pub h_counter: u16,
    pub v_counter: u16,
    pub line_counter: u16,
    #[serde(skip, default)]
    pub hint_pending: bool,
    pub last_data_write: u16,
    pub v30_offset: u16,
    pub is_pal: bool,

    // FIFO
    pub fifo: Vec<FifoEntry>,
    pub fifo_full: bool,
    pub bypass_fifo: bool,

    #[serde(skip, default = "default_framebuffer")]
    pub framebuffer: Vec<u16>,

    #[serde(skip, default = "default_sat")]
    pub sat: [u8; 0x400],

    #[serde(skip, default = "default_rendered_scanlines")]
    rendered_scanlines: [bool; 240],

    #[serde(skip, default = "default_latched_vsram")]
    pub(crate) latched_vsram: [u8; 80],
    #[serde(skip, default)]
    pub(crate) latched_mode3: u8,
    #[serde(skip, default)]
    pub(crate) latched_hscroll_a: u16,
    #[serde(skip, default)]
    pub(crate) latched_hscroll_b: u16,
    #[serde(skip, default)]
    pub(crate) latched_scroll_line: u16,
    #[serde(skip, default)]
    pub(crate) latched_scroll_valid: bool,

    #[serde(skip)]
    pub debug_plane: Option<char>,
    /// Whether sprite overflow (dot or count) occurred on the previous scanline.
    /// Used to gate the X=0 sprite mask trigger.
    #[serde(skip, default)]
    pub(crate) prev_line_sprite_overflow: bool,
    /// 68k stall cycles owed by the most recent DMA operation. Drained by the
    /// Bus through `take_dma_stall_cycles`.
    #[serde(skip, default)]
    pub dma_stall_cycles: u32,
    /// Whether the SAT cache is initialized. `tick` latches it at each
    /// line boundary and clears this flag only on reset. Rendering paths
    /// lazily sync if not yet valid (so a freshly constructed VDP doesn't
    /// render with an empty SAT cache).
    #[serde(skip, default)]
    pub(crate) sat_cache_valid: bool,
    /// HINT is "due" on the current line but its MCLK threshold hasn't
    /// yet been reached. Asserted in `tick` once `mclk_line_clocks` crosses
    /// `HINT_OFFSET_MCLK`.
    #[serde(skip, default)]
    pub(crate) hint_due: bool,
    /// VINT is "due" on the current line; same deferred-assertion model.
    #[serde(skip, default)]
    pub(crate) vint_due: bool,
    /// Mid-line segmented-render watermark. Pixels at x < line_split_x for the
    /// current scanline have already been committed to the framebuffer with the
    /// state at the time they were drawn. A mid-line CRAM/VSRAM/register write
    /// advances this watermark to the current MCLK's pixel position and
    /// re-renders pixels [watermark..320] with the new state. Reset to 0 at
    /// each fresh line render.
    #[serde(skip, default)]
    pub(crate) line_split_x: u16,
}

impl Default for Vdp {
    fn default() -> Self {
        Self::new()
    }
}

impl Vdp {
    const ACTIVE_DISPLAY_START_MCLK: u32 = 860;
    /// MCLK offset within a line at which HINT becomes asserted.
    /// Hardware asserts a few slots into the line — well before active
    /// display starts. Approximated at 200 MCLK.
    const HINT_OFFSET_MCLK: u32 = 200;
    /// MCLK offset within the first VBlank line at which VINT is asserted.
    /// Hardware asserts ~slot 0 of line 0xE0+ several slots later.
    const VINT_OFFSET_MCLK: u32 = 480;

    pub fn new() -> Self {
        let mut vdp = Self {
            vram: [0; 0x10000],
            cram: [0; 128],
            vsram: [0; 80],
            registers: [0; NUM_REGISTERS],
            status: STATUS_FIFO_EMPTY | 0x3400,
            command: CommandState::default(),
            cram_cache: [0; 64],
            mclk_line_clocks: 0,
            h_counter: 0,
            v_counter: 0,
            line_counter: 0,
            hint_pending: false,
            last_data_write: 0,
            v30_offset: 0,
            is_pal: false,
            fifo: Vec::with_capacity(4),
            fifo_full: false,
            bypass_fifo: false,
            framebuffer: vec![0; 320 * 240],
            sat: [0; 0x400],
            rendered_scanlines: [false; 240],
            latched_vsram: [0; 80],
            latched_mode3: 0,
            latched_hscroll_a: 0,
            latched_hscroll_b: 0,
            latched_scroll_line: 0,
            latched_scroll_valid: false,
            debug_plane: {
                #[cfg(debug_assertions)]
                {
                    std::env::var("GENTEEL_DEBUG_PLANE")
                        .ok()
                        .and_then(|s| s.chars().next())
                }
                #[cfg(not(debug_assertions))]
                {
                    None
                }
            },
            prev_line_sprite_overflow: false,
            dma_stall_cycles: 0,
            sat_cache_valid: false,
            hint_due: false,
            vint_due: false,
            line_split_x: 0,
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
        self.status = STATUS_FIFO_EMPTY | 0x3400;
        self.command = CommandState::default();
        self.fifo.clear();
        self.fifo_full = false;
        self.bypass_fifo = false;
        self.mclk_line_clocks = 0;
        self.v_counter = 0;
        self.h_counter = 0;
        self.line_counter = 0;
        self.hint_pending = false;
        self.rendered_scanlines.fill(false);
        self.latched_scroll_valid = false;
        self.prev_line_sprite_overflow = false;
        self.dma_stall_cycles = 0;
        self.reconstruct_cram_cache();
        // SAT cache stays invalid; first render or first tick line wrap
        // will latch it from VRAM.
        self.sat_cache_valid = false;
    }

    /// Return and clear DMA stall cycles owed to the 68k.
    pub fn take_dma_stall_cycles(&mut self) -> u32 {
        let c = self.dma_stall_cycles;
        self.dma_stall_cycles = 0;
        c
    }

    fn compute_hscroll_words(&self, fetch_line: u16, mode3: u8) -> (u16, u16) {
        let hs_mode = mode3 & 0x03;
        let hs_base = self.hscroll_address();
        let hs_addr = match hs_mode {
            0x00 => hs_base,
            // Prohibited mode: repeats the first 8 lines' scroll values.
            0x01 => hs_base + (((fetch_line as usize) & 7) * 4),
            // Per-cell: one longword per 8-line cell row. The table is the same
            // as per-line, read every 8th entry -> byte offset (line & ~7) * 4.
            0x02 => hs_base + (((fetch_line as usize) & !7) * 4),
            0x03 => hs_base + ((fetch_line as usize) * 4),
            _ => hs_base,
        };

        let read_word = |addr: usize, vram: &[u8; 0x10000]| -> u16 {
            let hi = vram[addr & 0xFFFF];
            let lo = vram[(addr.wrapping_add(1)) & 0xFFFF];
            ((hi as u16) << 8) | (lo as u16)
        };

        (
            read_word(hs_addr, &self.vram),
            read_word(hs_addr.wrapping_add(2), &self.vram),
        )
    }

    pub(crate) fn latch_scroll_state_for_line(&mut self, line: u16) {
        self.latched_mode3 = self.registers[REG_MODE3];
        self.latched_vsram = self.vsram;
        let (a, b) = self.compute_hscroll_words(line, self.latched_mode3);
        self.latched_hscroll_a = a;
        self.latched_hscroll_b = b;
        self.latched_scroll_line = line;
        self.latched_scroll_valid = true;
    }

    pub fn set_pal(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    pub fn write_data(&mut self, value: u16) {
        self.command.pending = false;
        self.last_data_write = value;

        if self.bypass_fifo {
            // Check for DMA Fill (Mode 2, code 1, bit 7 of source high set)
            if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
                && (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
                && self.command.dma_pending
            {
                self.execute_dma();
                return;
            }

            self.process_fifo_entry(FifoEntry {
                address: self.command.address,
                code: self.command.code,
                value,
            });
        } else {
            // Check for DMA Fill - on real hardware, writing to data port triggers the fill.
            // If the FIFO is used, the trigger itself might be delayed?
            // In most implementations, the *write* that triggers it is what matters.
            if (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
                && (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
                && self.command.dma_pending
            {
                // For now, we still handle DMA Fill synchronously to pass existing tests,
                // but we will move it to process_slot soon for full cycle accuracy.
                self.execute_dma();
                return;
            }

            // If FIFO is not bypassed, queue the write.
            if self.fifo.len() < 4 {
                self.fifo.push(FifoEntry {
                    address: self.command.address,
                    code: self.command.code,
                    value,
                });
                if self.fifo.len() == 4 {
                    self.fifo_full = true;
                }
                self.status &= !STATUS_FIFO_EMPTY;
                if self.fifo_full {
                    self.status |= STATUS_FIFO_FULL;
                }
            } else {
                // FIFO full: drain the oldest queued entry to free a slot
                // (approximating the CPU stalling until a slot opens) so writes
                // still commit in program order, then queue this one. The FIFO
                // stays full, so STATUS_FIFO_FULL remains correctly asserted.
                let oldest = self.fifo.remove(0);
                self.process_fifo_entry(oldest);
                self.fifo.push(FifoEntry {
                    address: self.command.address,
                    code: self.command.code,
                    value,
                });
            }
        }

        self.command.address = self
            .command
            .address
            .wrapping_add(self.auto_increment() as u16);
    }

    fn process_fifo_entry(&mut self, entry: FifoEntry) {
        let addr = entry.address;
        let code = entry.code;
        let value = entry.value;

        match code & 0x0F {
            VRAM_WRITE => {
                let idx = addr as usize;
                if idx < self.vram.len() {
                    self.vram[idx] = (value >> 8) as u8;
                    self.vram[idx ^ 1] = (value & 0xFF) as u8;
                    // Writes to the SAT region during active display do NOT
                    // update the SAT cache (LSU latches it once per line at
                    // HBlank). Cache refresh happens at the next line wrap.
                }
            }
            CRAM_WRITE => {
                let idx = (addr as usize / 2) & 0x3F;
                self.cram[idx * 2] = (value & 0xFF) as u8;
                self.cram[idx * 2 + 1] = (value >> 8) as u8;
                self.cram_cache[idx] = Self::genesis_color_to_rgb565(value);
                self.redraw_current_scanline_if_visible();
            }
            VSRAM_WRITE => {
                let idx = (addr as usize) % 80;
                self.vsram[idx] = (value >> 8) as u8;
                if idx + 1 < 80 {
                    self.vsram[idx + 1] = (value & 0xFF) as u8;
                }
                self.redraw_current_scanline_if_visible();
            }
            _ => {}
        }
    }

    pub fn read_data(&mut self) -> u16 {
        self.command.pending = false;

        let val = self.command.read_buffer;
        self.command.cd4_flag = false;

        self.try_prefetch();

        val
    }

    pub(crate) fn try_prefetch(&mut self) {
        if !self.fifo.is_empty() {
            // Wait for FIFO to drain before prefetching
            return;
        }

        let addr = self.command.address;
        let code = self.command.code;

        match code & 0x0F {
            VRAM_READ => {
                let idx = addr as usize;
                let val = ((self.vram[idx] as u16) << 8) | (self.vram[(addr ^ 1) as usize] as u16);
                self.command.read_buffer = val;
                self.command.cd4_flag = true;
            }
            CRAM_READ => {
                let idx = (addr as usize) % 128;
                let mut val = if idx + 1 < self.cram.len() {
                    ((self.cram[idx + 1] as u16) << 8) | (self.cram[idx] as u16)
                } else {
                    0
                };
                // Borrow undefined bits from FIFO history (approximated by last_data_write)
                val |= self.last_data_write & 0xF000;
                self.command.read_buffer = val;
                self.command.cd4_flag = true;
            }
            VSRAM_READ => {
                let idx = (addr as usize) % 80;
                let mut val = if idx + 1 < self.vsram.len() {
                    ((self.vsram[idx] as u16) << 8) | (self.vsram[idx + 1] as u16)
                } else {
                    0
                };
                // VSRAM has 10 bits, borrow undefined top bits
                val |= self.last_data_write & 0xFC00;
                self.command.read_buffer = val;
                self.command.cd4_flag = true;
            }
            _ => {
                self.command.cd4_flag = true;
                return; // Do not increment address on invalid read target
            }
        }

        self.command.address = self
            .command
            .address
            .wrapping_add(self.auto_increment() as u16);
    }

    pub fn write_control(&mut self, value: u16) {
        if self.command.pending {
            // Second word of command
            self.command.code = (self.command.code & 0x03) | ((value >> 2) & 0x3C) as u8;
            self.command.address = (self.command.address & 0x3FFF) | ((value & 0x0003) << 14);
            self.command.pending = false;

            // Check if DMA should be triggered (CD5 bit set in code)
            if (self.command.code & 0x20) != 0
                && (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0
            {
                self.command.dma_pending = true;
            }

            // Prefetch if target is a read
            if (self.command.code & 0x01) == 0 {
                self.try_prefetch();
            }
        } else {
            // Check if this is a register write (Bits 15,14 = 10)
            if (value & 0xC000) == 0x8000 {
                let reg = ((value >> 8) & 0x1F) as usize;
                let val = (value & 0xFF) as u8;
                if reg < NUM_REGISTERS {
                    self.registers[reg] = val;
                    if matches!(
                        reg,
                        REG_MODE1
                            | REG_MODE2
                            | REG_PLANE_A
                            | REG_WINDOW
                            | REG_PLANE_B
                            | REG_BG_COLOR
                            | REG_MODE3
                            | REG_MODE4
                            | REG_HSCROLL
                            | REG_PLANE_SIZE
                            | REG_WINDOW_H_POS
                            | REG_WINDOW_V_POS
                    ) {
                        self.redraw_current_scanline_if_visible();
                    }
                }
                return;
            }

            // First word of command
            self.command.code = (self.command.code & 0xFC) | ((value >> 14) & 0x03) as u8;
            self.command.address = (self.command.address & 0xC000) | (value & 0x3FFF);
            self.command.pending = true;
        }
    }

    #[inline(always)]
    pub fn read_status(&mut self) -> u16 {
        // Reading the status register clears the write pending flag (resets the command state machine).
        self.command.pending = false;
        let mut res = self.status;
        if self.command.dma_pending {
            res |= STATUS_DMA;
        }
        // Region bit (bit 0) reflects the console video standard (1 = PAL/50Hz).
        if self.is_pal {
            res |= STATUS_PAL;
        }
        // The VBLANK flag (bit 3) also reads set whenever the display is
        // force-blanked (reg 1 bit 6 = 0), because the VDP then yields full bus
        // bandwidth for the whole frame.
        if !self.display_enabled() {
            res |= STATUS_VBLANK;
        }
        // Reading status clears the read-and-clear flags: VINT/F (bit 7),
        // sprite overflow (bit 6), and sprite collision (bit 5).
        self.status &= !(STATUS_VINT_PENDING | STATUS_SOVR | STATUS_COLLISION);
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
        self.command.pending
    }

    pub fn display_enabled(&self) -> bool {
        (self.registers[REG_MODE2] & MODE2_DISPLAY_ENABLE) != 0
    }

    /// Whether the VDP is currently within an active-display scanline
    /// (used by the CPU to tighten HINT-sensing cadence — see R3).
    pub fn in_active_display(&self) -> bool {
        (self.status & STATUS_VBLANK) == 0
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
        if self.h40_mode() {
            ((self.registers[REG_SPRITE_TABLE] as usize) & 0x7E) << 9
        } else {
            ((self.registers[REG_SPRITE_TABLE] as usize) & 0x7F) << 9
        }
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

    pub fn acknowledge_vint(&mut self) {
        self.status &= !STATUS_VINT_PENDING;
    }

    pub fn acknowledge_hint(&mut self) {
        self.hint_pending = false;
    }

    pub fn vblank_pending(&self) -> bool {
        (self.status & STATUS_VINT_PENDING) != 0 && self.vint_enabled()
    }

    pub fn hint_pending(&self) -> bool {
        self.hint_pending && self.hint_enabled()
    }

    /// Map a 0-based H tick within the line to the externally visible 8-bit H
    /// counter, applying the hardware jump:
    ///   * H40: 0x00..=0xB6 then 0xE4..=0xFF (total 211 values)
    ///   * H32: 0x00..=0x93 then 0xE9..=0xFF (total 171 values)
    #[inline]
    pub(crate) fn h_counter_value_for_tick(tick: u32, is_h40: bool) -> u8 {
        if is_h40 {
            // First segment: 0x00..=0xB6 (0xB7 values)
            // Second segment: 0xE4..=0xFF (0x1C values)
            if tick <= 0xB6 {
                tick as u8
            } else {
                let s2 = (tick - 0xB7) as u8;
                0xE4u8.wrapping_add(s2)
            }
        } else if tick <= 0x93 {
            tick as u8
        } else {
            let s2 = (tick - 0x94) as u8;
            0xE9u8.wrapping_add(s2)
        }
    }

    /// Map an internal 0-based line number (0..262 NTSC / 0..313 PAL) to the
    /// externally visible 8-bit V counter, applying the hardware jump.
    ///
    /// NTSC: 0x00..=0xEA then 0xE5..=0xFF (total 262).
    /// PAL: 0x00..=0xFF, then a short low run right after the wrap, then a high
    ///   run ending at 0xFF (total 313). V28: low run 0x00..=0x02 then
    ///   0xCA..=0xFF; V30: low run 0x00..=0x0A then 0xD2..=0xFF.
    #[inline]
    pub(crate) fn v_counter_value_for_line(line: u16, is_pal: bool, v30: bool) -> u8 {
        if !is_pal {
            // NTSC
            if line <= 0xEA {
                line as u8
            } else {
                let s2 = line - 0xEB;
                0xE5u8.wrapping_add(s2 as u8)
            }
        } else if line <= 0xFF {
            line as u8
        } else {
            // The external counter wraps to 0x00 for a few lines immediately
            // after 0xFF, then jumps to the high run that ends at 0xFF.
            let (low_run, high_start): (u16, u16) = if v30 { (0x0B, 0xD2) } else { (0x03, 0xCA) };
            let s = line - 0x100;
            if s < low_run {
                s as u8
            } else {
                (high_start + (s - low_run)) as u8
            }
        }
    }

    pub fn read_hv_counter(&self) -> u16 {
        let is_h40 = self.h40_mode();
        // Approximate H tick from MCLK. Total H ticks per line: 211 (H40), 171 (H32).
        let total_ticks: u32 = if is_h40 { 211 } else { 171 };
        let tick = (self.mclk_line_clocks * total_ticks) / 3420;
        let h = Self::h_counter_value_for_tick(tick, is_h40);

        let v30 = (self.registers[REG_MODE2] & MODE2_V30_MODE) != 0;
        let v = Self::v_counter_value_for_line(self.v_counter, self.is_pal, v30);
        ((v as u16) << 8) | (h as u16)
    }

    pub(crate) fn sync_sat_cache(&mut self) {
        let base = self.sprite_table_address();
        for (i, byte) in self.sat.iter_mut().enumerate() {
            *byte = self.vram[(base + i) & 0xFFFF];
        }
        self.sat_cache_valid = true;
    }

    /// Used by render paths to ensure the SAT cache has at least been
    /// initialized once. Production code latches at line boundary in
    /// `tick`; this catches the cold-start case (and tests that build
    /// VRAM directly without ticking).
    pub(crate) fn ensure_sat_cache(&mut self) {
        if !self.sat_cache_valid {
            self.sync_sat_cache();
        }
    }

    /// Map current MCLK within an active scanline to a pixel-x position.
    /// Used for the mid-line segmented re-render (R1/R2).
    ///   * Before active display: returns 0 (no pixels emitted yet).
    ///   * Past active display end: returns scanline width (no remaining pixels).
    pub(crate) fn mid_line_pixel_x(&self) -> u16 {
        let width = self.screen_width();
        if self.mclk_line_clocks < Self::ACTIVE_DISPLAY_START_MCLK {
            return 0;
        }
        let active_end = 3420;
        if self.mclk_line_clocks >= active_end {
            return width;
        }
        let elapsed = self.mclk_line_clocks - Self::ACTIVE_DISPLAY_START_MCLK;
        let active_span = active_end - Self::ACTIVE_DISPLAY_START_MCLK;
        // Linear map MCLK -> pixel within the active region.
        let x = (elapsed * width as u32) / active_span;
        x.min(width as u32) as u16
    }

    /// Mid-line CRAM/VSRAM/register write hook: instead of atomically
    /// re-rendering the whole scanline (which clobbered all earlier pixels
    /// drawn with the previous state — fatal for road-gradient effects),
    /// commit only pixels [line_split_x .. screen_width] with the new state
    /// and advance the watermark to the current MCLK's pixel position.
    pub(crate) fn redraw_current_scanline_if_visible(&mut self) {
        if !self.display_enabled() {
            return;
        }

        // HBlank window of line N: the *previous* line N-1 has already been
        // fully rendered and its pixels are on screen — writes here cannot
        // change them. The write IS queued for the upcoming line N, whose
        // render at MCLK 860 will pick up the new state automatically.
        // (R-RR-3: the previous behavior incorrectly redrew line N-1 with
        // the new state, applying the next line's palette to the previous
        // line — exactly the off-by-one that made Road Rash II's road
        // gradient render as noise.)
        if self.mclk_line_clocks < Self::ACTIVE_DISPLAY_START_MCLK {
            return;
        }

        let line = self.v_counter;
        let line_idx = line as usize;
        if line_idx >= self.screen_height() as usize || !self.rendered_scanlines[line_idx] {
            return;
        }

        let split_x = self.mid_line_pixel_x();
        if split_x >= self.screen_width() {
            // Line fully emitted; nothing to repaint.
            return;
        }
        // Pixels at [0..split_x] are "emitted" by the scan beam already and
        // must stay put. Pixels at [split_x..screen_width] are still in
        // flight and reflect the current state — re-render them.
        self.line_split_x = split_x;
        self.render_line_from(line, split_x);
    }

    fn render_scanline_if_needed(&mut self, line: u16) {
        if line < self.screen_height() && !self.rendered_scanlines[line as usize] {
            self.render_line(line);
        }
    }

    pub fn set_v_counter(&mut self, v: u16) {
        self.v_counter = v;
    }

    pub fn set_h_counter(&mut self, h: u16) {
        self.h_counter = h;
    }

    /// Decode the plane size from the VDP register value (Reg 16)
    pub fn decode_plane_size(val: u8) -> (usize, usize) {
        // Bits 0-1 (HSZ1-0): horizontal size (width in tiles)
        let w = match val & 0x03 {
            0x00 => 32,
            0x01 => 64,
            0x03 => 128,
            _ => 32,
        };
        // Bits 4-5 (VSZ1-0): vertical size (height in tiles)
        let h = match (val >> 4) & 0x03 {
            0x00 => 32,
            0x01 => 64,
            0x03 => 128,
            _ => 32,
        };
        (w, h)
    }

    pub(crate) fn plane_size(&self) -> (usize, usize) {
        Self::decode_plane_size(self.registers[REG_PLANE_SIZE])
    }

    pub(crate) fn window_address(&self) -> usize {
        if self.h40_mode() {
            ((self.registers[REG_WINDOW] as usize) & 0x3C) << 10
        } else {
            ((self.registers[REG_WINDOW] as usize) & 0x3E) << 10
        }
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
        // Increment frame-based rolling offset for V30 mode
        self.v30_offset = self.v30_offset.wrapping_add(1);
    }

    /// Advance VDP state by N Master Clock (MCLK) cycles.
    pub fn tick<F>(&mut self, mclk: u32, mut read_bus_word: F)
    where
        F: FnMut(u32) -> u16,
    {
        if !self.latched_scroll_valid {
            self.latch_scroll_state_for_line(self.v_counter);
        }

        let prev_line_clocks = self.mclk_line_clocks;
        self.mclk_line_clocks += mclk;

        // Deferred-interrupt assertion: if HINT/VINT was queued at the
        // previous line wrap and the MCLK has now crossed its threshold,
        // assert the actual pending flag.
        if self.hint_due
            && prev_line_clocks < Self::HINT_OFFSET_MCLK
            && self.mclk_line_clocks >= Self::HINT_OFFSET_MCLK
        {
            self.hint_pending = true;
            self.hint_due = false;
        }
        if self.vint_due
            && prev_line_clocks < Self::VINT_OFFSET_MCLK
            && self.mclk_line_clocks >= Self::VINT_OFFSET_MCLK
        {
            self.status |= STATUS_VINT_PENDING;
            self.vint_due = false;
        }

        let is_h40 = self.h40_mode();
        let total_slots = if is_h40 { 210 } else { 171 };

        let prev_slot = if is_h40 {
            (prev_line_clocks * 210) / 3420
        } else {
            prev_line_clocks / 20
        };

        let curr_slot = if is_h40 {
            (self.mclk_line_clocks * 210) / 3420
        } else {
            self.mclk_line_clocks / 20
        };

        let process_limit = std::cmp::min(curr_slot, total_slots as u32);

        if prev_line_clocks < Self::ACTIVE_DISPLAY_START_MCLK
            && self.mclk_line_clocks >= Self::ACTIVE_DISPLAY_START_MCLK
        {
            self.render_scanline_if_needed(self.v_counter);
        }

        for slot_idx in prev_slot..process_limit {
            self.process_slot(slot_idx as usize, is_h40, &mut read_bus_word);
        }

        // Handle line wrapping (3420 MCLK per line)
        if self.mclk_line_clocks >= 3420 {
            self.mclk_line_clocks -= 3420;
            let frame_lines = if self.is_pal { 313 } else { 262 };
            self.v_counter = (self.v_counter + 1) % frame_lines;
            self.latch_scroll_state_for_line(self.v_counter);
            // Latch the SAT cache (LSU) from VRAM. On real hardware this
            // happens in two passes during HBlank/active display; we
            // approximate with a single line-boundary snapshot.
            self.sync_sat_cache();
            // New line: reset the mid-line render watermark.
            self.line_split_x = 0;

            let active_lines = self.screen_height();
            self.hint_pending = false;
            self.hint_due = false;
            self.vint_due = false;

            // Handle VBlank status flag based on V counter
            if self.v_counter == active_lines {
                self.status |= STATUS_VBLANK;
                // VINT is "due" but the actual STATUS_VINT_PENDING flag is
                // set later once the MCLK threshold within this line is
                // crossed (see post-wrap deferred-assertion check below).
                self.vint_due = true;
            } else if self.v_counter == 0 {
                self.status &= !STATUS_VBLANK;
            }

            // The H-int counter is decremented on every active line AND on the
            // first blanking line (v_counter == active_lines); only the
            // remaining blanking lines reload it. Using `<` here dropped the
            // HINT at the active/blank boundary (one lost HINT per frame).
            if self.v_counter <= active_lines {
                if self.line_counter == 0 {
                    self.line_counter = self.registers[REG_H_INT_COUNTER] as u16;
                    self.hint_due = true;
                } else {
                    self.line_counter -= 1;
                }
            } else {
                self.line_counter = self.registers[REG_H_INT_COUNTER] as u16;
            }

            if self.v_counter == 0 {
                self.rendered_scanlines.fill(false);
            }

            let next_line_curr_slot = if is_h40 {
                (self.mclk_line_clocks * 210) / 3420
            } else {
                self.mclk_line_clocks / 20
            };
            if self.mclk_line_clocks >= Self::ACTIVE_DISPLAY_START_MCLK {
                self.render_scanline_if_needed(self.v_counter);
            }
            // Post-wrap deferred-interrupt check: if the current tick
            // advanced past the new line's HINT/VINT thresholds, assert now.
            if self.hint_due && self.mclk_line_clocks >= Self::HINT_OFFSET_MCLK {
                self.hint_pending = true;
                self.hint_due = false;
            }
            if self.vint_due && self.mclk_line_clocks >= Self::VINT_OFFSET_MCLK {
                self.status |= STATUS_VINT_PENDING;
                self.vint_due = false;
            }
            for slot_idx in 0..next_line_curr_slot {
                self.process_slot(slot_idx as usize, is_h40, &mut read_bus_word);
            }
        }

        // Line clock 0 begins in HBlank. Visible fetch begins once the line
        // has advanced past the HBlank window.
        if self.mclk_line_clocks < Self::ACTIVE_DISPLAY_START_MCLK {
            self.status |= STATUS_HBLANK;
        } else {
            self.status &= !STATUS_HBLANK;
        }
    }

    fn process_slot<F>(&mut self, slot_idx: usize, is_h40: bool, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        let is_external = if is_h40 {
            if slot_idx < 210 {
                H40_EXTERNAL_SLOTS[slot_idx]
            } else {
                false
            }
        } else {
            if slot_idx < 171 {
                H32_EXTERNAL_SLOTS[slot_idx]
            } else {
                false
            }
        };

        // If in VBlank, nearly all slots are external opportunities
        let in_vblank = (self.status & STATUS_VBLANK) != 0;
        let is_available = is_external || in_vblank;

        if !is_available {
            return;
        }

        if !self.fifo.is_empty() {
            // R-RR-2: temporarily set mclk_line_clocks to this slot's MCLK
            // position so that any redraw triggered by the drained entry
            // computes its split_x relative to the slot's actual pixel
            // position, not the (later) end-of-tick MCLK.
            let saved_mclk = self.mclk_line_clocks;
            let total_slots = if is_h40 { 210u32 } else { 171u32 };
            let slot_mclk = (slot_idx as u32 * 3420) / total_slots;
            self.mclk_line_clocks = slot_mclk;

            let entry = self.fifo.remove(0);
            self.process_fifo_entry(entry);

            self.mclk_line_clocks = saved_mclk;

            self.fifo_full = false;
            self.status &= !STATUS_FIFO_FULL;
            if self.fifo.is_empty() {
                self.status |= STATUS_FIFO_EMPTY;

                // Trigger deferred prefetch if waiting
                if !self.command.cd4_flag && (self.command.code & 0x01) == 0 {
                    self.try_prefetch();
                }
            }
        } else if self.command.dma_pending {
            self.step_dma(read_bus_word);
        }
    }
}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        let mut new_vdp: Vdp = match Vdp::deserialize(state) {
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
mod tests_read;

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

#[cfg(test)]
mod tests_decode_plane_size;

#[cfg(test)]
mod tests_getters;

#[cfg(test)]
mod tests_constants;

#[cfg(test)]
mod test_dma_transfer;

#[cfg(test)]
mod tests_audit_fixes;
#[cfg(test)]
mod tests_sprite_iterator;
