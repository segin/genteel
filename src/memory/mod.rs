//! Memory module for the Genesis emulator
//!
//! Provides both simple memory (for testing) and full memory bus with Genesis memory map.

use std::cell::RefCell;
use std::rc::Rc;
pub mod bus;
use bus::Bus;

#[cfg(test)]
mod tests_property;

pub trait MemoryInterface: std::fmt::Debug {
    fn read_byte(&mut self, address: u32) -> u8;
    fn write_byte(&mut self, address: u32, value: u8);
    fn read_word(&mut self, address: u32) -> u16;
    fn write_word(&mut self, address: u32, value: u16);
    fn read_long(&mut self, address: u32) -> u32;
    fn write_long(&mut self, address: u32, value: u32);
}

// Blanket impl for Box<dyn MemoryInterface>
impl MemoryInterface for Box<dyn MemoryInterface> {
    fn read_byte(&mut self, address: u32) -> u8 {
        (**self).read_byte(address)
    }
    fn write_byte(&mut self, address: u32, value: u8) {
        (**self).write_byte(address, value);
    }
    fn read_word(&mut self, address: u32) -> u16 {
        (**self).read_word(address)
    }
    fn write_word(&mut self, address: u32, value: u16) {
        (**self).write_word(address, value);
    }
    fn read_long(&mut self, address: u32) -> u32 {
        (**self).read_long(address)
    }
    fn write_long(&mut self, address: u32, value: u32) {
        (**self).write_long(address, value);
    }
}

#[derive(Clone, Debug)]
pub struct SharedBus {
    pub bus: Rc<RefCell<Bus>>,
}

impl SharedBus {
    pub fn new(bus: Rc<RefCell<Bus>>) -> Self {
        Self { bus }
    }
}

impl MemoryInterface for SharedBus {
    fn read_byte(&mut self, address: u32) -> u8 {
        self.bus.borrow_mut().read_byte(address)
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        self.bus.borrow_mut().write_byte(address, value);
    }

    fn read_word(&mut self, address: u32) -> u16 {
        self.bus.borrow_mut().read_word(address)
    }

    fn write_word(&mut self, address: u32, value: u16) {
        self.bus.borrow_mut().write_word(address, value);
    }

    fn read_long(&mut self, address: u32) -> u32 {
        self.bus.borrow_mut().read_long(address)
    }

    fn write_long(&mut self, address: u32, value: u32) {
        self.bus.borrow_mut().write_long(address, value);
    }
}

#[derive(Debug, Clone)]
pub struct Memory {
    pub data: Vec<u8>,
}

impl Memory {
    pub fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
    }
}

impl MemoryInterface for Memory {
    fn read_byte(&mut self, address: u32) -> u8 {
        self.data[address as usize]
    }

    fn write_byte(&mut self, address: u32, value: u8) {
        self.data[address as usize] = value;
    }

    fn read_word(&mut self, address: u32) -> u16 {
        let address = address as usize;
        (self.data[address] as u16) << 8 | (self.data[address + 1] as u16)
    }

    fn write_word(&mut self, address: u32, value: u16) {
        let address = address as usize;
        self.data[address] = (value >> 8) as u8;
        self.data[address + 1] = value as u8;
    }

    fn read_long(&mut self, address: u32) -> u32 {
        let address = address as usize;
        (self.data[address] as u32) << 24
            | (self.data[address + 1] as u32) << 16
            | (self.data[address + 2] as u32) << 8
            | (self.data[address + 3] as u32)
    }

    fn write_long(&mut self, address: u32, value: u32) {
        let address = address as usize;
        self.data[address] = (value >> 24) as u8;
        self.data[address + 1] = (value >> 16) as u8;
        self.data[address + 2] = (value >> 8) as u8;
        self.data[address + 3] = value as u8;
    }
}

impl Memory {

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
