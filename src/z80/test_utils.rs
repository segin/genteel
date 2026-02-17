use crate::memory::IoInterface;
use crate::memory::Memory;
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

pub fn create_z80(program: &[u8]) -> Z80<Memory, TestIo> {
    let mut m = Memory::new(0x10000);
    for (i, &b) in program.iter().enumerate() {
        m.data[i] = b;
    }
    Z80::new(m, TestIo::default())
}
