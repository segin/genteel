use genteel::z80::Z80;
use genteel::memory::{MemoryInterface, IoInterface};
use std::time::Instant;

#[derive(Debug)]
struct SimpleMemory {
    data: Vec<u8>,
}

impl SimpleMemory {
    fn new(size: usize) -> Self {
        Self { data: vec![0; size] }
    }
}

impl MemoryInterface for SimpleMemory {
    fn read_byte(&mut self, address: u32) -> u8 {
        self.data[address as usize & 0xFFFF]
    }
    fn write_byte(&mut self, address: u32, value: u8) {
        self.data[address as usize & 0xFFFF] = value;
    }
    fn read_word(&mut self, address: u32) -> u16 {
        let addr = address as usize & 0xFFFF;
        let low = self.data[addr] as u16;
        let high = self.data[(addr + 1) & 0xFFFF] as u16;
        (high << 8) | low
    }
    fn write_word(&mut self, address: u32, value: u16) {
        let addr = address as usize & 0xFFFF;
        self.data[addr] = value as u8;
        self.data[(addr + 1) & 0xFFFF] = (value >> 8) as u8;
    }
    fn read_long(&mut self, _address: u32) -> u32 {
        0
    }
    fn write_long(&mut self, _address: u32, _value: u32) {
    }
}

#[derive(Debug)]
struct SimpleIo;
impl IoInterface for SimpleIo {
    fn read_port(&mut self, _port: u16) -> u8 { 0 }
    fn write_port(&mut self, _port: u16, _value: u8) {}
}

fn main() {
    let mut mem = SimpleMemory::new(65536);
    // Simple loop: DEC A; JP NZ, -1
    // DEC A: 3D
    // JP NZ, nn: C2 low high
    // 0000: 3D       DEC A
    // 0001: C2 00 00 JP NZ, 0000
    mem.write_byte(0, 0x3D);
    mem.write_byte(1, 0xC2);
    mem.write_byte(2, 0x00);
    mem.write_byte(3, 0x00);

    // Baseline: Using Box<dyn Trait> to force dynamic dispatch
    let mem: Box<dyn MemoryInterface> = Box::new(mem);
    let io: Box<dyn IoInterface> = Box::new(SimpleIo);

    let mut z80 = Z80::new(mem, io);
    z80.a = 0xFF;

    // Warmup
    for _ in 0..1000 {
        z80.step();
        if z80.pc == 4 { z80.pc = 0; }
    }

    let start = Instant::now();
    let iterations = 100_000_000;
    for _ in 0..iterations {
         z80.step();
         if z80.pc == 4 { z80.pc = 0; }
    }
    let duration = start.elapsed();
    println!("Time taken: {:?}", duration);
    println!("Time per step: {:?}", duration / iterations as u32);
}
