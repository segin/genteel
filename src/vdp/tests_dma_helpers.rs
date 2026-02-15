use super::*;

#[test]
fn test_dma_mode() {
    let mut vdp = Vdp::new();

    // Default is 0
    assert_eq!(vdp.dma_mode(), 0);

    // Set Fill Mode (0x80)
    vdp.registers[REG_DMA_SRC_HI] = 0x80;
    assert_eq!(vdp.dma_mode(), 0x80);

    // Set Copy Mode (0xC0)
    vdp.registers[REG_DMA_SRC_HI] = 0xC0;
    assert_eq!(vdp.dma_mode(), 0xC0);

    // Set some random mode/address mix
    vdp.registers[REG_DMA_SRC_HI] = 0x95;
    assert_eq!(vdp.dma_mode(), 0x95);
}

#[test]
fn test_dma_source() {
    let mut vdp = Vdp::new();

    // Reg 21 (LO) = 0x10 -> 0x10 << 1 = 0x20 (A1-A8)
    // Reg 22 (MID) = 0x20 -> 0x20 << 9 = 0x4000 (A9-A16)
    // Reg 23 (HI) = 0x05 -> 0x05 << 17 = 0xA0000 (A17-A23)

    vdp.registers[REG_DMA_SRC_LO] = 0x10;
    vdp.registers[REG_DMA_SRC_MID] = 0x20;
    vdp.registers[REG_DMA_SRC_HI] = 0x05;

    let expected = 0xA4020;
    assert_eq!(vdp.dma_source(), expected);
}

#[test]
fn test_dma_source_wrapping() {
    let mut vdp = Vdp::new();

    // Test full range values for address parts
    // REG_DMA_SRC_HI also contains mode bits, which are included in dma_source()
    vdp.registers[REG_DMA_SRC_LO] = 0xFF;
    vdp.registers[REG_DMA_SRC_MID] = 0xFF;
    vdp.registers[REG_DMA_SRC_HI] = 0xFF;

    // 0xFF << 1 = 0x1FE
    // 0xFF << 9 = 0x1FE00
    // 0xFF << 17 = 0x1FE0000

    let expected = 0x1FE0000 | 0x1FE00 | 0x1FE;
    assert_eq!(vdp.dma_source(), expected);
}

#[test]
fn test_dma_length() {
    let mut vdp = Vdp::new();

    vdp.registers[REG_DMA_LEN_LO] = 0xAA;
    vdp.registers[REG_DMA_LEN_HI] = 0x55;

    // 0x55AA
    assert_eq!(vdp.dma_length(), 0x55AA);
}

#[test]
fn test_dma_source_transfer() {
    let mut vdp = Vdp::new();

    // Set HI to 0xFF. Since dma_source_transfer masks with 0x3F,
    // it effectively uses 0x3F (0011 1111).
    // Bits 6 and 7 are ignored/masked out.
    vdp.registers[REG_DMA_SRC_HI] = 0xFF;
    vdp.registers[REG_DMA_SRC_MID] = 0xFF;
    vdp.registers[REG_DMA_SRC_LO] = 0xFF;

    // 0x3F << 17 = 0x7E0000
    // 0xFF << 9 = 0x1FE00
    // 0xFF << 1 = 0x1FE

    let expected = 0x7E0000 | 0x1FE00 | 0x1FE;
    assert_eq!(vdp.dma_source_transfer(), expected);
}
