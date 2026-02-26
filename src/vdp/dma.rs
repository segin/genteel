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
    fn step_dma<F>(&mut self, read_bus_word: &mut F)
    where
        F: FnMut(u32) -> u16;
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
        let fill_msb = (self.last_data_write >> 8) as u8;
        let fill_lsb = (self.last_data_write & 0xFF) as u8;
        let mut addr = self.command.address;
        let inc = self.registers[REG_AUTO_INC] as u16;

        if len > 0 {
            // First write is always LSB
            self.vram[addr as usize] = fill_lsb;
            addr = addr.wrapping_add(inc);

            // Remaining writes are MSB
            if len > 1 {
                let remaining_len = len - 1;
                if inc == 1 {
                    let start = addr as usize;
                    let count = remaining_len as usize;
                    let vram_len = self.vram.len();

                    // Handle wrapping
                    if start + count <= vram_len {
                        self.vram[start..start + count].fill(fill_msb);
                    } else {
                        let first_part = vram_len - start;
                        self.vram[start..vram_len].fill(fill_msb);
                        let remaining = count - first_part;
                        if remaining > 0 {
                            self.vram[0..remaining].fill(fill_msb);
                        }
                    }
                    addr = addr.wrapping_add(remaining_len as u16);
                } else if inc == 0 {
                    self.vram[addr as usize] = fill_msb;
                } else {
                    for _ in 0..remaining_len {
                        self.vram[addr as usize] = fill_msb;
                        addr = addr.wrapping_add(inc);
                    }
                }
            }
        }
        self.command.address = addr;
    }

    fn execute_dma(&mut self) -> u32 {
        let length = self.dma_length();
        let len = if length == 0 { 0x10000 } else { length };

        let mode = self.registers[REG_DMA_SRC_HI] & DMA_MODE_MASK;

        match mode {
            DMA_MODE_FILL => {
                self.perform_dma_fill(len);
            }
            DMA_MODE_COPY => {
                let mut source = (self.dma_source() & 0xFFFF) as u16;
                let mut dest = self.command.address;
                let inc = self.registers[REG_AUTO_INC] as u16;

                for _ in 0..len {
                    let val = self.vram[source as usize];
                    self.vram[dest as usize] = val;
                    source = source.wrapping_add(1);
                    dest = dest.wrapping_add(inc);
                }
                self.command.address = dest;
            }
            _ => {
                for _ in 0..len {
                    self.step_dma(&mut |_| 0xFFFF);
                }
            }
        }

        self.command.dma_pending = false;
        len
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
                let addr = self.command.address;
                let val = if length == self.dma_length() && self.dma_length() != 0 {
                    (self.last_data_write & 0xFF) as u8
                } else {
                    (self.last_data_write >> 8) as u8
                };
                
                if (self.command.code & 0x0F) == VRAM_WRITE {
                    self.vram[addr as usize] = val;
                }
                
                self.command.address = addr.wrapping_add(inc);
            }
            DMA_MODE_COPY => {
                let source = (self.dma_source() & 0xFFFF) as u16;
                let addr = self.command.address;
                
                if (self.command.code & 0x0F) == VRAM_WRITE {
                    let val = self.vram[source as usize];
                    self.vram[addr as usize] = val;
                }
                
                let next_source = source.wrapping_add(1);
                self.registers[REG_DMA_SRC_LO] = (next_source & 0xFF) as u8;
                self.registers[REG_DMA_SRC_MID] = (next_source >> 8) as u8;
                self.command.address = addr.wrapping_add(inc);
            }
            _ => {
                // Memory-to-VDP
                let source = self.dma_source_transfer();
                let val = read_bus_word(source);
                
                let addr = self.command.address;
                let code = self.command.code;
                match code & 0x0F {
                    VRAM_WRITE => {
                        let idx = addr as usize;
                        if idx < self.vram.len() {
                            self.vram[idx] = (val >> 8) as u8;
                            self.vram[idx ^ 1] = (val & 0xFF) as u8;
                        }
                    }
                    CRAM_WRITE => {
                        let idx = (addr as usize / 2) & 0x3F;
                        self.cram[idx * 2] = (val & 0xFF) as u8;
                        self.cram[idx * 2 + 1] = (val >> 8) as u8;
                        self.cram_cache[idx] = Self::genesis_color_to_rgb565(val);
                    }
                    VSRAM_WRITE => {
                        let idx = (addr as usize) % 80;
                        self.vsram[idx] = (val >> 8) as u8;
                        if idx + 1 < 80 {
                            self.vsram[idx + 1] = (val & 0xFF) as u8;
                        }
                    }
                    _ => {}
                }
                
                self.command.address = addr.wrapping_add(inc);
                
                let next_source = source.wrapping_add(2);
                self.registers[REG_DMA_SRC_LO] = ((next_source >> 1) & 0xFF) as u8;
                self.registers[REG_DMA_SRC_MID] = ((next_source >> 9) & 0xFF) as u8;
                self.registers[REG_DMA_SRC_HI] = (self.registers[REG_DMA_SRC_HI] & 0x80) | (((next_source >> 17) & 0x7F) as u8);
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
