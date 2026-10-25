use super::*;

#[test]
fn test_dma_fill_vram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

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
    assert!(
        vdp.command.dma_pending,
        "DMA pending should be true for Fill setup"
    );

    // 6. Write Fill Data (e.g. 0xAA)
    // Writing to data port triggers the fill in hardware.
    // This replaces the normal write.
    // It fills `dma_length` bytes starting at `control_address` (0x0000).
    vdp.write_data(0xAA00);

    assert!(
        !vdp.command.dma_pending,
        "DMA pending should be false after data write"
    );

    // 7. Verify VRAM (GPGX `vdp_dma_fill`): the triggering word 0xAA00 is
    // written normally at 0x0000 (MSB at 0, LSB at 1), then 16 fill steps
    // write the MSB to (address ^ 1) for addresses 0x0001..=0x0010.
    assert_eq!(vdp.vram[0], 0xAA, "fill step at 0x0001 lands on 0x0000");
    assert_eq!(vdp.vram[1], 0x00, "LSB of the triggering word survives");
    for i in 2..0x10 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
    // The last step (address 0x0010) lands on 0x0011, leaving 0x0010.
    assert_eq!(vdp.vram[0x10], 0x00);
    assert_eq!(vdp.vram[0x11], 0xAA);
    assert_eq!(vdp.vram[0x12], 0x00, "Should stop after 16 fill steps");
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 0, "length counts down to 0");
    assert_eq!(vdp.registers[REG_DMA_LEN_HI], 0);
}

#[test]
fn test_dma_copy_vram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

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
    // Copy source is a plain byte VRAM address (regs 21/22, no <<1), so
    // source 0x1000 = reg21 (low) 0x00, reg22 (high) 0x10.
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9610); // Reg 22 = 0x10 -> byte source 0x1000
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0000)
    // Command 0x30 (VRAM copy: CD5|CD4) at 0x0000.
    vdp.write_control(0x0000);
    vdp.write_control(0x00C0);

    assert!(vdp.command.dma_pending);

    // 7. Execute DMA
    let cycles = vdp.execute_dma().unwrap();

    assert!(!vdp.command.dma_pending);
    assert_eq!(cycles, 0x10);
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 0);
    assert_eq!(vdp.dma_copy_source(), 0x1010, "source advances by length");

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
    vdp.bypass_fifo = true;

    // 1. Enable DMA (Reg 1 bit 4)
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0 (0x0000)
    vdp.write_control(0x9300); // Reg 19 = 0x00
    vdp.write_control(0x9400); // Reg 20 = 0x00

    // 3. Set DMA Mode to Fill (Reg 23 bits 7,6 = 1,0) -> 0x80
    vdp.write_control(0x9780); // Reg 23 = 0x80

    // 4. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 5. Setup DMA Fill destination (VRAM 0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(
        vdp.command.dma_pending,
        "DMA pending should be true for Fill setup"
    );

    // 6. Write Fill Data (e.g. 0xAA)
    // This triggers the fill of 64KB (length 0 treated as 0x10000)
    vdp.write_data(0xAA00);

    assert!(
        !vdp.command.dma_pending,
        "DMA pending should be false after data write"
    );

    // 7. Verify VRAM
    // 0x10000 fill steps from address 1 cover every (address ^ 1), so the
    // whole of VRAM ends up holding the MSB.
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xAA, "Mismatch at index 0x{:04X}", i);
    }
}

#[test]
fn test_dma_copy_wrap_around() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Setup initial VRAM state
    // The copy reads (source ^ 1), so the first byte read is vram[1].
    vdp.vram[1] = 0xFF;

    // 3. Set DMA Length to 0 (0x0000) -> 64KB
    vdp.write_control(0x9300);
    vdp.write_control(0x9400);

    // 4. Set DMA Mode to Copy (Reg 23 = 0xC0) and Source Address 0x0000
    // Source: 0x000000. Reg 21=0, Reg 22=0, Reg 23=0xC0
    vdp.write_control(0x9500); // Reg 21 = 0x00
    vdp.write_control(0x9600); // Reg 22 = 0x00
    vdp.write_control(0x97C0); // Reg 23 = 0xC0

    // 5. Set Auto-increment to 1
    vdp.write_control(0x8F01);

    // 6. Setup Destination (0x0001) - overlapping copy
    // Command 0x4000 (VRAM Write) + addr 0x0001
    // Word 1: 0x0001
    // Word 2: 0x00C0 (CD5|CD4: VRAM copy).
    vdp.write_control(0x0001);
    vdp.write_control(0x00C0);

    assert!(vdp.command.dma_pending);

    // 7. Execute DMA
    // Length 0 -> 0x10000 bytes.
    // Copy from 0 to 1.
    // This overlapping copy should propagate 0xFF through the entire VRAM.
    let count = vdp.execute_dma().unwrap();

    assert!(!vdp.command.dma_pending);
    assert_eq!(count, 0x10000);

    // Verify VRAM
    for i in 0..0x10000 {
        assert_eq!(vdp.vram[i], 0xFF, "Mismatch at index 0x{:04X}", i);
    }
}

/// DMA Fill during active display: 2 cycles per byte of 68k stall.
#[test]
fn test_dma_fill_charges_stall_cycles_active_display() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_DMA_LEN_LO] = 0x10; // 16 bytes
    vdp.registers[REG_DMA_LEN_HI] = 0x00;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.command.dma_pending = true;
    vdp.command.code = VRAM_WRITE;
    // Ensure we're NOT in VBlank.
    vdp.status &= !STATUS_VBLANK;

    let _ = vdp.execute_dma();
    assert_eq!(vdp.take_dma_stall_cycles(), 16 * 2);
    assert_eq!(vdp.take_dma_stall_cycles(), 0);
}

/// DMA Fill during VBlank: 1 cycle per 4 bytes (8x cheaper).
#[test]
fn test_dma_fill_charges_stall_cycles_vblank() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_DMA_LEN_LO] = 0x10; // 16 bytes
    vdp.registers[REG_DMA_LEN_HI] = 0x00;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_FILL;
    vdp.command.dma_pending = true;
    vdp.command.code = VRAM_WRITE;
    vdp.status |= STATUS_VBLANK;

    let _ = vdp.execute_dma();
    assert_eq!(vdp.take_dma_stall_cycles(), 16 / 4);
}

/// DMA Copy during active display: 4 cycles per byte (read + write).
#[test]
fn test_dma_copy_charges_stall_cycles_active_display() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_DMA_LEN_LO] = 0x08; // 8 bytes
    vdp.registers[REG_DMA_LEN_HI] = 0x00;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.command.dma_pending = true;
    vdp.command.code = 0x30;
    vdp.status &= !STATUS_VBLANK;

    let _ = vdp.execute_dma();
    assert_eq!(vdp.take_dma_stall_cycles(), 8 * 4);
}

/// DMA Copy during VBlank: 1 cycle per 2 bytes.
#[test]
fn test_dma_copy_charges_stall_cycles_vblank() {
    let mut vdp = Vdp::new();
    vdp.registers[REG_DMA_LEN_LO] = 0x08; // 8 bytes
    vdp.registers[REG_DMA_LEN_HI] = 0x00;
    vdp.registers[REG_DMA_SRC_HI] = DMA_MODE_COPY;
    vdp.command.dma_pending = true;
    vdp.command.code = 0x30;
    vdp.status |= STATUS_VBLANK;

    let _ = vdp.execute_dma();
    assert_eq!(vdp.take_dma_stall_cycles(), 8 / 2);
}
