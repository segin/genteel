use crate::memory::{IoInterface, Memory, MemoryInterface, Z80Interface};
use crate::z80::Z80;
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct TestIo {
    pub ports: HashMap<u16, u8>,
}

impl IoInterface for TestIo {
    fn read_port(&mut self, port: u16) -> u8 {
        *self.ports.get(&port).unwrap_or(&0xFF)
    }

    fn write_port(&mut self, port: u16, value: u8) {
        self.ports.insert(port, value);
    }
}

/// Combined bus for testing Z80
pub struct CombinedBus {
    pub memory: Memory,
    pub io: TestIo,
}

impl CombinedBus {
    pub fn new(memory: Memory, io: TestIo) -> Self {
        Self { memory, io }
    }
}

impl MemoryInterface for CombinedBus {
    fn read_byte(&mut self, address: u32) -> u8 { self.memory.read_byte(address) }
    fn write_byte(&mut self, address: u32, value: u8) { self.memory.write_byte(address, value) }
    fn read_word(&mut self, address: u32) -> u16 { self.memory.read_word(address) }
    fn write_word(&mut self, address: u32, value: u16) { self.memory.write_word(address, value) }
    fn read_long(&mut self, address: u32) -> u32 { self.memory.read_long(address) }
    fn write_long(&mut self, address: u32, value: u32) { self.memory.write_long(address, value) }
}

impl IoInterface for CombinedBus {
    fn read_port(&mut self, port: u16) -> u8 { self.io.read_port(port) }
    fn write_port(&mut self, port: u16, value: u8) { self.io.write_port(port, value) }
}

pub fn create_z80(program: &[u8]) -> (Z80, CombinedBus) {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    (Z80::new(), CombinedBus::new(m, TestIo::default()))
}
