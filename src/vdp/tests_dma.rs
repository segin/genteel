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
    // Command: VRAM Write (0x1).
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
}

#[test]
fn test_dma_fill_wrap_around() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0 (Reg 19=0, 20=0) -> Should be interpreted as 0x10000
    vdp.write_control(0x9300);
    vdp.write_control(0x9400);

    // 3. Set DMA Mode to Fill (Reg 23 = 0x80)
    vdp.write_control(0x9780);

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending, "DMA pending should be true for Fill setup");

    // 6. Write Fill Data (0xBB)
    vdp.write_data(0xBB00);

    assert!(!vdp.dma_pending, "DMA pending should be false after completion");

    // 7. Verify VRAM - Should fill entire 64KB
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xBB, "Mismatch at index 0x{:04X}", i);
    }
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
}

#[test]
fn test_dma_copy_wrap_around() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Initialize VRAM with a pattern
    for i in 0..0x10000 {
        vdp.vram[i] = (i & 0xFF) as u8;
    }

    // 3. Set DMA Length to 0 (Reg 19=0, 20=0) -> Should be interpreted as 0x10000
    vdp.write_control(0x9300);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Reg 23 = 0xC0) and Source Address 0x0002
    // Source Address 0x0002 -> Reg 21 = 1 (bit 1 of source is 1, rest 0)
    vdp.write_control(0x9501); // Reg 21 = 0x01 (Source LO)
    vdp.write_control(0x9600); // Reg 22 = 0x00 (Source MID)
    vdp.write_control(0x97C0); // Reg 23 = 0xC0 (Mode=Copy, Source HI=0)

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending, "DMA pending should be true");

    // 7. Execute DMA manually
    let len = vdp.execute_dma();

    assert!(!vdp.dma_pending, "DMA pending should be false after execution");
    assert_eq!(len, 0x10000, "Length should be 0x10000 (64KB)");

    // 8. Verify VRAM
    // We copied 64KB from source 0x0002 to dest 0x0000.
    // Both wrap around 0xFFFF -> 0x0000.
    // Destination 0x0000 gets Source 0x0002.
    // ...
    // Destination 0xFFFD gets Source 0xFFFF.
    // Destination 0xFFFE gets Source 0x0000 (wrapped source).
    // Destination 0xFFFF gets Source 0x0001.

    // Note: The copy happens sequentially. But since source > dest, we are copying "forward"
    // so we overwrite dest before source reads it IF source < dest.
    // Here source (2) > dest (0).
    // So when we write to 0, we read from 2.
    // When we write to 2, we read from 4.
    // So we don't overwrite source data before reading it until we wrap.
    // However, when we reach dest=0xFFFE, source=0x0000.
    // We read from 0x0000. But 0x0000 was already written at step 0!
    // So we read the NEW value of 0x0000.
    // Step 0: dest[0] = source[2] (old val of 2).
    // ...
    // Step 0xFFFE: dest[0xFFFE] = source[0] (new val of 0, which is old val of 2).
    // Step 0xFFFF: dest[0xFFFF] = source[1] (new val of 1, which is old val of 3).

    // Let's verify this behavior.
    // Old values: vram[i] = i & 0xFF.
    // vram[0] becomes 2.
    // vram[1] becomes 3.
    // ...
    // vram[0xFFFE] becomes vram[0] (which is now 2).
    // vram[0xFFFF] becomes vram[1] (which is now 3).

    for i in 0..0xFFFE {
        let expected = ((i + 2) & 0xFF) as u8;
        assert_eq!(vdp.vram[i], expected, "Mismatch at index 0x{:04X}", i);
    }

    // Wrapped values
    assert_eq!(vdp.vram[0xFFFE], 2, "Mismatch at wrap 0xFFFE"); // vram[0]
    assert_eq!(vdp.vram[0xFFFF], 3, "Mismatch at wrap 0xFFFF"); // vram[1]
}
