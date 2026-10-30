use super::constants::*;
use super::{DmaKind, Vdp};

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
    fn dma_copy_source(&self) -> u16;
    fn dma_length(&self) -> u32;
    fn dma_source_transfer(&self) -> u32;
    fn is_dma_transfer(&self) -> bool;
    /// Run a running DMA fill or VRAM copy to completion at once, without
    /// slot timing, and return the number of units processed. 68k-bus
    /// transfers are rejected with `DmaError::NoBusReader` and left pending;
    /// a fill still waiting for its data-port write processes nothing.
    fn execute_dma(&mut self) -> Result<u32, DmaError>;
    fn perform_dma_fill(&mut self, len: u32);
    /// Process one DMA unit (a 68k word, a copied byte or a fill byte).
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
        self.set_dma_copy_source(source.wrapping_add(1));
        self.command.address = addr.wrapping_add(self.registers[REG_AUTO_INC] as u16);
    }

    fn set_dma_copy_source(&mut self, source: u16) {
        self.registers[REG_DMA_SRC_LO] = (source & 0xFF) as u8;
        self.registers[REG_DMA_SRC_MID] = (source >> 8) as u8;
    }

    /// The DMA is done: clear the busy state and write a control word held
    /// back during a 68k DMA.
    fn finish_dma(&mut self) {
        self.command.dma_pending = false;
        self.command.dma_half = false;
        self.command.dma_delay = 0;
        if let Some(word) = self.command.cached_control.take() {
            self.write_control(word);
        }
    }
}

impl DmaOps for Vdp {
    fn dma_mode(&self) -> u8 {
        self.registers[REG_DMA_SRC_HI]
    }

    fn dma_length(&self) -> u32 {
        ((self.registers[REG_DMA_LEN_HI] as u32) << 8) | (self.registers[REG_DMA_LEN_LO] as u32)
    }

    /// VRAM-copy source address: a plain byte VRAM address in regs 21/22,
    /// with no `<<1` word doubling (that only applies to 68K memory sources).
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

    /// Register 23 currently selects a 68k-bus transfer (modes 0 and 1).
    /// A pending DMA uses the kind latched in `command.dma_kind` instead.
    fn is_dma_transfer(&self) -> bool {
        (self.registers[REG_DMA_SRC_HI] & 0x80) == 0
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
                let data = (self.last_data_write() >> 8) as u8;
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
        if !self.command.dma_pending {
            return Ok(0);
        }
        if self.command.dma_kind == DmaKind::Bus {
            return Err(DmaError::NoBusReader);
        }
        let mut units = 0;
        while self.command.dma_pending && !self.command.dma_fill_wait {
            self.step_dma(&mut |_| 0);
            units += 1;
        }
        Ok(units)
    }

    fn step_dma<F>(&mut self, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16,
    {
        if !self.command.dma_pending || self.command.dma_fill_wait {
            /* An armed fill does nothing (and consumes no length) until its
            data-port write. */
            return;
        }

        let mut length = self.dma_length();
        if length == 0 {
            length = 0x10000;
        }

        match self.command.dma_kind {
            DmaKind::Fill => {
                self.perform_dma_fill(1);
                /* The source registers advance during a fill too (GPGX
                `vdp_dma_update`). */
                let next = self.dma_copy_source().wrapping_add(1);
                self.set_dma_copy_source(next);
            }
            DmaKind::Copy => self.dma_copy_step(),
            DmaKind::Bus => {
                let inc = self.registers[REG_AUTO_INC] as u16;
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
                let next = self.dma_copy_source().wrapping_add(1);
                self.set_dma_copy_source(next);
            }
        }

        length -= 1;
        self.registers[REG_DMA_LEN_LO] = (length & 0xFF) as u8;
        self.registers[REG_DMA_LEN_HI] = (length >> 8) as u8;

        if length == 0 {
            self.finish_dma();
        }
    }
}
