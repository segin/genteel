use crate::memory::IoInterface;
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
