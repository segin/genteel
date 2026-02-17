use genteel::memory::{IoInterface, MemoryInterface};
use genteel::z80::Z80;
use std::time::Instant;

#[derive(Debug)]
struct SimpleMemory {
    data: Vec<u8>,
}

impl SimpleMemory {
    fn new(size: usize) -> Self {
        Self {
            data: vec![0; size],
        }
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
    fn write_long(&mut self, _address: u32, _value: u32) {}
}

#[derive(Debug)]
struct SimpleIo;
impl IoInterface for SimpleIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0
    }
    fn write_port(&mut self, _port: u16, _value: u8) {}
}

struct BenchContext<'a> {
    mem: &'a mut SimpleMemory,
    io: &'a mut SimpleIo,
}

impl<'a> MemoryInterface for BenchContext<'a> {
    fn read_byte(&mut self, address: u32) -> u8 { self.mem.read_byte(address) }
    fn write_byte(&mut self, address: u32, value: u8) { self.mem.write_byte(address, value) }
    fn read_word(&mut self, address: u32) -> u16 { self.mem.read_word(address) }
    fn write_word(&mut self, address: u32, value: u16) { self.mem.write_word(address, value) }
    fn read_long(&mut self, address: u32) -> u32 { self.mem.read_long(address) }
    fn write_long(&mut self, address: u32, value: u32) { self.mem.write_long(address, value) }
}

impl<'a> IoInterface for BenchContext<'a> {
    fn read_port(&mut self, port: u16) -> u8 { self.io.read_port(port) }
    fn write_port(&mut self, port: u16, value: u8) { self.io.write_port(port, value) }
}

impl std::fmt::Debug for BenchContext<'_> {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "BenchContext")
    }
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

    // Optimized: Using concrete types
    let mut z80 = Z80::new();
    let mut io = SimpleIo;
    z80.a = 0xFF;

    // Warmup
    for _ in 0..1000 {
        let mut ctx = BenchContext { mem: &mut mem, io: &mut io };
        z80.step(&mut ctx);
        if z80.pc == 4 {
            z80.pc = 0;
        }
    }

    let start = Instant::now();
    let iterations = 100_000_000;
    for _ in 0..iterations {
        let mut ctx = BenchContext { mem: &mut mem, io: &mut io };
        z80.step(&mut ctx);
        if z80.pc == 4 {
            z80.pc = 0;
        }
    }
    let duration = start.elapsed();
    println!("Time taken: {:?}", duration);
    println!("Time per step: {:?}", duration / iterations as u32);
}
