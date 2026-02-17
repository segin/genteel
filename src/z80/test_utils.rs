use crate::memory::{IoInterface, Memory, MemoryInterface};
use crate::z80::Z80;
use std::collections::HashMap;
use std::ops::{Deref, DerefMut};

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

pub struct TestZ80 {
    pub cpu: Z80,
    pub memory: Memory,
    pub io: TestIo,
}

struct TestContext<'a> {
    memory: &'a mut Memory,
    io: &'a mut TestIo,
}

impl<'a> MemoryInterface for TestContext<'a> {
    fn read_byte(&mut self, address: u32) -> u8 {
        self.memory.read_byte(address)
    }
    fn write_byte(&mut self, address: u32, value: u8) {
        self.memory.write_byte(address, value)
    }
    fn read_word(&mut self, address: u32) -> u16 {
        self.memory.read_word(address)
    }
    fn write_word(&mut self, address: u32, value: u16) {
        self.memory.write_word(address, value)
    }
    fn read_long(&mut self, address: u32) -> u32 {
        self.memory.read_long(address)
    }
    fn write_long(&mut self, address: u32, value: u32) {
        self.memory.write_long(address, value)
    }
}

impl<'a> IoInterface for TestContext<'a> {
    fn read_port(&mut self, port: u16) -> u8 {
        self.io.read_port(port)
    }
    fn write_port(&mut self, port: u16, value: u8) {
        self.io.write_port(port, value)
    }
}

impl std::fmt::Debug for TestContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "TestContext")
    }
}

impl TestZ80 {
    pub fn new(cpu: Z80, memory: Memory, io: TestIo) -> Self {
        Self { cpu, memory, io }
    }

    pub fn step(&mut self) -> u8 {
        let mut context = TestContext {
            memory: &mut self.memory,
            io: &mut self.io,
        };
        self.cpu.step(&mut context)
    }

    pub fn trigger_interrupt(&mut self, vector: u8) -> u8 {
        let mut context = TestContext {
            memory: &mut self.memory,
            io: &mut self.io,
        };
        self.cpu.trigger_interrupt(&mut context, vector)
    }

    pub fn trigger_nmi(&mut self) -> u8 {
        let mut context = TestContext {
            memory: &mut self.memory,
            io: &mut self.io,
        };
        self.cpu.trigger_nmi(&mut context)
    }
}

impl Deref for TestZ80 {
    type Target = Z80;
    fn deref(&self) -> &Self::Target {
        &self.cpu
    }
}

impl DerefMut for TestZ80 {
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.cpu
    }
}
