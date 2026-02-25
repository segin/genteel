use super::constants::*;
use super::Vdp;

pub trait DmaOps {
    fn dma_mode(&self) -> u8;
    fn dma_source(&self) -> u32;
    fn dma_length(&self) -> u32;
    fn dma_source_transfer(&self) -> u32;
    fn is_dma_transfer(&self) -> bool;
    fn is_dma_fill(&self) -> bool;
    fn execute_dma(&mut self) -> u32;
    fn perform_dma_fill(&mut self, len: u32);
}

impl DmaOps for Vdp {
    fn dma_mode(&self) -> u8 {
        self.registers[REG_DMA_SRC_HI]
    }

    fn dma_source(&self) -> u32 {
        ((self.registers[REG_DMA_SRC_HI] as u32) << 17)
            | ((self.registers[REG_DMA_SRC_MID] as u32) << 9)
            | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    }

    fn dma_length(&self) -> u32 {
        ((self.registers[REG_DMA_LEN_HI] as u32) << 8) | (self.registers[REG_DMA_LEN_LO] as u32)
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

    fn perform_dma_fill(&mut self, len: u32) {
        let fill_byte = (self.last_data_write >> 8) as u8;
        let mut addr = self.control_address;
        let inc = self.registers[REG_AUTO_INC] as u16;

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

    fn execute_dma(&mut self) -> u32 {
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
                let inc = self.registers[REG_AUTO_INC] as u16;

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
}
