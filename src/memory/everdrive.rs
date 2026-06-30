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
//! References: krikzz "extended_ssf-v2" specification and Plutiedev "Beyond 4MB".

use serde::{Deserialize, Serialize};

/// Size of one bank / page (512 KB).
pub const BANK_SIZE: usize = 0x8_0000;
/// Number of 512KB slots covering the `$000000-$3FFFFF` window (8 * 512KB = 4MB).
pub const NUM_SLOTS: usize = 8;
/// Bank register width is 5 bits -> up to 32 banks of backing memory.
const NUM_BANKS: usize = 32;
/// Hard cap on backing memory (32 banks * 512KB = 16MB).
const MAX_MEM_SIZE: usize = NUM_BANKS * BANK_SIZE;
/// The visible cartridge window is always 4MB.
const WINDOW_SIZE: usize = NUM_SLOTS * BANK_SIZE;
/// Offset of the ROM header system field used to detect the mapper.
const HEADER_SYSTEM_OFFSET: usize = 0x100;
/// Magic string in the header that activates the mapper.
const HEADER_MAGIC: &[u8] = b"SEGA SSF";

/// Default per-slot bank mapping: slot N -> bank N, so the first 4MB maps
/// linearly at power-on (and the reset vector in slot 0 is always valid).
fn default_banks() -> [u8; NUM_SLOTS] {
    [0, 1, 2, 3, 4, 5, 6, 7]
}

/// EverDrive / SSF bank-switching mapper state.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EverdriveMapper {
    /// True once a `"SEGA SSF"` ROM has activated the mapper. This is derived
    /// from the ROM at load time, so it is not part of the serialized state
    /// (it is re-established by [`Bus::load_rom`](crate::memory::bus::Bus) after
    /// a save-state restore).
    #[serde(skip)]
    pub enabled: bool,

    /// Per-slot 5-bit bank selectors.
    #[serde(default = "default_banks")]
    pub banks: [u8; NUM_SLOTS],

    /// `W` bit: when set, CPU writes to `$000000-$3FFFFF` land in backing memory,
    /// turning the window into RAM. Write-protected (`false`) at power-on.
    #[serde(default)]
    pub write_enable: bool,

    /// `L` bit: on-board LED state (cosmetic; tracked for completeness).
    #[serde(default)]
    pub led: bool,

    /// `X` bit: 32X passthrough flag (tracked for completeness).
    #[serde(default)]
    pub mode_32x: bool,

    /// Backing memory: the ROM image occupies the low bytes and the remainder is
    /// usable as RAM. Reconstructed from the ROM by `Bus::load_rom`, so (like the
    /// raw ROM) it is not serialized.
    #[serde(skip)]
    pub mem: Vec<u8>,
}

impl Default for EverdriveMapper {
    fn default() -> Self {
        Self::new()
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
        }
    }

    /// Returns true if the ROM header selects the EverDrive / SSF mapper.
    pub fn rom_uses_mapper(rom: &[u8]) -> bool {
        rom.len() >= HEADER_SYSTEM_OFFSET + HEADER_MAGIC.len()
            && &rom[HEADER_SYSTEM_OFFSET..HEADER_SYSTEM_OFFSET + HEADER_MAGIC.len()] == HEADER_MAGIC
    }

    /// Activate the mapper for a freshly loaded ROM. The ROM image is copied into
    /// a backing buffer sized to hold at least the full 4MB window (and the whole
    /// ROM if it is larger), rounded up to a 512KB bank and capped at 16MB. Any
    /// bytes past the ROM start out as open-bus (`0xFF`) and become RAM once the
    /// `W` bit is set.
    ///
    /// The register state (banks / `W` / LED) is intentionally left untouched so
    /// that it survives a save-state restore, which loads the ROM *after*
    /// deserializing the register values.
    pub fn load_rom(&mut self, rom: &[u8]) {
        let needed = rom.len().max(WINDOW_SIZE);
        let size = needed
            .div_ceil(BANK_SIZE)
            .saturating_mul(BANK_SIZE)
            .min(MAX_MEM_SIZE);
        let mut mem = vec![0xFF; size];
        let n = rom.len().min(size);
        mem[..n].copy_from_slice(&rom[..n]);
        self.mem = mem;
        self.enabled = true;
    }

    /// Deactivate the mapper and release its backing memory (non-SSF ROM).
    pub fn disable(&mut self) {
        self.enabled = false;
        self.mem = Vec::new();
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
        if let Some(cell) = self.mem.get_mut(off) {
            *cell = value;
        }
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
