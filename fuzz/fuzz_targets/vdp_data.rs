#![no_main]
use libfuzzer_sys::fuzz_target;
use genteel::vdp::Vdp;

fuzz_target!(|data: &[u8]| {
    if data.len() < 6 {
        return;
    }
    
    let mut vdp = Vdp::new();
    
    // First 4 bytes set up the control state
    let ctrl1 = ((data[0] as u16) << 8) | (data[1] as u16);
    let ctrl2 = ((data[2] as u16) << 8) | (data[3] as u16);
    vdp.write_control(ctrl1);
    vdp.write_control(ctrl2);
    
    // Remaining bytes are data port writes
    for chunk in data[4..].chunks(2) {
        let value = if chunk.len() == 2 {
            ((chunk[0] as u16) << 8) | (chunk[1] as u16)
        } else {
            chunk[0] as u16
        };
        
        vdp.write_data(value);
    }
    
    // Also try reading
    let _ = vdp.read_data();
});
