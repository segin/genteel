use super::*;

fn write_data_bulk(vdp: &mut Vdp, data: &[u8]) {
    for chunk in data.chunks(2) {
        if chunk.len() == 2 {
            let val = ((chunk[0] as u16) << 8) | (chunk[1] as u16);
            vdp.write_data(val);
        }
    }
}

#[test]
fn test_bulk_write_optimization() {
    let mut vdp = Vdp::new();

    // Setup:
    // 1. Set Auto-increment to 2 (Reg 15 = 0x02)
    vdp.write_control(0x8F02);
    assert_eq!(vdp.registers[REG_AUTO_INC], 2);

    // 2. Set VRAM Write Address to 0x0000
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);
    assert_eq!(vdp.control_address, 0x0000);
    assert_eq!(vdp.control_code & 0x0F, VRAM_WRITE);

    // 3. Prepare data
    let data = [0x11, 0x22, 0x33, 0x44];

    // Execute
    write_data_bulk(&mut vdp, &data);

    // Verify VRAM content
    assert_eq!(vdp.vram[0], 0x11);
    assert_eq!(vdp.vram[1], 0x22);
    assert_eq!(vdp.vram[2], 0x33);
    assert_eq!(vdp.vram[3], 0x44);

    // Verify Control Address update
    assert_eq!(vdp.control_address, 0x0004);

    // Verify last_data_write update
    assert_eq!(vdp.last_data_write, 0x3344);
}

#[test]
fn test_bulk_write_fallback() {
    let mut vdp = Vdp::new();

    // Setup:
    // 1. Set Auto-increment to 1 (Reg 15 = 0x01) - forces fallback path
    vdp.write_control(0x8F01);
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);

    let data = [0xAA, 0xBB, 0xCC, 0xDD];
    write_data_bulk(&mut vdp, &data);

    // Verify behavior with auto-inc=1
    assert_eq!(vdp.vram[0], 0xDD);
    assert_eq!(vdp.vram[1], 0xCC);
    assert_eq!(vdp.control_address, 0x0002);

    // 2. Not VRAM Write (e.g. CRAM Write)
    vdp.write_control(0x8F02);
    vdp.write_control(0xC000);
    vdp.write_control(0x0000);

    let data_cram = [0x0E, 0xEE]; // White (0EEE)
    write_data_bulk(&mut vdp, &data_cram);

    assert_eq!(vdp.cram[0], 0xEE);
    assert_eq!(vdp.cram[1], 0x0E);
    assert_eq!(vdp.control_address, 0x0002);
}

#[test]
fn test_bulk_write_wrapping() {
    let mut vdp = Vdp::new();

    // Setup: Auto-inc 2, VRAM Write
    vdp.write_control(0x8F02);

    // Set address to 0xFFFE
    vdp.write_control(0x7FFE);
    vdp.write_control(0x0003);

    assert_eq!(vdp.control_address, 0xFFFE);

    let data = [0x11, 0x22, 0x33, 0x44];
    write_data_bulk(&mut vdp, &data);

    // Verify wrapping
    assert_eq!(vdp.vram[0xFFFE], 0x11);
    assert_eq!(vdp.vram[0xFFFF], 0x22);
    assert_eq!(vdp.vram[0x0000], 0x33);
    assert_eq!(vdp.vram[0x0001], 0x44);
    assert_eq!(vdp.control_address, 0x0002);
}
