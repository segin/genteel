//! Cartridge-side memory: battery-backed SRAM and the standard Sega ROM
//! bank mapper.
//!
//! ## SRAM
//!
//! The ROM header describes external RAM at offset `0x1B0`:
//!
//! ```text
//! 1B0h: dc.b 'RA', %1x1yz000, %abc00000
//! 1B4h: dc.l RAM start address
//! 1B8h: dc.l RAM end address
//!   yz = 10 even addresses only, 11 odd addresses only, 00 both
//! ```
//!
//! Most 8-bit SRAM chips sit on one byte lane only (Sonic 3: `RA F8 20`,
//! odd bytes at `$200001-$203FFF`), so the buffer is indexed by the CPU
//! address offset from the (even-aligned) window base. As in Genesis Plus
//! GX `sram.c`, every byte address in the window is backed; the lane type is
//! recorded (it identifies legacy save files) but doesn't gate accesses.
//!
//! ### `.srm` layout
//!
//! The on-disk image matches Genesis Plus GX: a buffer of up to 64 KiB
//! indexed by `address & 0xFFFF`, so a byte at CPU address `A` is stored at
//! file offset `A & 0xFFFF`. Bytes on an unused lane stay `0xFF`, and the
//! image is truncated after its last non-`0xFF` byte. Older genteel builds
//! stored only even-address bytes, packed one per file byte and zero-filled;
//! [`Sram::load_image`] recognises the two such files that can be told apart
//! (an all-zero file, and a packed file for even-lane SRAM) and converts
//! them.
//!
//! ## Sega mapper
//!
//! Cartridges larger than 4 MiB (Super Street Fighter II) page 512 KiB ROM
//! banks into the eight slots of `$000000-$3FFFFF` through the odd bytes
//! `$A130F3-$A130FF`; slot 0 is fixed so the vector table never moves.

use serde::{Deserialize, Serialize};

/// Largest SRAM window accepted from a ROM header (64 KiB, as in Genesis
/// Plus GX `sram_init`).
pub const MAX_SRAM_SIZE: usize = 0x10000;

/// Size of the on-disk SRAM image page (`address & 0xFFFF`).
const SRAM_IMAGE_PAGE: usize = 0x10000;

/// Default SRAM window used when the header does not describe one.
const DEFAULT_SRAM_START: u32 = 0x200000;
const DEFAULT_SRAM_END: u32 = 0x20FFFF;

/// ROM size above which the cartridge window can no longer hold the whole
/// image and the Sega bank mapper is assumed.
pub const SEGA_MAPPER_THRESHOLD: usize = 0x400000;

/// Size of one Sega mapper bank (512 KiB).
const MAPPER_BANK_SIZE: usize = 0x80000;

/// Which data-bus byte lanes an SRAM chip is wired to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum SramLanes {
    /// 16-bit SRAM: both even and odd addresses are storage.
    #[default]
    Both,
    /// 8-bit SRAM on D15-D8: only even addresses are storage.
    Even,
    /// 8-bit SRAM on D7-D0: only odd addresses are storage.
    Odd,
}

impl SramLanes {
    /// Decode the header RAM type byte (`0x1B2`, bits 4-3 = `yz`).
    pub fn from_header_type(byte: u8) -> Self {
        match (byte >> 3) & 0x03 {
            0b10 => SramLanes::Even,
            0b11 => SramLanes::Odd,
            _ => SramLanes::Both,
        }
    }
}

/// Battery-backed cartridge RAM.
///
/// Field names are serialized with their historical flat `Bus` keys so save
/// states written before this type existed still load.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Sram {
    /// Storage, indexed by `address - base()`.
    #[serde(rename = "sram")]
    pub data: Box<[u8]>,
    /// SRAM is currently decoded in the cartridge window.
    #[serde(rename = "sram_enabled")]
    pub mapped: bool,
    /// First CPU address of the window (as given by the header).
    #[serde(rename = "sram_start")]
    pub start: u32,
    /// Last CPU address of the window (inclusive).
    #[serde(rename = "sram_end")]
    pub end: u32,
    /// Byte lanes the chip is wired to.
    #[serde(rename = "sram_lanes", default)]
    pub lanes: SramLanes,
    /// `$A130F1` bit 1: writes are discarded while set.
    #[serde(rename = "sram_write_protect", default)]
    pub write_protect: bool,
    /// The cartridge has SRAM that `$A130F1` can map in.
    #[serde(rename = "sram_present", default)]
    pub present: bool,
    /// Mapping state applied at power-on and on reset.
    #[serde(rename = "sram_power_on_mapped", default)]
    pub power_on_mapped: bool,
}

impl Default for Sram {
    fn default() -> Self {
        Self::new()
    }
}

impl Sram {
    /// An absent 64 KiB SRAM at the default window.
    pub fn new() -> Self {
        Self {
            data: vec![0xFF; MAX_SRAM_SIZE].into_boxed_slice(),
            mapped: false,
            start: DEFAULT_SRAM_START,
            end: DEFAULT_SRAM_END,
            lanes: SramLanes::Both,
            write_protect: false,
            present: false,
            power_on_mapped: false,
        }
    }

    /// Configure SRAM for a freshly loaded ROM from its header.
    ///
    /// Header address quirks follow Genesis Plus GX: a start inside work RAM
    /// means no SRAM, a start beyond `$7FFFFF` falls back to the default
    /// window, and an inverted or oversized window is clamped to 64 KiB.
    /// Without a header, ROMs of up to 2 MiB get a 64 KiB SRAM at
    /// `$200000`, mapped from power-on (Genesis Plus GX `sram_init` default).
    pub fn configure_from_rom(rom: &[u8]) -> Self {
        let mut sram = Self::new();
        let has_header = rom.len() >= 0x1BC && &rom[0x1B0..0x1B2] == b"RA";
        if !has_header {
            sram.present = rom.len() <= 0x200000;
            sram.power_on_mapped = sram.present;
            sram.mapped = sram.present;
            return sram;
        }

        let be32 = |o: usize| u32::from_be_bytes([rom[o], rom[o + 1], rom[o + 2], rom[o + 3]]);
        let mut start = be32(0x1B4) & 0xFFFFFF;
        let mut end = be32(0x1B8) & 0xFFFFFF;
        sram.lanes = SramLanes::from_header_type(rom[0x1B2]);

        if start == 0xFF0000 {
            // Header points at work RAM: there is no external RAM.
            return sram;
        }
        if start >= 0x800000 {
            start = DEFAULT_SRAM_START;
            end = DEFAULT_SRAM_END;
        } else if start > end || (end as u64 - start as u64) >= MAX_SRAM_SIZE as u64 {
            end = start + (MAX_SRAM_SIZE as u32 - 1);
        }

        sram.start = start;
        sram.end = end;
        let size = (end as u64 - (start & !1) as u64 + 1) as usize;
        sram.data = vec![0xFF; size].into_boxed_slice();
        sram.present = true;
        // SRAM overlapping the ROM image stays hidden until $A130F1 maps it.
        sram.power_on_mapped = (start & !1) as usize >= rom.len();
        sram.mapped = sram.power_on_mapped;
        sram
    }

    /// Even-aligned first address of the window (buffer index 0).
    #[inline]
    pub fn base(&self) -> u32 {
        self.start & !1
    }

    /// True when SRAM is mapped and decodes `addr`.
    #[inline]
    pub fn contains(&self, addr: u32) -> bool {
        self.mapped && addr >= self.base() && addr <= self.end
    }

    /// Read a byte. Every byte address in the window is backed, as in
    /// Genesis Plus GX `sram.c`; bytes on a lane the chip doesn't use simply
    /// keep their `0xFF` fill unless software writes them.
    #[inline]
    pub fn read_byte(&self, addr: u32) -> u8 {
        let idx = addr.wrapping_sub(self.base()) as usize;
        self.data.get(idx).copied().unwrap_or(0xFF)
    }

    /// Write a byte unless write-protected.
    #[inline]
    pub fn write_byte(&mut self, addr: u32, value: u8) {
        if self.write_protect {
            return;
        }
        let idx = addr.wrapping_sub(self.base()) as usize;
        if let Some(cell) = self.data.get_mut(idx) {
            *cell = value;
        }
    }

    /// Handle a `$A130F1` write: bit 0 maps SRAM over ROM, bit 1 write
    /// protects it. Cartridges without SRAM ignore the register.
    pub fn write_control(&mut self, value: u8) {
        if self.present {
            self.mapped = value & 0x01 != 0;
            self.write_protect = value & 0x02 != 0;
        }
    }

    /// Return the mapping to its power-on state. Contents are kept: they are
    /// battery backed.
    pub fn reset(&mut self) {
        self.mapped = self.power_on_mapped;
        self.write_protect = false;
    }

    /// Offset of buffer index 0 in the on-disk image.
    fn image_offset(&self) -> usize {
        (self.base() as usize) & (SRAM_IMAGE_PAGE - 1)
    }

    /// The `.srm` image (Genesis Plus GX layout), truncated after its last
    /// non-`0xFF` byte. Empty when the SRAM has never been written.
    pub fn image(&self) -> Vec<u8> {
        if !self.present {
            return Vec::new();
        }
        let mut image = vec![0xFF; SRAM_IMAGE_PAGE];
        let off = self.image_offset();
        for (i, &b) in self.data.iter().enumerate().take(SRAM_IMAGE_PAGE) {
            image[(off + i) & (SRAM_IMAGE_PAGE - 1)] = b;
        }
        let used = image.iter().rposition(|&b| b != 0xFF).map_or(0, |p| p + 1);
        image.truncate(used);
        image
    }

    /// Load a `.srm` image. Missing trailing bytes read as `0xFF`.
    ///
    /// Files written by older genteel builds are exactly as long as the SRAM
    /// window and are handled as follows: an all-zero file never held a save
    /// (its SRAM writes were dropped) and is ignored; a packed even-address
    /// file for even-lane SRAM is detected by data on the odd lane, which a
    /// real image never has, and expanded.
    pub fn load_image(&mut self, image: &[u8]) {
        let legacy_sized = image.len() == self.data.len();
        if image.is_empty() || (legacy_sized && image.iter().all(|&b| b == 0)) {
            return;
        }
        if legacy_sized && self.is_legacy_even_lane_image(image) {
            self.data.fill(0xFF);
            for (k, &b) in image.iter().enumerate() {
                if let Some(cell) = self.data.get_mut(k * 2) {
                    *cell = b;
                }
            }
            return;
        }
        let off = self.image_offset();
        for (i, cell) in self.data.iter_mut().enumerate().take(SRAM_IMAGE_PAGE) {
            *cell = image
                .get((off + i) & (SRAM_IMAGE_PAGE - 1))
                .copied()
                .unwrap_or(0xFF);
        }
    }

    fn is_legacy_even_lane_image(&self, image: &[u8]) -> bool {
        if self.lanes != SramLanes::Even || self.image_offset() != 0 {
            return false;
        }
        image
            .iter()
            .take(self.data.len())
            .skip(1)
            .step_by(2)
            .any(|&b| b != 0xFF)
    }
}

/// Standard Sega ROM bank mapper (`$A130F3-$A130FF`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SegaMapper {
    /// Active for ROMs larger than 4 MiB. Derived from the ROM at load
    /// time, so not serialized.
    #[serde(skip)]
    pub enabled: bool,
    /// 512 KiB bank selected for each slot of the cartridge window.
    #[serde(default = "default_mapper_banks")]
    pub banks: [u8; 8],
}

fn default_mapper_banks() -> [u8; 8] {
    [0, 1, 2, 3, 4, 5, 6, 7]
}

impl Default for SegaMapper {
    fn default() -> Self {
        Self {
            enabled: false,
            banks: default_mapper_banks(),
        }
    }
}

impl SegaMapper {
    /// True for a register address handled by the mapper (odd bytes of
    /// `$A130F3-$A130FF`; `$A130F1` is the SRAM control register).
    #[inline]
    pub fn is_bank_register(addr: u32) -> bool {
        (0xA130F3..=0xA130FF).contains(&addr) && addr & 1 == 1
    }

    /// Select the bank for the slot addressed by a bank register write.
    pub fn write_register(&mut self, addr: u32, value: u8) {
        let slot = ((addr >> 1) & 0x07) as usize;
        if slot != 0 {
            self.banks[slot] = value;
        }
    }

    /// Restore the linear power-on mapping.
    pub fn reset(&mut self) {
        self.banks = default_mapper_banks();
    }

    /// ROM image offset for a CPU address in the cartridge window.
    #[inline]
    pub fn rom_offset(&self, addr: u32) -> usize {
        if !self.enabled || addr > 0x3FFFFF {
            return addr as usize;
        }
        let slot = ((addr >> 19) & 0x07) as usize;
        (self.banks[slot] as usize) * MAPPER_BANK_SIZE + (addr as usize & (MAPPER_BANK_SIZE - 1))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use proptest::prelude::*;

    fn rom_with_sram_header(len: usize, kind: u8, start: u32, end: u32) -> Vec<u8> {
        let mut rom = vec![0u8; len];
        rom[0x1B0] = b'R';
        rom[0x1B1] = b'A';
        rom[0x1B2] = kind;
        rom[0x1B3] = 0x20;
        rom[0x1B4..0x1B8].copy_from_slice(&start.to_be_bytes());
        rom[0x1B8..0x1BC].copy_from_slice(&end.to_be_bytes());
        rom
    }

    #[test]
    fn lane_decoding_follows_header_type_byte() {
        assert_eq!(SramLanes::from_header_type(0xF8), SramLanes::Odd);
        assert_eq!(SramLanes::from_header_type(0xB8), SramLanes::Odd);
        assert_eq!(SramLanes::from_header_type(0xF0), SramLanes::Even);
        assert_eq!(SramLanes::from_header_type(0xE0), SramLanes::Both);
        assert_eq!(SramLanes::from_header_type(0xE8), SramLanes::Both);
    }

    #[test]
    fn header_with_full_address_range_does_not_overflow() {
        let rom = rom_with_sram_header(0x1000, 0xE0, 0, 0xFFFF_FFFF);
        let sram = Sram::configure_from_rom(&rom);
        assert!(sram.present);
        assert_eq!(sram.data.len(), MAX_SRAM_SIZE);
        assert_eq!(sram.end, 0xFFFF);
    }

    #[test]
    fn header_pointing_at_work_ram_means_no_sram() {
        let rom = rom_with_sram_header(0x1000, 0xE0, 0xFF0000, 0xFFFFFF);
        assert!(!Sram::configure_from_rom(&rom).present);
    }

    #[test]
    fn header_start_beyond_cartridge_space_uses_default_window() {
        let rom = rom_with_sram_header(0x1000, 0xE0, 0x900000, 0x90FFFF);
        let sram = Sram::configure_from_rom(&rom);
        assert_eq!(
            (sram.start, sram.end),
            (DEFAULT_SRAM_START, DEFAULT_SRAM_END)
        );
    }

    #[test]
    fn inverted_header_window_is_clamped() {
        let rom = rom_with_sram_header(0x1000, 0xE0, 0x200001, 0x200000);
        let sram = Sram::configure_from_rom(&rom);
        assert_eq!(sram.end, 0x200001 + 0xFFFF);
    }

    #[test]
    fn image_matches_genesis_plus_gx_layout_for_odd_lane() {
        let rom = rom_with_sram_header(0x1000, 0xF8, 0x200001, 0x203FFF);
        let mut sram = Sram::configure_from_rom(&rom);
        sram.write_byte(0x200001, 0x11);
        sram.write_byte(0x200005, 0x55);
        let image = sram.image();
        assert_eq!(image, vec![0xFF, 0x11, 0xFF, 0xFF, 0xFF, 0x55]);

        let mut fresh = Sram::configure_from_rom(&rom);
        fresh.load_image(&image);
        assert_eq!(fresh.read_byte(0x200001), 0x11);
        assert_eq!(fresh.read_byte(0x200005), 0x55);
        assert_eq!(fresh.read_byte(0x200003), 0xFF);
    }

    #[test]
    fn unwritten_sram_has_empty_image() {
        let rom = rom_with_sram_header(0x1000, 0xE0, 0x200000, 0x20FFFF);
        assert!(Sram::configure_from_rom(&rom).image().is_empty());
    }

    #[test]
    fn all_zero_legacy_image_is_ignored() {
        let rom = rom_with_sram_header(0x1000, 0xF8, 0x200001, 0x203FFF);
        let mut sram = Sram::configure_from_rom(&rom);
        sram.load_image(&vec![0u8; 0x4000]);
        assert_eq!(sram.read_byte(0x200001), 0xFF);
    }

    #[test]
    fn legacy_packed_even_lane_image_is_expanded() {
        let rom = rom_with_sram_header(0x1000, 0xF0, 0x200000, 0x203FFE);
        let mut sram = Sram::configure_from_rom(&rom);
        let mut legacy = vec![0u8; sram.data.len()];
        legacy[0] = 0xAB;
        legacy[1] = 0xCD;
        sram.load_image(&legacy);
        assert_eq!(sram.read_byte(0x200000), 0xAB);
        assert_eq!(sram.read_byte(0x200002), 0xCD);
        assert_eq!(sram.read_byte(0x200004), 0x00);
    }

    #[test]
    fn write_control_ignored_without_sram() {
        let mut sram = Sram::new();
        sram.write_control(0x01);
        assert!(!sram.mapped);
    }

    #[test]
    fn mapper_slot_zero_is_fixed() {
        let mut m = SegaMapper {
            enabled: true,
            ..Default::default()
        };
        assert!(!SegaMapper::is_bank_register(0xA130F1));
        m.write_register(0xA130F3, 9);
        assert_eq!(m.rom_offset(0x080010), 9 * MAPPER_BANK_SIZE + 0x10);
        assert_eq!(m.rom_offset(0x000010), 0x10);
    }

    proptest! {
        #![proptest_config(proptest::prelude::ProptestConfig::with_cases(256))]
        #[test]
        fn prop_image_round_trip(
            kind in prop::sample::select(vec![0xE0u8, 0xF0, 0xF8]),
            writes in prop::collection::vec((0u32..0x4000, any::<u8>()), 0..64),
        ) {
            let rom = rom_with_sram_header(0x1000, kind, 0x200000, 0x203FFF);
            let mut sram = Sram::configure_from_rom(&rom);
            for (off, v) in &writes {
                sram.write_byte(0x200000 + off, *v);
            }
            let mut reloaded = Sram::configure_from_rom(&rom);
            let image = sram.image();
            reloaded.load_image(&image);
            /* The legacy heuristics only fire for an image exactly as long as
            the window; skip that rare coincidence. */
            if image.len() != sram.data.len() {
                prop_assert_eq!(&reloaded.data, &sram.data);
            }
        }

        #[test]
        fn prop_window_byte_round_trip(addr in 0x1FFFF0u32..0x204010, v in any::<u8>()) {
            let rom = rom_with_sram_header(0x1000, 0xF8, 0x200001, 0x203FFF);
            let mut sram = Sram::configure_from_rom(&rom);
            sram.write_byte(addr, v);
            if (0x200000..=0x203FFF).contains(&addr) {
                prop_assert!(sram.contains(addr));
                prop_assert_eq!(sram.read_byte(addr), v);
            } else {
                prop_assert!(!sram.contains(addr));
            }
        }
    }
}
