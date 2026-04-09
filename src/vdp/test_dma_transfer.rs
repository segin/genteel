use super::*;

#[test]
fn test_dma_memory_to_vram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 0x02 words
    vdp.write_control(0x9302);
    vdp.write_control(0x9400);

    // 3. Set DMA Source Address to 0x123456
    // Word address: 0x123456 >> 1 = 0x91A2B
    // Reg 21 (LO): 0x2B
    // Reg 22 (MID): 0x1A
    // Reg 23 (HI): 0x09 (with bit 7=0, bit 6=0 for memory transfer)
    vdp.write_control(0x952B); // Reg 21
    vdp.write_control(0x961A); // Reg 22
    vdp.write_control(0x9709); // Reg 23

    // 4. Set Auto-increment to 2
    vdp.write_control(0x8F02);

    // 5. Setup Destination (VRAM 0x0000)
    // Command 0x4000 (VRAM Write) + addr 0x0000
    // Word 2: 0x0080 (DMA bit set)
    vdp.write_control(0x4000);
    vdp.write_control(0x0080);

    assert!(vdp.command.dma_pending);

    // Execute first step of DMA
    vdp.step_dma(&mut |addr| {
        assert_eq!(addr, 0x123456);
        0xABCD
    });

    // Check VRAM write (word write: MSB at addr, LSB at addr ^ 1)
    assert_eq!(vdp.vram[0], 0xAB);
    assert_eq!(vdp.vram[1], 0xCD);

    // Check address and length update
    assert_eq!(vdp.command.address, 2);
    // Length decrement
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 1);
    // Source address increment (+2 bytes = 1 word)
    assert_eq!(vdp.registers[REG_DMA_SRC_LO], 0x2C);

    // Execute second step
    vdp.step_dma(&mut |addr| {
        assert_eq!(addr, 0x123458);
        0x1234
    });

    assert_eq!(vdp.vram[2], 0x12);
    assert_eq!(vdp.vram[3], 0x34);

    assert_eq!(vdp.command.address, 4);
    assert_eq!(vdp.registers[REG_DMA_LEN_LO], 0);
    assert!(!vdp.command.dma_pending);
}

#[test]
fn test_dma_memory_to_cram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 1 word
    vdp.write_control(0x9301);
    vdp.write_control(0x9400);

    // 3. Set DMA Source Address to 0x000000
    vdp.write_control(0x9500); // Reg 21
    vdp.write_control(0x9600); // Reg 22
    vdp.write_control(0x9700); // Reg 23

    // 4. Set Auto-increment to 2
    vdp.write_control(0x8F02);

    // 5. Setup Destination (CRAM 0x0000)
    // Command 0xC000 (CRAM Write) + addr 0x0000
    // Word 2: 0x0080 (DMA bit set)
    vdp.write_control(0xC000);
    vdp.write_control(0x0080);

    assert!(vdp.command.dma_pending);

    // Provide word for CRAM
    vdp.step_dma(&mut |_addr| 0x0EEE);

    // Check CRAM (stored as LSB then MSB? No, wait: index * 2)
    // CRAM: idx = addr / 2.
    // cram[idx*2] = val & 0xFF, cram[idx*2+1] = val >> 8
    assert_eq!(vdp.cram[0], 0xEE);
    assert_eq!(vdp.cram[1], 0x0E);
    // Check CRAM cache is updated
    assert_eq!(vdp.cram_cache[0], Vdp::genesis_color_to_rgb565(0x0EEE));

    assert!(!vdp.command.dma_pending);
}

#[test]
fn test_dma_memory_to_vsram() {
    let mut vdp = Vdp::new();
    vdp.bypass_fifo = true;

    // 1. Enable DMA
    vdp.write_control(0x8114);

    // 2. Set DMA Length to 1 word
    vdp.write_control(0x9301);
    vdp.write_control(0x9400);

    // 3. Set DMA Source Address to 0x000000
    vdp.write_control(0x9500); // Reg 21
    vdp.write_control(0x9600); // Reg 22
    vdp.write_control(0x9700); // Reg 23

    // 4. Set Auto-increment to 2
    vdp.write_control(0x8F02);

    // 5. Setup Destination (VSRAM 0x0000)
    // Command 0x4000 + 0x10 CD bit (0x4000 -> VRAM, but for VSRAM it's 0x4000 and CD4=1 -> 0x4000 in word 1, 0x0010 in word 2. Wait:
    // Command 5 is VSRAM WRITE: 0b0101.
    // Word 1: 0x4000. Word 2: 0x0090 (DMA=0x80 | VSRAM=0x10)
    vdp.write_control(0x4000);
    vdp.write_control(0x0090);

    assert!(vdp.command.dma_pending);

    // Provide word for VSRAM
    vdp.step_dma(&mut |_addr| 0x03FF);

    // Check VSRAM (idx = addr)
    // vsram[0] = val >> 8, vsram[1] = val & 0xFF
    assert_eq!(vdp.vsram[0], 0x03);
    assert_eq!(vdp.vsram[1], 0xFF);

    assert!(!vdp.command.dma_pending);
}
