use crate::debugger::Debuggable;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::VecDeque;

pub mod constants;
pub use constants::*;

pub mod dma;
pub use dma::{DmaError, DmaOps};

pub mod render;
pub use render::{RenderOps, SpriteAttributes, SpriteIterator};

pub(crate) mod timing;
use timing::{LineTiming, MCLK_PER_LINE};

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

fn default_layer() -> [u8; 320] {
    [0; 320]
}

/// The kind of DMA operation, latched from register 23 when the control
/// port receives a command with CD5 set.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum DmaKind {
    /// 68k bus to VRAM/CRAM/VSRAM. The 68k is frozen while it runs.
    #[default]
    Bus,
    /// VRAM/CRAM/VSRAM fill, started by the next data-port write.
    Fill,
    /// VRAM to VRAM copy.
    Copy,
}

impl DmaKind {
    /// Decode register 23 bits 7-6.
    pub fn from_register(reg23: u8) -> Self {
        match reg23 & DMA_MODE_MASK {
            DMA_MODE_FILL => DmaKind::Fill,
            DMA_MODE_COPY => DmaKind::Copy,
            _ => DmaKind::Bus,
        }
    }
}

/// VDP Command State Machine
#[derive(Debug, Default, Serialize, Deserialize)]
pub struct CommandState {
    pub pending: bool,
    pub code: u8,
    pub address: u16,
    /// A DMA operation is armed or running. While set, the status DMA busy
    /// flag reads set.
    pub dma_pending: bool,
    #[serde(default)]
    pub read_buffer: u16,
    #[serde(default)]
    pub cd4_flag: bool,
    /// The DMA kind latched at the CD5 control-port write. Later writes to
    /// register 23 do not change a pending or running DMA.
    #[serde(default)]
    pub dma_kind: DmaKind,
    /// A fill is armed and waits for its data-port write.
    #[serde(default)]
    pub dma_fill_wait: bool,
    /// Access slots still to pass before an armed fill writes its first byte
    /// (the slots taken by the triggering data-port write).
    #[serde(default)]
    pub(crate) dma_delay: u8,
    /// The first of the two access slots a VRAM word (68k DMA) or a copied
    /// byte needs has been used.
    #[serde(default)]
    pub(crate) dma_half: bool,
    /// The first of the two access slots the oldest FIFO entry (a VRAM
    /// write) needs has been used.
    #[serde(default)]
    pub(crate) fifo_half: bool,
    /// The second word of a 68k long write to the control port whose first
    /// word started a 68k DMA. It is written when the DMA completes.
    #[serde(default)]
    pub(crate) cached_control: Option<u16>,
}

impl CommandState {
    /// A 68k bus DMA is running (the 68k is frozen).
    #[inline]
    pub fn bus_dma_active(&self) -> bool {
        self.dma_pending && self.dma_kind == DmaKind::Bus
    }
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
    pub v_counter: u16,
    pub line_counter: u16,
    #[serde(default)]
    pub hint_pending: bool,
    /// The last four words written to the data port, as a ring. Hardware
    /// reads the unused bits of CRAM/VSRAM/8-bit VRAM reads, and the data of
    /// CRAM/VSRAM DMA fills, from the FIFO slot that will be written next
    /// (i.e. the oldest of the four).
    #[serde(default)]
    pub(crate) fifo_history: [u16; 4],
    #[serde(default)]
    pub(crate) fifo_history_idx: usize,
    pub is_pal: bool,

    // FIFO
    pub fifo: VecDeque<FifoEntry>,
    pub fifo_full: bool,
    /// Test hook: commit data-port writes at once instead of queueing them.
    #[cfg(test)]
    #[serde(skip)]
    pub(crate) bypass_fifo: bool,

    #[serde(skip, default = "default_framebuffer")]
    pub framebuffer: Vec<u16>,

    #[serde(skip, default = "default_sat")]
    pub sat: [u8; 0x400],

    #[serde(skip, default = "default_rendered_scanlines")]
    rendered_scanlines: [bool; 240],

    /// Layer buffers (plane B, plane A/window, sprites) of the line most
    /// recently rendered, kept so colour-only changes in the middle of a line
    /// can be recomposited without re-rendering the layers.
    #[serde(skip, default = "default_layer")]
    pub(crate) layer_b: [u8; 320],
    #[serde(skip, default = "default_layer")]
    pub(crate) layer_a: [u8; 320],
    #[serde(skip, default = "default_layer")]
    pub(crate) layer_s: [u8; 320],
    /// The line `layer_*` belong to.
    #[serde(skip, default)]
    pub(crate) layer_line: Option<u16>,
    /// `prev_line_sprite_overflow` as it was when `layer_line` was first
    /// rendered, so re-rendering that line evaluates sprites identically.
    #[serde(skip, default)]
    pub(crate) layer_prev_overflow: bool,

    #[serde(skip)]
    pub debug_plane: Option<char>,
    /// Whether sprite overflow (dot or count) occurred on the previous scanline.
    /// Used to gate the X=0 sprite mask trigger.
    #[serde(skip, default)]
    pub(crate) prev_line_sprite_overflow: bool,
    /// The VINT point of the first blanking line has passed since the
    /// scheduler last asked; the Z80 INT line is asserted there.
    #[serde(skip, default)]
    pub(crate) z80_vint: bool,
    /// Whether the internal SAT cache has been loaded. It is kept up to date
    /// by VRAM writes into the sprite table; a fresh or restored VDP loads it
    /// from VRAM once, on first use.
    #[serde(skip, default)]
    pub(crate) sat_cache_valid: bool,
    /// HV counter latch. Setting register 0 bit 1 (M3) captures the counter
    /// and reads return the captured value until M3 is cleared; a light gun
    /// driving TH would also relatch it.
    #[serde(default)]
    pub(crate) hv_latched: Option<u16>,
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
            status: STATUS_FIFO_EMPTY | 0x3400,
            command: CommandState::default(),
            cram_cache: [0; 64],
            mclk_line_clocks: 0,
            v_counter: 0,
            line_counter: 0,
            hint_pending: false,
            fifo_history: [0; 4],
            fifo_history_idx: 0,
            is_pal: false,
            fifo: VecDeque::with_capacity(4),
            fifo_full: false,
            #[cfg(test)]
            bypass_fifo: false,
            framebuffer: vec![0; 320 * 240],
            sat: [0; 0x400],
            rendered_scanlines: [false; 240],
            layer_b: [0; 320],
            layer_a: [0; 320],
            layer_s: [0; 320],
            layer_line: None,
            layer_prev_overflow: false,
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
            z80_vint: false,
            sat_cache_valid: false,
            hv_latched: None,
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
        self.mclk_line_clocks = 0;
        self.v_counter = 0;
        self.line_counter = 0;
        self.hint_pending = false;
        self.rendered_scanlines.fill(false);
        self.layer_line = None;
        self.prev_line_sprite_overflow = false;
        self.z80_vint = false;
        self.hv_latched = None;
        self.reconstruct_cram_cache();
        // SAT cache stays invalid; first render or first tick line wrap
        // will latch it from VRAM.
        self.sat_cache_valid = false;
    }

    /// Whether the Z80 INT line should be asserted: true once after each
    /// VINT point of the first blanking line (Genesis Plus GX asserts
    /// `Z80.irq_state` together with the 68k VINT, whether or not VINT is
    /// enabled in register 1).
    pub fn take_z80_vint(&mut self) -> bool {
        std::mem::take(&mut self.z80_vint)
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

    pub fn set_pal(&mut self, is_pal: bool) {
        self.is_pal = is_pal;
    }

    /// Record a data-port write in the 4-entry FIFO history ring.
    fn record_fifo_write(&mut self, value: u16) {
        self.fifo_history[self.fifo_history_idx & 3] = value;
        self.fifo_history_idx = (self.fifo_history_idx + 1) & 3;
    }

    /// The FIFO slot that will be written next (the oldest of the last four
    /// writes). Hardware sources undefined read bits and CRAM/VSRAM fill data
    /// from it.
    pub(crate) fn fifo_next_entry(&self) -> u16 {
        self.fifo_history[self.fifo_history_idx & 3]
    }

    /// The word most recently written to the data port.
    pub fn last_data_write(&self) -> u16 {
        self.fifo_history[(self.fifo_history_idx + 3) & 3]
    }

    pub fn write_data(&mut self, value: u16) {
        self.command.pending = false;

        if self.command.dma_fill_wait {
            /* This data-port write starts the armed fill. The fill was armed
            by the CD5 control write; DMA enable is not checked again here
            (Genesis Plus GX `vdp_68k_data_w_m5` only tests `dmafill`).
            Anything still queued in the FIFO is committed first so it cannot
            land on top of the fill afterwards. */
            self.flush_fifo();
            self.record_fifo_write(value);
            /* The triggering word is written normally first (GPGX
            `vdp_68k_data_w_m5`), then the fill continues from the next
            address. */
            self.process_fifo_entry(FifoEntry {
                address: self.command.address,
                code: self.command.code,
                value,
            });
            self.command.address = self
                .command
                .address
                .wrapping_add(self.auto_increment() as u16);
            /* The fill now runs one byte per access slot, after the two slots
            the triggering write takes (GPGX `vdp_dma_update` delays the start
            by two slots). The 68k is not stalled; the DMA busy flag reads set
            until the fill ends. */
            self.command.dma_fill_wait = false;
            self.command.dma_delay = 2;
            self.command.dma_half = false;
            return;
        }

        self.record_fifo_write(value);
        let entry = FifoEntry {
            address: self.command.address,
            code: self.command.code,
            value,
        };

        #[cfg(test)]
        if self.bypass_fifo {
            self.process_fifo_entry(entry);
            self.command.address = self
                .command
                .address
                .wrapping_add(self.auto_increment() as u16);
            return;
        }

        if self.fifo.len() < 4 {
            self.fifo.push_back(entry);
            if self.fifo.len() == 4 {
                self.fifo_full = true;
            }
            self.status &= !STATUS_FIFO_EMPTY;
            if self.fifo_full {
                self.status |= STATUS_FIFO_FULL;
            }
        } else {
            /* FIFO full: drain the oldest queued entry to free a slot
            (approximating the CPU stalling until a slot opens) so writes
            still commit in program order, then queue this one. The FIFO
            stays full, so STATUS_FIFO_FULL remains correctly asserted. */
            if let Some(oldest) = self.fifo.pop_front() {
                self.command.fifo_half = false;
                self.process_fifo_entry(oldest);
            }
            self.fifo.push_back(entry);
        }

        self.command.address = self
            .command
            .address
            .wrapping_add(self.auto_increment() as u16);
    }

    /// Commit every queued FIFO entry immediately, in order.
    pub(crate) fn flush_fifo(&mut self) {
        while let Some(entry) = self.fifo.pop_front() {
            self.process_fifo_entry(entry);
        }
        self.command.fifo_half = false;
        self.fifo_full = false;
        self.status &= !STATUS_FIFO_FULL;
        self.status |= STATUS_FIFO_EMPTY;
    }

    pub(crate) fn process_fifo_entry(&mut self, entry: FifoEntry) {
        match entry.code & 0x0F {
            VRAM_WRITE => self.write_vram_word(entry.address, entry.value),
            CRAM_WRITE => self.write_cram_word(entry.address, entry.value),
            VSRAM_WRITE => self.write_vsram_word(entry.address, entry.value),
            _ => {}
        }
    }

    /// Store a CRAM word. Address bit 0 is ignored and only the 9 implemented
    /// colour bits (`0000BBB0GGG0RRR0`) are kept.
    pub(crate) fn write_cram_word(&mut self, addr: u16, value: u16) {
        let idx = ((addr & 0x7E) >> 1) as usize;
        let value = value & CRAM_COLOR_MASK;
        let [lo, hi] = value.to_le_bytes();
        let rgb = Self::genesis_color_to_rgb565(value);
        let unchanged =
            self.cram[idx * 2] == lo && self.cram[idx * 2 + 1] == hi && self.cram_cache[idx] == rgb;
        self.cram[idx * 2] = lo;
        self.cram[idx * 2 + 1] = hi;
        self.cram_cache[idx] = rgb;
        if !unchanged {
            // A colour change only needs the line recomposited, not re-rendered.
            self.recomposite_current_scanline_if_visible();
        }
    }

    /// Store a VSRAM word. Address bit 0 is ignored, writes at or above 0x50
    /// (past the 40 implemented entries) are discarded, and only the 11
    /// implemented bits of each entry are kept.
    pub(crate) fn write_vsram_word(&mut self, addr: u16, value: u16) {
        let idx = (addr & 0x7E) as usize;
        if idx >= VSRAM_SIZE {
            return;
        }
        let [hi, lo] = (value & VSRAM_ENTRY_MASK).to_be_bytes();
        if self.vsram[idx] == hi && self.vsram[idx + 1] == lo {
            return;
        }
        self.vsram[idx] = hi;
        self.vsram[idx + 1] = lo;
        self.redraw_current_scanline_if_visible();
    }

    pub fn read_data(&mut self) -> u16 {
        self.command.pending = false;

        /* A read waits for queued writes to commit. If the prefetch has not
        happened yet (it is deferred while the FIFO holds entries), commit the
        FIFO now and fetch, so the read sees the data just written. */
        if !self.command.cd4_flag {
            self.flush_fifo();
            self.try_prefetch();
        }

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
        let next = self.fifo_next_entry();

        let val = match code & 0x0F {
            VRAM_READ => {
                // Address bit 0 is ignored: reads always return the aligned word.
                let idx = (addr & 0xFFFE) as usize;
                u16::from_be_bytes([self.vram[idx], self.vram[idx + 1]])
            }
            VRAM_READ_8BIT => {
                // Undocumented 8-bit read: the byte at addr ^ 1, upper bits
                // from the next FIFO entry.
                (self.vram[(addr ^ 1) as usize] as u16) | (next & 0xFF00)
            }
            CRAM_READ => {
                let idx = ((addr & 0x7E) >> 1) as usize;
                let word = u16::from_le_bytes([self.cram[idx * 2], self.cram[idx * 2 + 1]]);
                (word & CRAM_COLOR_MASK) | (next & !CRAM_COLOR_MASK)
            }
            VSRAM_READ => {
                let mut idx = (addr & 0x7E) as usize;
                if idx >= VSRAM_SIZE {
                    idx = 0;
                }
                let word = u16::from_be_bytes([self.vsram[idx], self.vsram[idx + 1]]);
                (word & VSRAM_ENTRY_MASK) | (next & !VSRAM_ENTRY_MASK)
            }
            _ => {
                self.command.cd4_flag = true;
                return; // Do not increment address on invalid read target
            }
        };
        self.command.read_buffer = val;
        self.command.cd4_flag = true;

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
            // A new command invalidates any earlier prefetched read data.
            self.command.cd4_flag = false;

            /* A command with CD5 set starts a DMA if DMA is enabled. The DMA
            kind is latched here (GPGX `vdp_68k_ctrl_w` sets `dma_type`), so
            later register 23 writes cannot turn it into another kind. A
            fill only arms here and starts at the next data-port write. */
            let cd5 = (self.command.code & 0x20) != 0;
            if cd5 && (self.registers[REG_MODE2] & MODE2_DMA_ENABLE) != 0 {
                let kind = DmaKind::from_register(self.registers[REG_DMA_SRC_HI]);
                self.command.dma_pending = true;
                self.command.dma_kind = kind;
                self.command.dma_fill_wait = kind == DmaKind::Fill;
                self.command.dma_delay = 0;
                self.command.dma_half = false;
            }

            /* Prefetch only for plain read commands. A command with CD5 set
            starts a DMA (e.g. VRAM copy, code 0x30) and must not read or
            advance the address. */
            if !cd5 && (self.command.code & 0x01) == 0 {
                self.try_prefetch();
            }
        } else {
            /* A 68k long write whose first word started a 68k DMA: the 68k is
            frozen, so the second word reaches the VDP only once the DMA is
            done (GPGX `vdp_68k_ctrl_w`, `cached_write`; Formula One,
            Kawasaki Superbike Challenge). */
            if self.command.bus_dma_active() {
                self.command.cached_control = Some(value);
                return;
            }
            /* First word of a command. The address bits 13-0 and code bits
            CD1-CD0 are loaded even when the word turns out to be a register
            write. */
            self.command.code = (self.command.code & 0xFC) | ((value >> 14) & 0x03) as u8;
            self.command.address = (self.command.address & 0xC000) | (value & 0x3FFF);

            // Register write (bits 15,14 = 10)
            if (value & 0xC000) == 0x8000 {
                let reg = ((value >> 8) & 0x1F) as usize;
                let val = (value & 0xFF) as u8;
                if reg < NUM_REGISTERS {
                    self.write_register(reg, val);
                }
                return;
            }

            self.command.pending = true;
        }
    }

    fn write_register(&mut self, reg: usize, val: u8) {
        let old = self.registers[reg];
        let changed = old != val;
        self.registers[reg] = val;
        /* Setting M3 (register 0 bit 1) latches the HV counter at once and
        clearing it frees the counter (GPGX `vdp_reg_w` case 0, Sunset
        Riders). Rewriting the bit unchanged does nothing. */
        if reg == REG_MODE1 && ((old ^ val) & MODE1_HV_LATCH) != 0 {
            self.hv_latched = if (val & MODE1_HV_LATCH) != 0 {
                Some(self.compute_hv_counter())
            } else {
                None
            };
        }
        if !changed {
            return;
        }
        if matches!(reg, REG_MODE1 | REG_BG_COLOR) {
            // Column-0 masking and the backdrop only affect compositing.
            self.recomposite_current_scanline_if_visible();
        } else if matches!(
            reg,
            REG_MODE2
                | REG_PLANE_A
                | REG_WINDOW
                | REG_PLANE_B
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

    /// The status register as the 68k would read it, without any of the
    /// read side effects. Debuggers and the GUI use this.
    pub fn peek_status(&self) -> u16 {
        let mut res = self.status;
        if self.command.dma_pending {
            res |= STATUS_DMA;
        }
        // Region bit (bit 0) reflects the console video standard (1 = PAL/50Hz).
        if self.is_pal {
            res |= STATUS_PAL;
        }
        /* The VBLANK flag (bit 3) also reads set whenever the display is
        force-blanked (reg 1 bit 6 = 0), because the VDP then yields full bus
        bandwidth for the whole frame. */
        if !self.display_enabled() {
            res |= STATUS_VBLANK;
        }
        // HBLANK follows the beam position within the line.
        let t = self.line_timing();
        res &= !STATUS_HBLANK;
        if (t.hblank_start..t.hblank_end).contains(&(self.mclk_line_clocks % MCLK_PER_LINE)) {
            res |= STATUS_HBLANK;
        }
        res
    }

    /// Read the status register from the 68k side. This resets the control
    /// port write-pending latch and clears the sprite overflow and collision
    /// flags. The VINT flag (bit 7) stays set until the interrupt is
    /// acknowledged, so polling status cannot lose a masked VINT.
    #[inline(always)]
    pub fn read_status(&mut self) -> u16 {
        let res = self.peek_status();
        self.command.pending = false;
        self.status &= !(STATUS_SOVR | STATUS_COLLISION);
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

    /// H40 (320-pixel) mode is selected by register 12 bit 0 (RS1).
    pub fn h40_mode(&self) -> bool {
        (self.registers[REG_MODE4] & MODE4_RS1) != 0
    }

    /// Interlace mode 1 or 2 is selected (register 12 bit 1, LSM0).
    pub fn interlaced(&self) -> bool {
        (self.registers[REG_MODE4] & MODE4_LSM0) != 0
    }

    /// Interlace mode 2 (double resolution) is selected (register 12 bits
    /// 2-1 = 11).
    pub fn interlace_mode_2(&self) -> bool {
        (self.registers[REG_MODE4] & (MODE4_LSM0 | MODE4_LSM1)) == (MODE4_LSM0 | MODE4_LSM1)
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

    /// Word write to VRAM: the MSB goes to `addr` and the LSB to `addr ^ 1`
    /// (so an odd address stores the word byte-swapped at the even pair).
    pub fn write_vram_word(&mut self, addr: u16, value: u16) {
        self.write_vram_byte(addr, (value >> 8) as u8);
        self.write_vram_byte(addr ^ 1, (value & 0xFF) as u8);
    }

    /// Byte write to VRAM.
    #[inline]
    pub(crate) fn write_vram_byte(&mut self, addr: u16, value: u8) {
        self.vram[addr as usize] = value;
        /* Writes into the sprite attribute table also update the VDP's
        internal SAT cache (GPGX `vdp_bus_w`: 1 KiB block in H40, 512 bytes
        in H32, selected by register 5). */
        let (base_mask, addr_mask) = if self.h40_mode() {
            (0xFC00u16, 0x03FFu16)
        } else {
            (0xFE00, 0x01FF)
        };
        let sat_base = ((self.registers[REG_SPRITE_TABLE] as u16) << 9) & base_mask;
        if (addr & base_mask) == sat_base {
            self.sat[(addr & addr_mask) as usize] = value;
        }
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

    /// Timeline constants for the current H32/H40 mode.
    #[inline]
    pub(crate) fn line_timing(&self) -> &'static LineTiming {
        LineTiming::for_mode(self.h40_mode())
    }

    /// Number of lines per frame for the current video standard.
    #[inline]
    pub(crate) fn frame_lines(&self) -> u16 {
        if self.is_pal {
            313
        } else {
            262
        }
    }

    /// Externally visible 8-bit H counter at the current beam position.
    pub(crate) fn h_counter(&self) -> u8 {
        timing::h_counter_at(self.mclk_line_clocks, self.h40_mode())
    }

    /// Map an internal 0-based line number (0..262 NTSC / 0..313 PAL) to the
    /// 9-bit V counter, applying the hardware jump: once the counter passes
    /// its overflow value it continues from `overflow - lines_per_frame`.
    ///
    /// NTSC V28: 0x00..=0xEA, 0x1E5..=0x1FF. NTSC V30 never overflows.
    /// PAL V28: 0x000..=0x102, 0x1CA..=0x1FF. PAL V30: 0x000..=0x10A,
    /// 0x1D2..=0x1FF.
    #[inline]
    pub(crate) fn v_counter_raw(line: u16, is_pal: bool, v30: bool) -> u16 {
        let (overflow, lines): (u16, u16) = match (is_pal, v30) {
            (false, false) => (0xEA, 262),
            (false, true) => (0x106, 262),
            (true, false) => (0x102, 313),
            (true, true) => (0x10A, 313),
        };
        let line = line % lines;
        if line > overflow {
            line.wrapping_sub(lines) & 0x1FF
        } else {
            line
        }
    }

    /// The externally visible 8-bit V counter for an internal line number.
    #[cfg(test)]
    pub(crate) fn v_counter_value_for_line(line: u16, is_pal: bool, v30: bool) -> u8 {
        Self::v_counter_raw(line, is_pal, v30) as u8
    }

    /// True when the HV-counter latch is enabled (reg 0 bit 1 / M3).
    pub fn hv_latch_enabled(&self) -> bool {
        (self.registers[REG_MODE1] & MODE1_HV_LATCH) != 0
    }

    /// Live HV counter derived from the current beam position. The V half
    /// changes at line-relative MCLK 0 (H 0xA5 in H40, 0x85 in H32), the same
    /// point at which the renderer starts treating writes as belonging to the
    /// new line.
    fn compute_hv_counter(&self) -> u16 {
        let h = self.h_counter();
        let v30 = (self.registers[REG_MODE2] & MODE2_V30_MODE) != 0;
        let vc = Self::v_counter_raw(self.v_counter, self.is_pal, v30);
        let v = self.interlace_v_counter(vc);
        ((v as u16) << 8) | (h as u16)
    }

    /// Apply the interlace transformation to a 9-bit V counter: in interlace
    /// mode 2 the counter is doubled, and in either interlace mode bit 0 is
    /// replaced by bit 8.
    #[inline]
    fn interlace_v_counter(&self, vc: u16) -> u8 {
        if !self.interlaced() {
            return vc as u8;
        }
        let vc = if self.interlace_mode_2() { vc << 1 } else { vc };
        ((vc & !1) | ((vc >> 8) & 1)) as u8
    }

    pub fn read_hv_counter(&self) -> u16 {
        // While latching is enabled, reads return the value captured when M3
        // was set (or at the last light-gun TH edge); otherwise the beam.
        match self.hv_latched {
            Some(v) if self.hv_latch_enabled() => v,
            _ => self.compute_hv_counter(),
        }
    }

    /// Capture the HV counter when a device drives the TH input (a light
    /// gun seeing the beam, Genesis Plus GX `lightgun.c`). Only latches while
    /// latching is enabled (reg 0 bit 1); otherwise a no-op. The 68k writing
    /// TH through the I/O ports never calls this, and no light gun is
    /// emulated yet; setting M3 latches the counter through `write_register`.
    pub fn latch_hv_counter(&mut self) {
        if self.hv_latch_enabled() {
            self.hv_latched = Some(self.compute_hv_counter());
        }
    }

    pub(crate) fn sync_sat_cache(&mut self) {
        let base = self.sprite_table_address();
        for (i, byte) in self.sat.iter_mut().enumerate() {
            *byte = self.vram[(base + i) & 0xFFFF];
        }
        self.sat_cache_valid = true;
    }

    /// Load the SAT cache from VRAM if it has never been loaded (fresh or
    /// restored VDP, or tests that build VRAM directly). Afterwards it is
    /// maintained by `write_vram_byte` only.
    pub(crate) fn ensure_sat_cache(&mut self) {
        if !self.sat_cache_valid {
            self.sync_sat_cache();
        }
    }

    /// The already-rendered line and first pixel that a state change made at
    /// the current beam position can still affect, if any.
    ///
    /// * `[0, tail_mclk)`: the last pixels of the previous line are being
    ///   emitted, so the change affects that line from the current pixel.
    /// * `[tail_mclk, active_start)`: blanking; the current line has not been
    ///   drawn yet and will pick the change up when it renders.
    /// * `[active_start, 3420)`: the current line is being emitted.
    pub(crate) fn beam_target(&self) -> Option<(u16, u16)> {
        let t = self.line_timing();
        let m = self.mclk_line_clocks;
        let (line, x) = if m < t.tail_mclk {
            if self.v_counter == 0 {
                return None;
            }
            (
                self.v_counter - 1,
                t.pixels_in_line + (m / t.mclk_per_pixel) as u16,
            )
        } else if m >= t.active_start {
            (
                self.v_counter,
                ((m - t.active_start) / t.mclk_per_pixel).min(u16::MAX as u32) as u16,
            )
        } else {
            return None;
        };
        let width = self.screen_width();
        if line >= self.screen_height() || !self.rendered_scanlines[line as usize] || x >= width {
            return None;
        }
        Some((line, x))
    }

    /// Pixel position the beam is emitting on the current line (0 before
    /// active display starts, the screen width once the line is complete).
    #[cfg(test)]
    pub(crate) fn mid_line_pixel_x(&self) -> u16 {
        let t = self.line_timing();
        let width = self.screen_width();
        if self.mclk_line_clocks < t.active_start {
            return 0;
        }
        let x = (self.mclk_line_clocks - t.active_start) / t.mclk_per_pixel;
        (x as u16).min(width)
    }

    /// Mid-line CRAM/VSRAM/register write hook: pixels the beam has already
    /// emitted keep the state they were drawn with; pixels from the current
    /// beam position to the end of the affected line are redrawn with the new
    /// state.
    pub(crate) fn redraw_current_scanline_if_visible(&mut self) {
        if let Some((line, x)) = self.beam_target() {
            self.render_line_from(line, x);
        }
    }

    /// Like `redraw_current_scanline_if_visible`, for changes that only
    /// affect colours (CRAM, backdrop, column masking): the line's layers are
    /// reused and only recomposited from the beam position.
    pub(crate) fn recomposite_current_scanline_if_visible(&mut self) {
        if let Some((line, x)) = self.beam_target() {
            self.recomposite_line_from(line, x);
        }
    }

    fn render_scanline_if_needed(&mut self, line: u16) {
        if line < self.screen_height() && !self.rendered_scanlines[line as usize] {
            self.render_line(line);
        }
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

    pub fn window_address(&self) -> usize {
        if self.h40_mode() {
            ((self.registers[REG_WINDOW] as usize) & 0x3C) << 10
        } else {
            ((self.registers[REG_WINDOW] as usize) & 0x3E) << 10
        }
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

    /// Clamp timing counters into range (they may come from a restored or
    /// hand-edited state).
    fn sanitize_counters(&mut self) {
        if self.mclk_line_clocks >= MCLK_PER_LINE {
            self.mclk_line_clocks %= MCLK_PER_LINE;
        }
        let lines = self.frame_lines();
        if self.v_counter >= lines {
            self.v_counter %= lines;
        }
    }

    /// Advance VDP state by N Master Clock (MCLK) cycles. Spans of any
    /// length are processed line by line, so every line wrap, render and
    /// interrupt in the span happens.
    pub fn tick<F>(&mut self, mclk: u32, mut read_bus_word: F)
    where
        F: FnMut(u32) -> u16,
    {
        self.sanitize_counters();
        let mut remaining = mclk;
        while remaining > 0 {
            let step = remaining.min(MCLK_PER_LINE - self.mclk_line_clocks);
            self.advance_within_line(step, &mut read_bus_word);
            remaining -= step;
            if self.mclk_line_clocks >= MCLK_PER_LINE {
                self.start_next_line();
            }
        }
    }

    /// Advance by `step` MCLK without crossing the end of the line, firing
    /// the events (access slots, line render, VINT) in chronological order.
    fn advance_within_line<F>(&mut self, step: u32, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        let start = self.mclk_line_clocks;
        let end = start + step;
        let t = *self.line_timing();

        // VINT (H 0x00/0x01) always precedes the start of active display.
        let events = [
            (t.vint, LineEvent::Vint),
            (t.active_start, LineEvent::Render),
        ];

        let mut cursor = start;
        for (at, event) in events {
            if start < at && at <= end {
                self.run_slots(cursor, at, read_bus_word);
                cursor = at;
                self.mclk_line_clocks = at;
                match event {
                    LineEvent::Render => self.render_scanline_if_needed(self.v_counter),
                    LineEvent::Vint => {
                        if self.v_counter == self.screen_height() {
                            self.status |= STATUS_VINT_PENDING;
                            self.z80_vint = true;
                        }
                    }
                }
            }
        }
        self.run_slots(cursor, end, read_bus_word);
        self.mclk_line_clocks = end;
    }

    /// Run the access slots whose start lies in `[from, to)`.
    fn run_slots<F>(&mut self, from: u32, to: u32, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        if to <= from {
            return;
        }
        let t = self.line_timing();
        let first = t.slot_at(from) + u32::from(t.slot_start(t.slot_at(from)) < from);
        let last = t.slot_at(to) + u32::from(t.slot_start(t.slot_at(to)) < to);
        let h40 = self.h40_mode();
        for slot in first..last.min(t.slots) {
            self.process_slot(slot, h40, read_bus_word);
        }
    }

    /// Line wrap: the V counter advances and the per-line state for the new
    /// line (VBLANK flag, line interrupt counter) updates.
    fn start_next_line(&mut self) {
        self.mclk_line_clocks -= MCLK_PER_LINE;
        let frame_lines = self.frame_lines();
        self.v_counter = (self.v_counter + 1) % frame_lines;

        let active_lines = self.screen_height();

        /* VBLANK is set when the V counter first reports the first blanking
        line and cleared when it reaches the last line of the frame (the line
        before line 0, during which line 0 is fetched). */
        if self.v_counter == active_lines {
            self.status |= STATUS_VBLANK;
            self.start_vblank();
        } else if self.v_counter == frame_lines - 1 {
            self.status &= !STATUS_VBLANK;
        }

        /* The H-int counter is decremented on every active line AND on the
        first blanking line (v_counter == active_lines); only the remaining
        blanking lines reload it. A pending HINT stays asserted until the
        68k acknowledges it. */
        if self.v_counter <= active_lines {
            if self.line_counter == 0 {
                self.line_counter = self.registers[REG_H_INT_COUNTER] as u16;
                self.hint_pending = true;
            } else {
                self.line_counter -= 1;
            }
        } else {
            self.line_counter = self.registers[REG_H_INT_COUNTER] as u16;
        }

        if self.v_counter == 0 {
            self.rendered_scanlines.fill(false);
        }
    }

    /// Per-frame work at the start of vertical blanking.
    fn start_vblank(&mut self) {
        // The odd/even field flag toggles every frame while interlaced.
        if self.interlaced() {
            self.status ^= STATUS_ODD_FRAME;
        } else {
            self.status &= !STATUS_ODD_FRAME;
        }
    }

    /// One access slot. During active display only the external slots are
    /// available; during blanking (or with the display off) every slot is,
    /// except the refresh slots. The FIFO is served before DMA. A VRAM word
    /// (FIFO entry or 68k DMA) and a copied byte take two slots each, a fill
    /// byte or a CRAM/VSRAM word one (GPGX `fifo_byte_access`,
    /// `vdp_dma_update`).
    fn process_slot<F>(&mut self, slot: u32, is_h40: bool, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        let blanking = self.blanking();
        if !timing::slot_available(slot, is_h40, blanking) {
            return;
        }

        if let Some(&entry) = self.fifo.front() {
            if Self::is_byte_access(entry.code) && !self.command.fifo_half {
                self.command.fifo_half = true;
                return;
            }
            self.command.fifo_half = false;

            /* Temporarily place the beam at this slot so a redraw triggered
            by the drained entry splits the line at the slot's pixel. */
            let saved_mclk = self.mclk_line_clocks;
            self.mclk_line_clocks = self.line_timing().slot_start(slot);

            self.fifo.pop_front();
            self.process_fifo_entry(entry);

            self.mclk_line_clocks = saved_mclk;

            self.fifo_full = false;
            self.status &= !STATUS_FIFO_FULL;
            if self.fifo.is_empty() {
                self.status |= STATUS_FIFO_EMPTY;

                // Trigger deferred prefetch if waiting
                if !self.command.cd4_flag
                    && (self.command.code & 0x01) == 0
                    && (self.command.code & 0x20) == 0
                {
                    self.try_prefetch();
                }
            }
        } else if self.command.dma_pending && !self.command.dma_fill_wait {
            if !self.dma_slot_ready(slot, is_h40, blanking) {
                return;
            }
            let saved_mclk = self.mclk_line_clocks;
            self.mclk_line_clocks = self.line_timing().slot_start(slot);
            self.step_dma(read_bus_word);
            self.mclk_line_clocks = saved_mclk;
        }
    }

    /// Whether the VDP is in vertical blanking or the display is off, so
    /// every non-refresh slot is free for external access.
    #[inline]
    fn blanking(&self) -> bool {
        (self.status & STATUS_VBLANK) != 0 || !self.display_enabled()
    }

    /// A write with this code goes to VRAM byte by byte and needs two access
    /// slots per word. Codes with CD2-CD1 clear (VRAM, and the invalid codes
    /// that behave like it) do.
    #[inline]
    fn is_byte_access(code: u8) -> bool {
        (code & 0x06) == 0
    }

    /// Account one available access slot to the running DMA and return
    /// whether a DMA unit completes in it.
    fn dma_slot_ready(&mut self, slot: u32, is_h40: bool, blanking: bool) -> bool {
        if self.command.dma_delay > 0 {
            self.command.dma_delay -= 1;
            return false;
        }
        let two_slots = match self.command.dma_kind {
            DmaKind::Copy => true,
            DmaKind::Fill => false,
            DmaKind::Bus => {
                let byte_access = Self::is_byte_access(self.command.code);
                /* A 68k DMA to CRAM or VSRAM in blanking also loses the slot
                after each refresh slot (GPGX: 161/198 instead of 166/204). */
                if !byte_access && blanking && timing::follows_refresh_slot(slot, is_h40) {
                    return false;
                }
                byte_access
            }
        };
        if two_slots && !self.command.dma_half {
            self.command.dma_half = true;
            return false;
        }
        self.command.dma_half = false;
        true
    }

    /// MCLK from the current beam position until the oldest FIFO entry has
    /// been written and a slot is free again: the time a 68k data-port write
    /// to a full FIFO waits (GPGX `vdp_68k_data_w_m5`). `after` is 68k time
    /// already owed but not yet run on the VDP; slots before it are taken.
    /// The slot pattern of the current line is assumed for later lines.
    pub fn fifo_wait_mclk(&self, after: u32) -> u32 {
        let Some(&oldest) = self.fifo.front() else {
            return 0;
        };
        let mut needed = if Self::is_byte_access(oldest.code) && !self.command.fifo_half {
            2
        } else {
            1
        };
        let t = self.line_timing();
        let h40 = self.h40_mode();
        let blanking = self.blanking();
        let now = self.mclk_line_clocks;
        let from = now + after.min(MCLK_PER_LINE);
        let lines = from / MCLK_PER_LINE;
        let pos = from % MCLK_PER_LINE;
        let first =
            lines * t.slots + t.slot_at(pos) + u32::from(t.slot_start(t.slot_at(pos)) < pos);
        // At most two lines ahead: every line has free slots.
        for slot in first..first + 2 * t.slots {
            let line_slot = slot % t.slots;
            if timing::slot_available(line_slot, h40, blanking) {
                needed -= 1;
                if needed == 0 {
                    let at = (slot / t.slots) * MCLK_PER_LINE + t.slot_start(line_slot);
                    return at.saturating_sub(now) + 1;
                }
            }
        }
        0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum LineEvent {
    Render,
    Vint,
}

impl Debuggable for Vdp {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap()
    }

    fn write_state(&mut self, state: &Value) {
        let mut new_vdp: Vdp = match Vdp::deserialize(state) {
            Ok(s) => s,
            Err(e) => {
                log::error!("Error deserializing VDP state: {}", e);
                return;
            }
        };

        // Swap framebuffer to preserve allocation
        std::mem::swap(&mut self.framebuffer, &mut new_vdp.framebuffer);

        // Reconstruct CRAM cache
        new_vdp.reconstruct_cram_cache();
        // Out-of-range counters in a restored state are clamped, not trusted.
        new_vdp.sanitize_counters();

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

#[cfg(test)]
mod tests_data_port;

#[cfg(test)]
mod tests_timeline;

#[cfg(test)]
mod tests_dma_semantics;

#[cfg(test)]
mod tests_render_rules;

#[cfg(test)]
mod tests_interlace;
