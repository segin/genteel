#![no_main]

//! VDP renderer fuzzer.
//!
//! Loads arbitrary VRAM, CRAM, VSRAM and register contents, then renders
//! every visible line directly and runs one full frame through `tick`.
//! Rendering must never panic or index out of bounds.

use genteel::vdp::{RenderOps, Vdp, NUM_REGISTERS};
use libfuzzer_sys::fuzz_target;

/// Master clocks per scanline.
const MCLK_PER_LINE: u32 = 3420;
/// Lines in a PAL frame (the longer of the two).
const LINES_PER_FRAME: u32 = 313;

fuzz_target!(|data: &[u8]| {
    /* Registers, then 128 bytes of CRAM, 80 of VSRAM and the VRAM image. */
    if data.len() < NUM_REGISTERS + 128 + 80 + 1024 {
        return;
    }
    let (regs, rest) = data.split_at(NUM_REGISTERS);
    let (cram, rest) = rest.split_at(128);
    let (vsram, vram) = rest.split_at(80);

    let mut vdp = Vdp::new();
    vdp.registers.copy_from_slice(regs);
    /* Keep the display on so the render paths are reached. */
    vdp.registers[1] |= 0x40;
    vdp.cram.copy_from_slice(cram);
    vdp.reconstruct_cram_cache();
    vdp.vsram.copy_from_slice(vsram);
    let vram_len = vram.len().min(vdp.vram.len());
    vdp.vram[..vram_len].copy_from_slice(&vram[..vram_len]);

    let height = vdp.screen_height();
    for line in 0..height {
        vdp.render_line(line);
    }

    vdp.tick(MCLK_PER_LINE * LINES_PER_FRAME, |addr| {
        let i = (addr as usize) % data.len();
        u16::from_be_bytes([data[i], data[(i + 1) % data.len()]])
    });
});
