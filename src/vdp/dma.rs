use super::constants::*;
use super::Vdp;

/// Reasons a DMA operation cannot be executed synchronously.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DmaError {
    /// A 68k-bus to VDP transfer needs a bus reader; it runs slot by slot
    /// through `tick`/`step_dma` instead.
    NoBusReader,
}

impl std::fmt::Display for DmaError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            DmaError::NoBusReader => write!(f, "68k-bus DMA requires a bus reader"),
        }
    }
}

impl std::error::Error for DmaError {}

pub trait DmaOps {
    fn dma_mode(&self) -> u8;
    fn dma_source(&self) -> u32;
    fn dma_copy_source(&self) -> u16;
    fn dma_length(&self) -> u32;
    fn dma_source_transfer(&self) -> u32;
    fn is_dma_transfer(&self) -> bool;
    fn is_dma_fill(&self) -> bool;
    /// Run a pending DMA fill or VRAM copy to completion and return the
    /// number of units processed. 68k-bus transfers are rejected with
    /// `DmaError::NoBusReader` and left pending.
    fn execute_dma(&mut self) -> Result<u32, DmaError>;
    fn perform_dma_fill(&mut self, len: u32);
    fn step_dma<F>(&mut self, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16;
}

impl Vdp {
    /// One VRAM copy step. The copy reads and writes the byte-swapped
    /// (adjacent) addresses, and only transfers data when CD4 is set in the
    /// command (GPGX `vdp_dma_copy`). Source and destination advance
    /// regardless.
    fn dma_copy_step(&mut self) {
        let source = self.dma_copy_source();
        let addr = self.command.address;
        if (self.command.code & 0x10) != 0 {
            let data = self.vram[(source ^ 1) as usize];
            self.write_vram_byte(addr ^ 1, data);
        }
        let next_source = source.wrapping_add(1);
        self.registers[REG_DMA_SRC_LO] = (next_source & 0xFF) as u8;
        self.registers[REG_DMA_SRC_MID] = (next_source >> 8) as u8;
        self.command.address = addr.wrapping_add(self.registers[REG_AUTO_INC] as u16);
    }

    fn charge_dma_stall(&mut self, mode: u8, len: u32) {
        /* Charge 68k stall cycles for the duration of the DMA. Slot
        availability differs by ~7x between active display (~14 slots per
        line) and VBlank (~167 slots per line), so the stall calibration
        differs accordingly:
          active display: Fill=2/byte, Copy=4/byte
          VBlank:         Fill=1 per 4 bytes, Copy=1 per 2 bytes */
        let in_vblank = (self.status & STATUS_VBLANK) != 0;
        let stall = match (mode, in_vblank) {
            (DMA_MODE_COPY, true) => len / 2,
            (DMA_MODE_COPY, false) => len.saturating_mul(4),
            (_, true) => len / 4,
            (_, false) => len.saturating_mul(2),
        };
        self.dma_stall_cycles = self.dma_stall_cycles.saturating_add(stall);
    }
}

impl DmaOps for Vdp {
    fn dma_mode(&self) -> u8 {
        self.registers[REG_DMA_SRC_HI]
    }

    /// Word-doubled 68K source assembly. Superseded in production by
    /// `dma_source_transfer` (which also models the RAM-address force).
    fn dma_source(&self) -> u32 {
        ((self.registers[REG_DMA_SRC_HI] as u32) << 17)
            | ((self.registers[REG_DMA_SRC_MID] as u32) << 9)
            | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    }

    fn dma_length(&self) -> u32 {
        ((self.registers[REG_DMA_LEN_HI] as u32) << 8) | (self.registers[REG_DMA_LEN_LO] as u32)
    }

    /// VRAM-copy source address. Unlike `dma_source`, a copy source is a plain
    /// byte VRAM address in regs 21/22 with no `<<1` word doubling (that doubling
    /// only applies to 68K memory sources).
    fn dma_copy_source(&self) -> u16 {
        ((self.registers[REG_DMA_SRC_MID] as u16) << 8) | (self.registers[REG_DMA_SRC_LO] as u16)
    }

    fn dma_source_transfer(&self) -> u32 {
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
    fn is_dma_transfer(&self) -> bool {
        (self.registers[REG_DMA_SRC_HI] & 0x80) == 0
    }

    fn is_dma_fill(&self) -> bool {
        (self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK) == DMA_MODE_FILL
    }

    /// DMA fill of `len` units starting at the current address. The data-port
    /// write that triggered the fill has already been written normally and
    /// advanced the address (GPGX `vdp_68k_data_w_m5` + `vdp_dma_fill`):
    ///   * VRAM: the MSB of that data word is written to `address ^ 1` on
    ///     each step.
    ///   * CRAM / VSRAM: the word in the next FIFO slot (not the fill data)
    ///     is written to each entry.
    ///   * other targets: nothing is written, the address still advances.
    fn perform_dma_fill(&mut self, len: u32) {
        let inc = self.registers[REG_AUTO_INC] as u16;
        let mut addr = self.command.address;
        match self.command.code & 0x0F {
            VRAM_WRITE => {
                let last = self.fifo_history[(self.fifo_history_idx + 3) & 3];
                let data = (last >> 8) as u8;
                for _ in 0..len {
                    self.write_vram_byte(addr ^ 1, data);
                    addr = addr.wrapping_add(inc);
                }
            }
            CRAM_WRITE => {
                let data = self.fifo_next_entry();
                for _ in 0..len {
                    self.write_cram_word(addr, data);
                    addr = addr.wrapping_add(inc);
                }
            }
            VSRAM_WRITE => {
                let data = self.fifo_next_entry();
                for _ in 0..len {
                    self.write_vsram_word(addr, data);
                    addr = addr.wrapping_add(inc);
                }
            }
            _ => {
                addr = addr.wrapping_add(inc.wrapping_mul(len as u16));
            }
        }
        self.command.address = addr;
    }

    fn execute_dma(&mut self) -> Result<u32, DmaError> {
        let mode = self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK;
        if mode != DMA_MODE_FILL && mode != DMA_MODE_COPY {
            return Err(DmaError::NoBusReader);
        }

        let len_regs = self.dma_length() as u16;
        let len = if len_regs == 0 {
            0x10000
        } else {
            len_regs as u32
        };
        self.charge_dma_stall(mode, len);

        if mode == DMA_MODE_FILL {
            self.perform_dma_fill(len);
            /* The source registers advance by the length even for a fill
            (GPGX `vdp_dma_update`). */
            let end = self.dma_copy_source().wrapping_add(len_regs);
            self.registers[REG_DMA_SRC_LO] = (end & 0xFF) as u8;
            self.registers[REG_DMA_SRC_MID] = (end >> 8) as u8;
        } else {
            // Each copy step advances the source registers itself.
            for _ in 0..len {
                self.dma_copy_step();
            }
        }

        // The length counter has run down to zero.
        self.registers[REG_DMA_LEN_LO] = 0;
        self.registers[REG_DMA_LEN_HI] = 0;
        self.command.dma_pending = false;
        Ok(len)
    }

    fn step_dma<F>(&mut self, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        if !self.command.dma_pending {
            return;
        }

        let mode = self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK;
        let inc = self.registers[REG_AUTO_INC] as u16;

        let mut length = self.dma_length();
        if length == 0 {
            length = 0x10000;
        }

        match mode {
            DMA_MODE_FILL => {
                /* A VRAM fill does nothing until the fill data is written to
                the data port; `write_data` then completes it synchronously via
                `execute_dma`. Do not step (and do not consume length) here. */
                return;
            }
            DMA_MODE_COPY => self.dma_copy_step(),
            _ => {
                // Memory-to-VDP
                let source = self.dma_source_transfer();
                let val = read_bus_word(source);

                let addr = self.command.address;
                let code = self.command.code;
                self.process_fifo_entry(super::FifoEntry {
                    address: addr,
                    code,
                    value: val,
                });

                self.command.address = addr.wrapping_add(inc);

                /* The source advances within its 128 KiB block: only the
                word address in registers 21/22 increments (and wraps);
                register 23 is never modified (GPGX `vdp_dma_68k_*`). */
                let word = (((self.registers[REG_DMA_SRC_MID] as u16) << 8)
                    | (self.registers[REG_DMA_SRC_LO] as u16))
                    .wrapping_add(1);
                self.registers[REG_DMA_SRC_LO] = (word & 0xFF) as u8;
                self.registers[REG_DMA_SRC_MID] = (word >> 8) as u8;
            }
        }

        length -= 1;
        self.registers[REG_DMA_LEN_LO] = (length & 0xFF) as u8;
        self.registers[REG_DMA_LEN_HI] = (length >> 8) as u8;

        if length == 0 {
            self.command.dma_pending = false;
        }
    }
}
