// src/memory/mod.rs

pub struct Memory {
    // For now, a simple vector for the memory.
    // The Genesis has a 24-bit address bus, so 16MB of address space.
    pub data: Vec<u8>,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }

    pub fn read_long(&self, address: u32) -> u32 {
        let address = address as usize;
        (self.data[address] as u32) << 24
            | (self.data[address + 1] as u32) << 16
            | (self.data[address + 2] as u32) << 8
            | (self.data[address + 3] as u32)
    }

    pub fn read_word(&self, address: u32) -> u16 {
        let address = address as usize;
        (self.data[address] as u16) << 8
            | (self.data[address + 1] as u16)
    }
}
