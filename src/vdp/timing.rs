//! Scanline timeline of the VDP.
//!
//! Every line is 3420 master clocks (MCLK) long. Line-relative MCLK 0 is the
//! point where the V counter increments and the line interrupt fires, which
//! on hardware is H counter 0xA5 in H40 mode and 0x85 in H32 mode (BlastEm
//! hslot 165/133, Genesis Plus GX `cycle2hc40`/`cycle2hc32`). Because that
//! point is 16 (H40) / 14 (H32) pixels before the end of active display, a
//! line's 320/256 visible pixels are split across two VDP lines:
//!
//! ```text
//!  MCLK 0 ......... tail_mclk ....... active_start ............. 3420
//!  | last pixels of |  right border,   | first pixels of this line |
//!  | previous line  |  HBlank, HSync,  |                           |
//!  |                |  left border     |                           |
//! ```
//!
//! H counter, HBLANK/VBLANK flags, interrupts, FIFO access slots and the
//! renderer all derive from this single timeline, so a write that follows a
//! V-counter change lands on the line the V counter reports.

pub(crate) const MCLK_PER_LINE: u32 = 3420;

/// Per-mode positions within a line, in line-relative MCLK.
#[derive(Debug, Clone, Copy)]
pub(crate) struct LineTiming {
    /// HBLANK status flag set from this MCLK...
    pub hblank_start: u32,
    /// ...until (exclusive) this MCLK.
    pub hblank_end: u32,
    /// VINT assertion point on the first VBlank line.
    pub vint: u32,
    /// MCLK at which pixel 0 of this line is emitted.
    pub active_start: u32,
    /// MCLK per pixel.
    pub mclk_per_pixel: u32,
    /// Pixels emitted between `active_start` and the end of the line.
    pub pixels_in_line: u16,
    /// The remaining pixels of the previous line are emitted in [0, tail_mclk).
    pub tail_mclk: u32,
    /// Number of 2-pixel access slots per line.
    pub slots: u32,
}

pub(crate) const H40_TIMING: LineTiming = LineTiming {
    hblank_start: 228,
    hblank_end: 872,
    vint: 788,
    active_start: 988,
    mclk_per_pixel: 8,
    pixels_in_line: 304,
    tail_mclk: 128,
    slots: 210,
};

pub(crate) const H32_TIMING: LineTiming = LineTiming {
    hblank_start: 280,
    hblank_end: 860,
    vint: 770,
    active_start: 1000,
    mclk_per_pixel: 10,
    pixels_in_line: 242,
    tail_mclk: 140,
    slots: 171,
};

/// External (CPU/DMA) access slot positions in line-relative MCLK while the
/// display is active (Genesis Plus GX FIFO timing tables).
pub(crate) const H40_EXTERNAL_SLOT_MCLK: [u32; 18] = [
    352, 820, 948, 1076, 1332, 1460, 1588, 1844, 1972, 2100, 2356, 2484, 2612, 2868, 2996, 3124,
    3364, 3380,
];
pub(crate) const H32_EXTERNAL_SLOT_MCLK: [u32; 16] = [
    230, 510, 810, 970, 1130, 1450, 1610, 1770, 2090, 2250, 2410, 2730, 2890, 3050, 3350, 3370,
];

/// Start MCLK of H counter values 0xE6..=0xF7 in H40 mode. During HSYNC the
/// H40 dot clock (EDCLK) slows down, so these steps are 17-20 MCLK long
/// instead of the usual 16.
const H40_HSYNC_STEPS: [u32; 18] = [
    304, 321, 341, 361, 381, 399, 419, 439, 459, 478, 497, 517, 537, 557, 575, 595, 615, 635,
];

impl LineTiming {
    pub(crate) fn for_mode(h40: bool) -> &'static LineTiming {
        if h40 {
            &H40_TIMING
        } else {
            &H32_TIMING
        }
    }

    /// Access slot index containing line-relative `mclk`.
    #[inline]
    pub(crate) fn slot_at(&self, mclk: u32) -> u32 {
        (mclk.min(MCLK_PER_LINE) * self.slots) / MCLK_PER_LINE
    }

    /// First MCLK of access slot `slot`.
    #[inline]
    pub(crate) fn slot_start(&self, slot: u32) -> u32 {
        (slot * MCLK_PER_LINE).div_ceil(self.slots)
    }
}

const fn build_external_slots(table: &[u32], slots: u32) -> [bool; 256] {
    let mut out = [false; 256];
    let mut i = 0;
    while i < table.len() {
        out[((table[i] * slots) / MCLK_PER_LINE) as usize] = true;
        i += 1;
    }
    out
}

const H40_EXTERNAL_SLOTS: [bool; 256] =
    build_external_slots(&H40_EXTERNAL_SLOT_MCLK, H40_TIMING.slots);
const H32_EXTERNAL_SLOTS: [bool; 256] =
    build_external_slots(&H32_EXTERNAL_SLOT_MCLK, H32_TIMING.slots);

/// Whether access slot `slot` is available to the CPU/DMA during active
/// display.
#[inline]
pub(crate) fn is_external_slot(slot: u32, h40: bool) -> bool {
    let table = if h40 {
        &H40_EXTERNAL_SLOTS
    } else {
        &H32_EXTERNAL_SLOTS
    };
    table.get(slot as usize).copied().unwrap_or(false)
}

/// Refresh slots per line are spread evenly: one in every
/// `REFRESH_PERIOD_*` slots. That leaves 204 (H40) / 166 (H32) of the
/// 210 / 171 slots for external access during blanking, the Genesis Plus GX
/// `dma_timing` blank rates. Their exact positions are not modelled.
const REFRESH_PERIOD_H40: u32 = 35;
const REFRESH_PERIOD_H32: u32 = 34;

/// Whether access slot `slot` is a DRAM refresh slot during blanking.
#[inline]
pub(crate) fn is_refresh_slot(slot: u32, h40: bool) -> bool {
    let period = if h40 {
        REFRESH_PERIOD_H40
    } else {
        REFRESH_PERIOD_H32
    };
    slot % period == period - 1
}

/// Whether the slot before `slot` (wrapping to the previous line) is a
/// refresh slot.
#[inline]
pub(crate) fn follows_refresh_slot(slot: u32, h40: bool) -> bool {
    let slots = LineTiming::for_mode(h40).slots;
    is_refresh_slot((slot + slots - 1) % slots, h40)
}

/// Whether access slot `slot` can serve the FIFO or DMA: an external slot
/// during active display, any slot but a refresh slot during blanking.
#[inline]
pub(crate) fn slot_available(slot: u32, h40: bool, blanking: bool) -> bool {
    if blanking {
        !is_refresh_slot(slot, h40)
    } else {
        is_external_slot(slot, h40)
    }
}

/// Externally visible 8-bit H counter at line-relative `mclk`.
///
/// H40: 0xA5..=0xB6, 0xE4..=0xFF, 0x00..=0xA4 (211 values; 0xB6 and 0xE4
/// last half a step). H32: 0x85..=0x93, 0xE9..=0xFF, 0x00..=0x84 (171
/// values of 20 MCLK each).
pub(crate) fn h_counter_at(mclk: u32, h40: bool) -> u8 {
    let m = mclk % MCLK_PER_LINE;
    if h40 {
        if m < 272 {
            0xA5 + (m / 16) as u8
        } else if m < 280 {
            0xB6
        } else if m < 288 {
            0xE4
        } else if m < 304 {
            0xE5
        } else if m < 652 {
            let step = H40_HSYNC_STEPS.iter().rposition(|&s| s <= m).unwrap_or(0);
            0xE6 + step as u8
        } else {
            0xF8u8.wrapping_add(((m - 652) / 16) as u8)
        }
    } else {
        let idx = m / 20;
        if idx < 15 {
            0x85 + idx as u8
        } else if idx < 38 {
            0xE9 + (idx - 15) as u8
        } else {
            (idx - 38) as u8
        }
    }
}
