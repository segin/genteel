#![no_main]
use libfuzzer_sys::fuzz_target;
use genteel::vdp::Vdp;

fuzz_target!(|data: &[u8]| {
    let mut vdp = Vdp::new();
    
    // Each pair of bytes is a control port write
    for chunk in data.chunks(2) {
        let value = if chunk.len() == 2 {
            ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        } else {
            chunk[0] as u16
        };
        
        vdp.write_control(value);
    }
    
    // Read status to exercise that path
    let _ = vdp.read_status();
});
