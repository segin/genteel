use super::*;

#[test]
fn test_write_data_bulk_optimized() {
    let mut vdp = Vdp::new();

    // Setup for VRAM Write with Auto-Increment 2
    vdp.write_control(0x8F02); // Set Auto-Increment to 2 (Reg 15)
    vdp.write_control(0x4000); // Set Control Code to VRAM Write (0x1) at address 0x0000.

    let data = [0xAA, 0xBB, 0xCC, 0xDD, 0xEE, 0xFF];
    vdp.write_data_bulk(&data);

    // Optimized path:
    // Chunk 0: [0xAA, 0xBB] -> written to addr 0x0000.
    // vram[0] = 0xAA, vram[1] = 0xBB.
    // Address increments by 2 -> 0x0002.
    // Chunk 1: [0xCC, 0xDD] -> written to addr 0x0002.
    // vram[2] = 0xCC, vram[3] = 0xDD.
    // Address increments by 2 -> 0x0004.
    // Chunk 2: [0xEE, 0xFF] -> written to addr 0x0004.
    // vram[4] = 0xEE, vram[5] = 0xFF.
    // Address increments by 2 -> 0x0006.

    assert_eq!(vdp.vram[0], 0xAA);
    assert_eq!(vdp.vram[1], 0xBB);
    assert_eq!(vdp.vram[2], 0xCC);
    assert_eq!(vdp.vram[3], 0xDD);
    assert_eq!(vdp.vram[4], 0xEE);
    assert_eq!(vdp.vram[5], 0xFF);
    assert_eq!(vdp.control_address, 0x0006);

    // Verify odd address start
    vdp.write_control(0x4001); // Addr 0x0001
    // Auto-inc is still 2.

    let data2 = [0x11, 0x22];
    vdp.write_data_bulk(&data2);

    // Chunk: [0x11, 0x22] -> addr 0x0001.
    // vram[1] = 0x11, vram[1^1] = vram[0] = 0x22.
    // Address increments by 2 -> 0x0003.

    assert_eq!(vdp.vram[1], 0x11);
    assert_eq!(vdp.vram[0], 0x22);
    assert_eq!(vdp.control_address, 0x0003);
}

#[test]
fn test_write_data_bulk_fallback() {
    let mut vdp = Vdp::new();

    // 1. Auto-increment != 2
    vdp.write_control(0x8F01); // Set Auto-Increment to 1 (Reg 15)
    vdp.write_control(0x4000); // VRAM Write at 0x0000

    let data = [0xAA, 0xBB];
    vdp.write_data_bulk(&data);

    // Fallback path:
    // Chunk: [0xAA, 0xBB] -> val 0xAABB.
    // write_data(0xAABB) called.
    // vram[0] = 0xAA, vram[1] = 0xBB.
    // Addr increments by 1 -> 0x0001.

    assert_eq!(vdp.vram[0], 0xAA);
    assert_eq!(vdp.vram[1], 0xBB);
    assert_eq!(vdp.control_address, 0x0001);

    // 2. Not VRAM Write (e.g. CRAM Write)
    vdp.write_control(0x8F02); // Set Auto-Increment to 2
    vdp.write_control(0xC000); // CRAM Write (0x3) at 0x0000

    let data_cram = [0x0E, 0xEE]; // White (0EEE)
    vdp.write_data_bulk(&data_cram);

    // Fallback path:
    // Chunk: [0x0E, 0xEE] -> val 0x0EEE.
    // write_data(0x0EEE) called.
    // CRAM writes handle packing.
    // 0x0EEE -> stored as 0x0E, 0xEE in CRAM (little endian in CRAM array? check `write_data`)

    // write_data logic for CRAM:
    // cram[addr] = val & 0xFF
    // cram[addr+1] = val >> 8
    // So cram[0] = 0xEE, cram[1] = 0x0E.

    assert_eq!(vdp.cram[0], 0xEE);
    assert_eq!(vdp.cram[1], 0x0E);
    // Addr increments by 2 -> 0x0002.
    assert_eq!(vdp.control_address, 0x0002);
}
