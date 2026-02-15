use super::*;

#[test]
fn test_dma_fill_vram() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA (Reg 1 bit 4)
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0x10 bytes
    vdp.write_control(0x9310); // Reg 19 = 0x10
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0) -> 0x80
    vdp.write_control(0x9780); // Reg 23 = 0x80

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    // Check if dma_pending is set
    assert!(vdp.dma_pending, "DMA pending should be true for Fill setup");

    // 6. Write Fill Data (e.g. 0xAA)
    vdp.write_data(0xAA00);

    assert!(
        !vdp.dma_pending,
        "DMA pending should be false after data write"
    );

    // 7. Verify VRAM
    for i in 0..0x10 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
    assert_eq!(vdp.vram[0x10], 0x00, "Should stop at 0x10");

    // Verify DMA Length registers are cleared
    assert_eq!(vdp.registers[19], 0, "DMA Length Low should be 0");
    assert_eq!(vdp.registers[20], 0, "DMA Length High should be 0");
}

#[test]
#[ignore]
fn test_dma_copy_vram() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup Source Data at 0x1000
    for i in 0..0x10 {
        vdp.vram[0x1000 + i] = (i as u8) + 1; // 1, 2, 3...
    }

    // 3. Set DMA Length to 0x10
    vdp.write_control(0x9310);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Mode 3) and Source Address
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9608); // Reg 22 = 0x08
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending);

    // 7. Execute DMA
    let cycles = vdp.execute_dma();

    assert!(!vdp.dma_pending);
    assert_eq!(cycles, 0x10);

    // Verify VRAM
    for i in 0..0x10 {
        let expected = (i as u8) + 1;
        assert_eq!(vdp.vram[i], expected, "Mismatch at index 0x{:04X}", i);
    }

    // Verify DMA Length registers are cleared
    assert_eq!(vdp.registers[19], 0, "DMA Length Low should be 0");
    assert_eq!(vdp.registers[20], 0, "DMA Length High should be 0");
}

#[test]
fn test_dma_fill_via_execute() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA (Reg 1 bit 4)
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0x10 bytes
    vdp.write_control(0x9310);
    vdp.write_control(0x9400);

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0)
    vdp.write_control(0x9780);

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0100)
    vdp.write_control(0x8104); // Disable DMA
    vdp.write_data(0xBB00);    // last_data_write = 0xBB00
    vdp.write_control(0x8114); // Re-enable DMA

    vdp.write_control(0x4100);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending);

    // 6. Execute DMA
    let cycles = vdp.execute_dma();

    assert!(!vdp.dma_pending);
    assert_eq!(cycles, 0x10);

    // 7. Verify VRAM at 0x0100
    for i in 0..0x10 {
        assert_eq!(vdp.vram[0x0100 + i], 0xBB, "Mismatch at index 0x{:04X}", 0x0100 + i);
    }
}
