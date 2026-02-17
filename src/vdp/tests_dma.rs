use super::*;

#[test]
fn test_dma_fill_vram() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA (Reg 1 bit 4)
    // Mode Register 2: 0x81 (bit 7) | 0x14 (DMA=1, Display=0) -> 0x94?
    // Wait, Mode Reg 2 is Reg 1.
    // Reg 1 defaults: 0x04?
    // We want to set Reg 1 to 0x14 (Display OFF, DMA ON, V30 OFF, etc).
    // Command: 0x8114.
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0x10 bytes (8 words? No, reg 19/20 is in words usually, but for fill it's bytes? No, usually words).
    // The code: for _ in 0..dma_length.
    // And each iteration writes 1 byte to VRAM (if VRAM fill).
    // "let mut dma_length = ...".
    // "VRAM Fill ... self.vram[addr] = fill_data".
    // So it fills `dma_length` bytes?
    // Documentation says Reg 19/20 is length in words (16-bit).
    // If code loops `dma_length` times and writes bytes, then it treats it as byte count?
    // Or maybe the loop counts words but writes bytes?
    // The loop body executes once per count.
    // If VRAM fill writes bytes, and loop runs N times, it writes N bytes.
    // If Reg 19/20 holds N, and N is supposed to be words, then it should write 2*N bytes?
    // Or maybe VRAM Fill treats Reg 19/20 as bytes?
    // Standard docs: "DMA Length Counter ... in words".
    // But for Fill? "The VDP will perform the fill operation for the specified number of bytes... wait, usually words."
    // Actually, VRAM fill writes the high byte of the data port to VRAM.
    // Address increments by 1 usually? No, by auto-increment.
    // If auto-increment is 1, it writes every byte.
    // If auto-increment is 2, it writes every other byte.
    // The code: "self.control_address = self.control_address.wrapping_add(self.auto_increment() as u16);"
    // So it respects auto-increment.
    // So if I set Length=0x10. It writes 0x10 times.
    // If auto-increment is 1, it fills 0x10 bytes.

    vdp.write_control(0x9310); // Reg 19 = 0x10
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0) -> 0x80
    // And destination is determined by Control Code? Yes.
    // Reg 23 also holds high bits of source, but for Fill it's ignored or used as mode.
    vdp.write_control(0x9780); // Reg 23 = 0x80

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    // Command: VRAM Write (0x1).
    // Addr 0x0000.
    // Word 1: 0x4000.
    // Word 2: DMA bit (CD5) IS set for Fill. (0x0080).

    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    // Check if dma_pending is set
    // It should be set because CD5 is 1
    assert!(vdp.dma_pending, "DMA pending should be true for Fill setup");

    // 6. Write Fill Data (e.g. 0xAA)
    // Writing to data port triggers the fill in hardware.
    // This replaces the normal write.
    // It fills `dma_length` bytes starting at `control_address` (0x0000).
    vdp.write_data(0xAA00);

    assert!(
        !vdp.dma_pending,
        "DMA pending should be false after data write"
    );

    // 7. Verify VRAM
    // Length is 0x10 (16 bytes).
    // Writes 16 bytes: indices 0x0000 to 0x000F.
    // 0x0000..0x000F is 0xAA.
    // 0x0010 is 0x00.

    for i in 0..0x10 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
    assert_eq!(vdp.vram[0x10], 0x00, "Should stop at 0x10");
}

#[test]
#[ignore]
fn test_dma_copy_vram() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup Source Data at 0x1000
    // We can write directly to vram for setup to be easier
    for i in 0..0x10 {
        vdp.vram[0x1000 + i] = (i as u8) + 1; // 1, 2, 3...
    }

    // 3. Set DMA Length to 0x10
    vdp.write_control(0x9310);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Mode 3) and Source Address
    // Mode 3 is bits 7,6 = 11. Reg 23 = 0xC0.
    // Source Address 0x1000.
    // Reg 21 (A1-A8): 0x00.
    // Reg 22 (A9-A16): 0x08 (A12=1).
    // Reg 23 (A17-A23): 0x00 (masked with 0x3F).
    // So Reg 23 is 0xC0.
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9608); // Reg 22 = 0x08
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    // Command 0x21 (DMA VRAM Write) at 0x0000.
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending);

    // 7. Execute DMA
    let cycles = vdp.execute_dma();

    assert!(!vdp.dma_pending);
    assert_eq!(cycles, 0x10);

    // Verify VRAM
    // 0x0000 should contain what was at 0x1000
    for i in 0..0x10 {
        let expected = (i as u8) + 1;
        assert_eq!(vdp.vram[i], expected, "Mismatch at index 0x{:04X}", i);
    }
}

#[test]
fn test_dma_fill_wrap_around() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0 (which means 0x10000 / 64KB)
    vdp.write_control(0x9300); // Reg 19 = 0x00
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 = 0x80)
    vdp.write_control(0x9780);

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    // Command 0x1 (VRAM Write) at 0x0000 + DMA bit (0x80)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending);

    // 6. Write Fill Data (e.g. 0xBB)
    // This triggers the fill of 0x10000 bytes
    vdp.write_data(0xBB00);

    assert!(!vdp.dma_pending);

    // 7. Verify VRAM - All 64KB should be filled
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xBB, "Mismatch at index 0x{:04X}", i);
    }
}

#[test]
fn test_dma_copy_wrap_around() {
    let mut vdp = Vdp::new();

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup VRAM with a pattern so we can verify copy
    for i in 0..0x10000 {
        vdp.vram[i] = (i & 0xFF) as u8;
    }

    // 3. Set DMA Length to 0 (which means 0x10000 / 64KB)
    vdp.write_control(0x9300); // Reg 19 = 0x00
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 4. Set DMA Mode to Copy (Reg 23 = 0xC0) and Source Address
    // We'll copy from 0x8000 to 0x0000.
    // Source: 0x8000.
    // Reg 21 (A1-A8): 0x00
    // Reg 22 (A9-A16): 0x40 (A15=1 => 0x8000 >> 1 = 0x4000? No. Source is address >> 1 usually? No.
    // Let's check dma_source() implementation:
    // ((self.registers[REG_DMA_SRC_HI] as u32) << 17) | ((self.registers[REG_DMA_SRC_MID] as u32) << 9) | ((self.registers[REG_DMA_SRC_LO] as u32) << 1)
    // It constructs a 24-bit address.
    // If we want 0x8000 (32768):
    // 0x8000 = 1 0000 0000 0000 0000
    // Bit 15 set.
    // Reg 21 (LO) covers bits 1-8.
    // Reg 22 (MID) covers bits 9-16.
    // Reg 23 (HI) covers bits 17-23.
    // We want bit 15 set. That is bit 6 of Reg 22?
    // (Reg 22 << 9) covers 9..16.
    // Bit 15 corresponds to bit (15-9)=6 of Reg 22.
    // So Reg 22 = 0x40 (bit 6 set).
    // Reg 21 = 0.
    // Reg 23 = 0xC0 (Mode Copy + 0 address bits).

    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9640); // Reg 22 = 0x40
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.dma_pending);

    // 7. Execute DMA manually (since Copy is triggered by command write and executed by step/cpu usually)
    let cycles = vdp.execute_dma();

    assert!(!vdp.dma_pending);
    // Should process 0x10000 items
    assert_eq!(cycles, 0x10000);

    // 8. Verify Copy
    // We copied from 0x8000 to 0x0000, length 0x10000.
    // Source range: 0x8000 .. 0x18000 (wrapping at 0x10000).
    // Dest range: 0x0000 .. 0x10000.
    // Since copy loop is: vram[dest] = vram[src].
    // Since src starts ahead of dest (0x8000 > 0x0000), it reads initial values until src wraps.
    // When i=0x8000, src wraps to 0x0000. Dest is 0x8000.
    // At this point, vram[0x0000] has already been written (at i=0).
    // So from i=0x8000 onwards, it reads the *new* values from 0x0000..0x8000.
    // Initial pattern: vram[x] = x & 0xFF.
    // i=0..0x7FFF: src=0x8000+i. dest=i.
    // vram[i] = vram[0x8000+i] = (0x8000+i) & 0xFF.
    // i=0x8000..0xFFFF: src=i (wrapped). dest=i.
    // vram[i] = vram[src] = vram[i-0x8000] (new value).
    // Wait, src at i=0x8000 is (0x8000 + 0x8000) & 0xFFFF = 0x0000.
    // dest is 0x8000.
    // vram[0x8000] = vram[0x0000].
    // vram[0x0000] was set to (0x8000) & 0xFF = 0x00.
    // So vram[0x8000] becomes 0x00.
    // Generally:
    // Low half (0..0x7FFF) gets High half initial values.
    // High half (0x8000..0xFFFF) gets Low half new values (which are High half initial values).
    // So effectively, the whole VRAM should contain the pattern from the initial High half.
    // Initial High half pattern: (0x8000+k) & 0xFF.
    // Let's verify a few points.

    // Index 0: Should be initial vram[0x8000] = 0x00.
    assert_eq!(vdp.vram[0], 0x00);
    // Index 1: Should be initial vram[0x8001] = 0x01.
    assert_eq!(vdp.vram[1], 0x01);

    // Index 0x8000: Should be new vram[0x0000] = 0x00.
    assert_eq!(vdp.vram[0x8000], 0x00);
    // Index 0x8001: Should be new vram[0x0001] = 0x01.
    assert_eq!(vdp.vram[0x8001], 0x01);

    // So it seems it copies correctly with wrapping.
}
