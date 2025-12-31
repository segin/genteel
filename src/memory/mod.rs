//! Memory module for the Genesis emulator
//!
//! Provides both simple memory (for testing) and full memory bus with Genesis memory map.

pub mod bus;
#[cfg(test)]
mod tests_property;

#[derive(Debug, Clone)]
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

    pub fn read_word_le(&self, address: u32) -> u16 {
        let address = address as usize;
        (self.data[address + 1] as u16) << 8
            | (self.data[address] as u16)
    }

    pub fn read_byte(&self, address: u32) -> u8 {
        self.data[address as usize]
    }

    pub fn write_byte(&mut self, address: u32, value: u8) {
        self.data[address as usize] = value;
    }

    pub fn write_word(&mut self, address: u32, value: u16) {
        let address = address as usize;
        self.data[address] = (value >> 8) as u8;
        self.data[address + 1] = value as u8;
    }

    pub fn write_long(&mut self, address: u32, value: u32) {
        let address = address as usize;
        self.data[address] = (value >> 24) as u8;
        self.data[address + 1] = (value >> 16) as u8;
        self.data[address + 2] = (value >> 8) as u8;
        self.data[address + 3] = value as u8;
    }

    pub fn hex_dump(&self, start: u32, end: u32) -> String {
        let mut output = String::new();
        for i in (start..=end).step_by(16) {
            output.push_str(&format!("{:08x}: ", i));
            for j in 0..16 {
                if (i + j) <= end {
                    output.push_str(&format!("{:02X} ", self.data[(i + j) as usize]));
                } else {
                    output.push_str("   ");
                }
            }
            output.push_str(" "); // Add space before ASCII part

            for j in 0..16 {
                if (i + j) <= end {
                    let byte = self.data[(i + j) as usize];
                    if byte.is_ascii_graphic() {
                        output.push(byte as char);
                    } else {
                        output.push('.');
                    }
                }
            }
            output.push('\n');
        }
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hex_dump() {
        let mut memory = Memory::new(256);
        for i in 0..32 {
            memory.data[i] = 'A' as u8 + i as u8;
        }
        
        let dump = memory.hex_dump(0, 31);
        let expected_lines: Vec<&str> = vec![
            "00000000: 41 42 43 44 45 46 47 48 49 4A 4B 4C 4D 4E 4F 50  ABCDEFGHIJKLMNOP",
            "00000010: 51 52 53 54 55 56 57 58 59 5A 5B 5C 5D 5E 5F 60  QRSTUVWXYZ[\\]^_`",
        ];

        let actual_lines: Vec<&str> = dump.trim().lines().collect();

        assert_eq!(actual_lines.len(), expected_lines.len());

        for (i, actual_line) in actual_lines.iter().enumerate() {
            assert_eq!(*actual_line, expected_lines[i]);
        }
    }
}
