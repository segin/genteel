#![no_main]
use libfuzzer_sys::fuzz_target;
use genteel::vdp::Vdp;

fuzz_target!(|data: &[u8]| {
    if data.len() < 1024 {
        return; // Need some data to make rendering interesting
    }
    
    let mut vdp = Vdp::new();
    
    // Fill VRAM with fuzz data
    let vram_len = data.len().min(0x10000);
    vdp.vram[..vram_len].copy_from_slice(&data[..vram_len]);
    
    // Fill CRAM with some of the fuzz data
    if data.len() >= 128 {
        vdp.cram.copy_from_slice(&data[..128]);
    }
    
    // Set up registers for rendering
    // Enable display (reg 1 bit 6)
    vdp.registers[1] = 0x44;
    // Set Plane A address
    vdp.registers[2] = 0x30;
    // Set auto-increment
    vdp.registers[15] = 2;
    
    // Render several lines
    for line in 0..224 {
        vdp.render_line(line);
    }
    
    // Render full frame
    vdp.render_frame();
});
