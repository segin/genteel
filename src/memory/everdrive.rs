//! EverDrive "extended SSF" cartridge mapper (4MB RAM).
//!
//! Implements the bank-switching mapper used by the Mega EverDrive, which is an
//! extension of the Super Street Fighter II / "Sega" mapper. It is activated
//! when the ROM header system field at offset `$0100` contains `"SEGA SSF"`
//! (instead of the usual `"SEGA MEGA DRIVE"` / `"SEGA GENESIS"`).
//!
//! The `$000000-$3FFFFF` cartridge window is divided into eight 512KB slots.
//! Each slot has a 5-bit bank register selecting one of up to 32 pages (16MB) of
//! the cartridge's on-board memory. The key EverDrive extension is that this
//! memory is *writable* (the `W` bit), so a program can treat the full 4MB
//! window as RAM ("ROM memory can be used as RAM").
//!
//! Register map (write-only), within the `$A130F0-$A130FF` window:
//!
//! | Address    | Function                                              |
//! |------------|-------------------------------------------------------|
//! | `$A130F0`  | Control byte `[P X W L . . . .]` (see below)          |
//! | `$A130F1`  | Slot 0 bank (5-bit)                                    |
//! | `$A130F3`  | Slot 1 bank (5-bit)                                    |
//! | `$A130F5`  | Slot 2 bank (5-bit)                                    |
//! | `$A130F7`  | Slot 3 bank (5-bit)                                    |
//! | `$A130F9`  | Slot 4 bank (5-bit)                                    |
//! | `$A130FB`  | Slot 5 bank (5-bit)                                    |
//! | `$A130FD`  | Slot 6 bank (5-bit)                                    |
//! | `$A130FF`  | Slot 7 bank (5-bit)                                    |
//!
//! Control byte (`$A130F0`): `P` = protection latch (a new value only loads when
//! this bit is set), `X` = 32X mode, `W` = ROM write-enable (`0` = write
//! protected, `1` = writable RAM), `L` = on-board LED.
//!
//! The mapper owns the cartridge image: its backing memory *is* the ROM
//! (plus RAM past the end of it), so the bus keeps no second copy. Writes
//! made through the `W` bit are tracked in 4 KiB pages; save states carry
//! only those pages, and the original bytes of any overwritten ROM page are
//! kept so the unmodified image can be handed back for a state restore.
//!
//! References: krikzz "extended_ssf-v2" specification and Plutiedev "Beyond 4MB".

use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::BTreeMap;

/// Size of one bank / page (512 KB).
pub const BANK_SIZE: usize = 0x8_0000;
/// Number of 512KB slots covering the `$000000-$3FFFFF` window (8 * 512KB = 4MB).
pub const NUM_SLOTS: usize = 8;
/// The visible cartridge window is always 4MB.
const WINDOW_SIZE: usize = NUM_SLOTS * BANK_SIZE;
/// Granularity at which writes to backing memory are tracked for save states.
pub const RAM_PAGE_SIZE: usize = 0x1000;
/// Offset of the ROM header system field used to detect the mapper.
const HEADER_SYSTEM_OFFSET: usize = 0x100;
/// Magic string in the header that activates the mapper.
const HEADER_MAGIC: &[u8] = b"SEGA SSF";

/// Default per-slot bank mapping: slot N -> bank N, so the first 4MB maps
/// linearly at power-on (and the reset vector in slot 0 is always valid).
fn default_banks() -> [u8; NUM_SLOTS] {
    [0, 1, 2, 3, 4, 5, 6, 7]
}

/// One written page of backing memory in a save state.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SavedRamPage {
    /// Page index (byte offset / [`RAM_PAGE_SIZE`]).
    pub page: u32,
    /// The page contents as a hex string (two digits per byte).
    pub data: String,
}

/// Serialized form of the mapper: the registers, plus the written pages.
/// Every field defaults, so states written before the pages were saved
/// still load (their mapper RAM simply comes back as the bare ROM image).
#[derive(Serialize, Deserialize)]
struct MapperState {
    #[serde(default = "default_banks")]
    banks: [u8; NUM_SLOTS],
    #[serde(default)]
    write_enable: bool,
    #[serde(default)]
    led: bool,
    #[serde(default)]
    mode_32x: bool,
    #[serde(default)]
    ram_pages: Vec<SavedRamPage>,
}

/// EverDrive / SSF bank-switching mapper state.
#[derive(Debug, Clone)]
pub struct EverdriveMapper {
    /// True once a `"SEGA SSF"` ROM has activated the mapper. This is derived
    /// from the ROM at load time, so it is not part of the serialized state
    /// (it is re-established by [`Bus::restore_rom`](crate::memory::bus::Bus)
    /// after a save-state restore).
    pub enabled: bool,

    /// Per-slot 5-bit bank selectors.
    pub banks: [u8; NUM_SLOTS],

    /// `W` bit: when set, CPU writes to `$000000-$3FFFFF` land in backing memory,
    /// turning the window into RAM. Write-protected (`false`) at power-on.
    pub write_enable: bool,

    /// `L` bit: on-board LED state (cosmetic; tracked for completeness).
    pub led: bool,

    /// `X` bit: 32X passthrough flag (tracked for completeness).
    pub mode_32x: bool,

    /// Backing memory: the ROM image occupies the low `rom_len` bytes and
    /// the remainder is usable as RAM.
    pub mem: Vec<u8>,

    /// Length of the ROM image at the start of `mem`.
    rom_len: usize,

    /// Pages of `mem` written since the ROM was loaded.
    dirty: Vec<bool>,

    /// Original contents of ROM pages that have been overwritten, so the
    /// unmodified image can be recovered.
    rom_originals: BTreeMap<usize, Box<[u8]>>,

    /// Written pages from a deserialized state, applied by the next
    /// [`load_rom`](Self::load_rom).
    pending_pages: Vec<SavedRamPage>,
}

impl Default for EverdriveMapper {
    fn default() -> Self {
        Self::new()
    }
}

impl Serialize for EverdriveMapper {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        MapperState {
            banks: self.banks,
            write_enable: self.write_enable,
            led: self.led,
            mode_32x: self.mode_32x,
            ram_pages: self.written_pages(),
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for EverdriveMapper {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let state = MapperState::deserialize(deserializer)?;
        Ok(Self {
            banks: state.banks.map(|b| b & 0x1F),
            write_enable: state.write_enable,
            led: state.led,
            mode_32x: state.mode_32x,
            pending_pages: state.ram_pages,
            ..Self::new()
        })
    }
}

impl EverdriveMapper {
    /// Create an inactive mapper (used for normal, non-SSF cartridges).
    pub fn new() -> Self {
        Self {
            enabled: false,
            banks: default_banks(),
            write_enable: false,
            led: false,
            mode_32x: false,
            mem: Vec::new(),
            rom_len: 0,
            dirty: Vec::new(),
            rom_originals: BTreeMap::new(),
            pending_pages: Vec::new(),
        }
    }

    /// Returns true if the ROM header selects the EverDrive / SSF mapper.
    pub fn rom_uses_mapper(rom: &[u8]) -> bool {
        rom.len() >= HEADER_SYSTEM_OFFSET + HEADER_MAGIC.len()
            && &rom[HEADER_SYSTEM_OFFSET..HEADER_SYSTEM_OFFSET + HEADER_MAGIC.len()] == HEADER_MAGIC
    }

    /// Activate the mapper for a freshly loaded ROM. The image becomes the
    /// backing memory itself, grown to at least the full 4MB window and
    /// rounded up to a 512KB bank. Bytes past the ROM start out as open bus
    /// (`0xFF`) and become RAM once the `W` bit is set.
    ///
    /// The register state (banks / `W` / LED) is intentionally left untouched so
    /// that it survives a save-state restore, which loads the ROM *after*
    /// deserializing the register values. Written pages from that state are
    /// applied here.
    pub fn load_rom(&mut self, rom: impl Into<Vec<u8>>) {
        let mut mem = rom.into();
        self.rom_len = mem.len();
        let size = mem.len().max(WINDOW_SIZE).div_ceil(BANK_SIZE) * BANK_SIZE;
        mem.resize(size, 0xFF);
        self.mem = mem;
        self.dirty = vec![false; size / RAM_PAGE_SIZE];
        self.rom_originals.clear();
        self.enabled = true;
        for saved in std::mem::take(&mut self.pending_pages) {
            if !self.apply_saved_page(&saved) {
                log::warn!(
                    "Ignoring invalid mapper RAM page {} in save state",
                    saved.page
                );
            }
        }
    }

    /// Deactivate the mapper and release its backing memory (non-SSF ROM).
    pub fn disable(&mut self) {
        self.enabled = false;
        self.mem = Vec::new();
        self.rom_len = 0;
        self.dirty = Vec::new();
        self.rom_originals.clear();
        self.pending_pages.clear();
    }

    /// The ROM image as the CPU currently sees it (including any bytes
    /// overwritten through the `W` bit).
    pub fn rom_image(&self) -> &[u8] {
        &self.mem[..self.rom_len.min(self.mem.len())]
    }

    /// Take the unmodified ROM image out of the mapper, leaving it disabled.
    /// Overwritten ROM pages are restored to their original bytes first.
    pub fn take_rom(&mut self) -> Vec<u8> {
        let mut mem = std::mem::take(&mut self.mem);
        for (&page, original) in &self.rom_originals {
            let start = page * RAM_PAGE_SIZE;
            mem[start..start + original.len()].copy_from_slice(original);
        }
        mem.truncate(self.rom_len);
        self.disable();
        mem
    }

    /// Reset the register state to power-on defaults. The backing memory (which
    /// holds the ROM image) is left intact, matching PSRAM persistence across a
    /// soft reset.
    pub fn reset_registers(&mut self) {
        self.banks = default_banks();
        self.write_enable = false;
        self.led = false;
        self.mode_32x = false;
    }

    /// Map a CPU address in `$000000-$3FFFFF` to an offset in backing memory.
    #[inline]
    fn device_offset(&self, addr: u32) -> usize {
        let slot = ((addr >> 19) & 0x7) as usize; // 512KB slot (0..=7)
        let bank = self.banks[slot] as usize; // 5-bit page selector
        bank * BANK_SIZE + (addr as usize & (BANK_SIZE - 1))
    }

    /// Read a byte from the banked window. An out-of-range bank reads as open
    /// bus (`0xFF`).
    #[inline]
    pub fn read(&self, addr: u32) -> u8 {
        self.mem
            .get(self.device_offset(addr))
            .copied()
            .unwrap_or(0xFF)
    }

    /// Write a byte to the banked window. Ignored unless the `W` bit is set;
    /// bounds-checked so an out-of-range bank is a silent no-op.
    #[inline]
    pub fn write(&mut self, addr: u32, value: u8) {
        if !self.write_enable {
            return;
        }
        let off = self.device_offset(addr);
        if off < self.mem.len() {
            self.mark_written(off / RAM_PAGE_SIZE);
            self.mem[off] = value;
        }
    }

    /// Record that `page` is about to change, keeping its original bytes
    /// the first time a ROM page is overwritten.
    fn mark_written(&mut self, page: usize) {
        if self.dirty[page] {
            return;
        }
        self.dirty[page] = true;
        let start = page * RAM_PAGE_SIZE;
        if start < self.rom_len {
            let original = self.mem[start..start + RAM_PAGE_SIZE].into();
            self.rom_originals.insert(page, original);
        }
    }

    /// Written pages whose contents differ from what loading the ROM gives.
    fn written_pages(&self) -> Vec<SavedRamPage> {
        let mut pages = Vec::new();
        for (page, _) in self.dirty.iter().enumerate().filter(|(_, &d)| d) {
            let start = page * RAM_PAGE_SIZE;
            let data = &self.mem[start..start + RAM_PAGE_SIZE];
            let unchanged = match self.rom_originals.get(&page) {
                Some(original) => **original == *data,
                None => data.iter().all(|&b| b == 0xFF),
            };
            if !unchanged {
                pages.push(SavedRamPage {
                    page: page as u32,
                    data: encode_hex(data),
                });
            }
        }
        pages
    }

    /// Write a saved page into backing memory. Returns false (and changes
    /// nothing) if the page is out of range or malformed.
    fn apply_saved_page(&mut self, saved: &SavedRamPage) -> bool {
        let page = saved.page as usize;
        if page >= self.dirty.len() {
            return false;
        }
        let Some(data) = decode_hex(&saved.data, RAM_PAGE_SIZE) else {
            return false;
        };
        self.mark_written(page);
        let start = page * RAM_PAGE_SIZE;
        self.mem[start..start + RAM_PAGE_SIZE].copy_from_slice(&data);
        true
    }

    /// Returns true for an address in the `$A130F0-$A130FF` register window.
    #[inline]
    pub fn is_register(addr: u32) -> bool {
        (0xA130F0..=0xA130FF).contains(&addr)
    }

    /// Handle a byte write to a mapper control / bank register.
    pub fn write_register(&mut self, addr: u32, value: u8) {
        match addr & 0xF {
            // $A130F0: control byte [P X W L . . . .].
            0x0 => {
                // P (bit 7) is a protection latch: the value only loads when set.
                if value & 0x80 != 0 {
                    self.mode_32x = value & 0x40 != 0; // X
                    self.write_enable = value & 0x20 != 0; // W (1 = writable)
                    self.led = value & 0x10 != 0; // L
                }
            }
            // $A130F1/F3/.../FF: 5-bit bank selector for slot 0..=7.
            0x1 => self.banks[0] = value & 0x1F,
            0x3 => self.banks[1] = value & 0x1F,
            0x5 => self.banks[2] = value & 0x1F,
            0x7 => self.banks[3] = value & 0x1F,
            0x9 => self.banks[4] = value & 0x1F,
            0xB => self.banks[5] = value & 0x1F,
            0xD => self.banks[6] = value & 0x1F,
            0xF => self.banks[7] = value & 0x1F,
            // Even addresses ($A130F2/F4/...) are the unused high bytes of the
            // 16-bit bank registers.
            _ => {}
        }
    }
}

fn encode_hex(data: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(data.len() * 2);
    for &b in data {
        out.push(DIGITS[(b >> 4) as usize] as char);
        out.push(DIGITS[(b & 0xF) as usize] as char);
    }
    out
}

/// Decode exactly `len` bytes of hex, or `None` if the text is malformed.
fn decode_hex(text: &str, len: usize) -> Option<Vec<u8>> {
    let digits = text.as_bytes();
    if digits.len() != len * 2 {
        return None;
    }
    let nibble = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    digits
        .chunks_exact(2)
        .map(|pair| Some((nibble(pair[0])? << 4) | nibble(pair[1])?))
        .collect()
}
