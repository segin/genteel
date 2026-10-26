#![no_main]

//! Save-state / debugger restore fuzzer.
//!
//! Feeds arbitrary JSON to `Debuggable::write_state` on the VDP, APU, I/O
//! and Z80, then runs each component for a while. Out-of-range restored
//! fields must be clamped or rejected, never cause a panic.

use genteel::apu::Apu;
use genteel::debugger::Debuggable;
use genteel::io::Io;
use genteel::memory::{IoInterface, Memory};
use genteel::vdp::Vdp;
use genteel::z80::Z80;
use libfuzzer_sys::fuzz_target;
use serde_json::Value;

/// I/O space with nothing attached: reads float high, writes are dropped.
#[derive(Debug)]
struct OpenBusIo;

impl IoInterface for OpenBusIo {
    fn read_port(&mut self, _port: u16) -> u8 {
        0xFF
    }
    fn write_port(&mut self, _port: u16, _value: u8) {}
}

/// Master clocks per scanline.
const MCLK_PER_LINE: u32 = 3420;

fuzz_target!(|data: &[u8]| {
    let Ok(state) = serde_json::from_slice::<Value>(data) else {
        return;
    };

    let mut vdp = Vdp::new();
    vdp.write_state(&state);
    vdp.tick(MCLK_PER_LINE * 320, |_| 0);
    vdp.read_status();
    let _ = vdp.read_state();

    let mut apu = Apu::new();
    apu.write_state(&state);
    for _ in 0..64 {
        apu.tick_cycles(64);
        apu.generate_sample();
    }
    let _ = apu.read_state();

    let mut io = Io::new();
    io.write_state(&state);
    for addr in (0xA10000..0xA10020).step_by(2) {
        io.read(addr + 1);
    }
    let _ = io.read_state();

    let mut z80 = Z80::new(Memory::new(0x10000), OpenBusIo);
    z80.write_state(&state);
    for _ in 0..64 {
        z80.step();
    }
    let _ = z80.read_state();
});
