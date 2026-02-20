use super::*;
use proptest::prelude::*;

#[test]
fn test_dma_mode() {
    let mut vdp = Vdp::new();

    // Check initial state (should be 0)
    assert_eq!(vdp.dma_mode(), 0, "Initial dma_mode should be 0");

    // Set Register 23 (REG_DMA_SRC_HI) to various values

    // Mode 0: Memory to VRAM (Bit 7=0, Bit 6=0)
    vdp.registers[REG_DMA_SRC_HI] = 0x00;
    assert_eq!(vdp.dma_mode(), 0x00);

    // Mode 1: VRAM Fill (Bit 7=1, Bit 6=0) -> 0x80
    vdp.registers[REG_DMA_SRC_HI] = 0x80;
    assert_eq!(vdp.dma_mode(), 0x80);

    // Mode 2: VRAM Copy (Bit 7=1, Bit 6=1) -> 0xC0
    vdp.registers[REG_DMA_SRC_HI] = 0xC0;
    assert_eq!(vdp.dma_mode(), 0xC0);

    // Test with lower bits set (should be included in mode value as it returns the whole register)
    vdp.registers[REG_DMA_SRC_HI] = 0x80 | 0x1F; // Fill mode + some address bits
    assert_eq!(vdp.dma_mode(), 0x9F);
}

#[test]
fn test_dma_source() {
    let mut vdp = Vdp::new();

    // Test Case 1: All zeros
    vdp.registers[REG_DMA_SRC_LO] = 0;
    vdp.registers[REG_DMA_SRC_MID] = 0;
    vdp.registers[REG_DMA_SRC_HI] = 0;
    assert_eq!(vdp.dma_source(), 0);

    // Test Case 2: Bit patterns
    // Reg 21 (LO) << 1.   Max: 0xFF << 1 = 0x1FE.
    // Reg 22 (MID) << 9.  Max: 0xFF << 9 = 0x1FE00.
    // Reg 23 (HI) << 17.  Max: 0xFF << 17 = 0x1FE0000.

    vdp.registers[REG_DMA_SRC_LO] = 0x01; // Bit 0 set -> Address bit 1
    vdp.registers[REG_DMA_SRC_MID] = 0;
    vdp.registers[REG_DMA_SRC_HI] = 0;
    assert_eq!(vdp.dma_source(), 2);

    vdp.registers[REG_DMA_SRC_LO] = 0;
    vdp.registers[REG_DMA_SRC_MID] = 0x01; // Bit 0 set -> Address bit 9
    vdp.registers[REG_DMA_SRC_HI] = 0;
    assert_eq!(vdp.dma_source(), 512);

    vdp.registers[REG_DMA_SRC_LO] = 0;
    vdp.registers[REG_DMA_SRC_MID] = 0;
    vdp.registers[REG_DMA_SRC_HI] = 0x01; // Bit 0 set -> Address bit 17
    assert_eq!(vdp.dma_source(), 131072);

    // Test Case 3: Combined
    // Reg 21 = 0x55 (01010101) -> ...10101010
    // Reg 22 = 0xAA (10101010) -> ...10101010 000000000
    // Reg 23 = 0x0F (00001111) -> ...11110 000000000 000000000
    vdp.registers[REG_DMA_SRC_LO] = 0x55;
    vdp.registers[REG_DMA_SRC_MID] = 0xAA;
    vdp.registers[REG_DMA_SRC_HI] = 0x0F;

    let expected = ((0x0F as u32) << 17) | ((0xAA as u32) << 9) | ((0x55 as u32) << 1);
    assert_eq!(vdp.dma_source(), expected);

    // Test Case 4: With Mode Bits set in Reg 23
    // Even if mode bits are set, dma_source() includes them in the shift.
    vdp.registers[REG_DMA_SRC_HI] = 0x8F; // Fill mode + 0x0F address
    let expected_with_mode = ((0x8F as u32) << 17) | ((0xAA as u32) << 9) | ((0x55 as u32) << 1);
    assert_eq!(vdp.dma_source(), expected_with_mode);
}

#[test]
fn test_dma_source_transfer() {
    let mut vdp = Vdp::new();

    // Setup registers
    vdp.registers[REG_DMA_SRC_LO] = 0x55;
    vdp.registers[REG_DMA_SRC_MID] = 0xAA;

    // Case 1: Reg 23 has no mode bits
    vdp.registers[REG_DMA_SRC_HI] = 0x0F;
    let expected_base = ((0x0F as u32) << 17) | ((0xAA as u32) << 9) | ((0x55 as u32) << 1);
    assert_eq!(vdp.dma_source_transfer(), expected_base);

    // Case 2: Reg 23 has mode bit 7 set (0x80)
    // dma_source_transfer should ignore bit 7.
    vdp.registers[REG_DMA_SRC_HI] = 0x8F; // 0x80 | 0x0F
                                          // It should return same as if it was 0x0F
    assert_eq!(vdp.dma_source_transfer(), expected_base);

    // Case 3: Reg 23 has bit 6 set (0x40) -> RAM Transfer
    vdp.registers[REG_DMA_SRC_HI] = 0x4F;
    let expected_ram = 0xFF0000 | ((0xAA as u32) << 9) | ((0x55 as u32) << 1);
    assert_eq!(vdp.dma_source_transfer(), expected_ram);
}

#[test]
fn test_dma_length() {
    let mut vdp = Vdp::new();

    // Test length calculation
    vdp.registers[REG_DMA_LEN_LO] = 0xFF;
    vdp.registers[REG_DMA_LEN_HI] = 0x00;
    assert_eq!(vdp.dma_length(), 0xFF);

    vdp.registers[REG_DMA_LEN_LO] = 0x00;
    vdp.registers[REG_DMA_LEN_HI] = 0xFF;
    assert_eq!(vdp.dma_length(), 0xFF00);

    vdp.registers[REG_DMA_LEN_LO] = 0x12;
    vdp.registers[REG_DMA_LEN_HI] = 0x34;
    assert_eq!(vdp.dma_length(), 0x3412);
}

#[test]
fn test_dma_type_checks() {
    let mut vdp = Vdp::new();

    // Transfer: Bit 7 = 0
    vdp.registers[REG_DMA_SRC_HI] = 0x00;
    assert!(vdp.is_dma_transfer());
    assert!(!vdp.is_dma_fill());

    vdp.registers[REG_DMA_SRC_HI] = 0x7F;
    assert!(vdp.is_dma_transfer());
    assert!(!vdp.is_dma_fill());

    // Fill: Bit 7 = 1, Bit 6 = 0
    vdp.registers[REG_DMA_SRC_HI] = 0x80;
    assert!(!vdp.is_dma_transfer());
    assert!(vdp.is_dma_fill());

    vdp.registers[REG_DMA_SRC_HI] = 0xBF;
    assert!(!vdp.is_dma_transfer());
    assert!(vdp.is_dma_fill());

    // Copy: Bit 7 = 1, Bit 6 = 1
    vdp.registers[REG_DMA_SRC_HI] = 0xC0;
    assert!(!vdp.is_dma_transfer());
    assert!(!vdp.is_dma_fill());

    vdp.registers[REG_DMA_SRC_HI] = 0xFF;
    assert!(!vdp.is_dma_transfer());
    assert!(!vdp.is_dma_fill());
}

proptest! {
    #[test]
    fn test_dma_mode_prop(val in 0u8..=255) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_SRC_HI] = val;
        prop_assert_eq!(vdp.dma_mode(), val);
    }

    #[test]
    fn test_dma_source_prop(hi in 0u8..=255, mid in 0u8..=255, lo in 0u8..=255) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_SRC_HI] = hi;
        vdp.registers[REG_DMA_SRC_MID] = mid;
        vdp.registers[REG_DMA_SRC_LO] = lo;

        let expected = ((hi as u32) << 17) |
                       ((mid as u32) << 9) |
                       ((lo as u32) << 1);

        prop_assert_eq!(vdp.dma_source(), expected);
    }

    #[test]
    fn test_dma_source_transfer_prop(hi in 0u8..=255, mid in 0u8..=255, lo in 0u8..=255) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_SRC_HI] = hi;
        vdp.registers[REG_DMA_SRC_MID] = mid;
        vdp.registers[REG_DMA_SRC_LO] = lo;

        let expected = if (hi & 0x40) != 0 {
            0xFF0000 | ((mid as u32) << 9) | ((lo as u32) << 1)
        } else {
            (((hi & 0x3F) as u32) << 17) |
            ((mid as u32) << 9) |
            ((lo as u32) << 1)
        };

        prop_assert_eq!(vdp.dma_source_transfer(), expected);
    }

    #[test]
    fn test_dma_length_prop(hi in 0u8..=255, lo in 0u8..=255) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_LEN_HI] = hi;
        vdp.registers[REG_DMA_LEN_LO] = lo;
        let expected = ((hi as u32) << 8) | (lo as u32);
        prop_assert_eq!(vdp.dma_length(), expected);
    }

    #[test]
    fn test_is_dma_transfer_prop(reg_hi in 0u8..=255u8) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_SRC_HI] = reg_hi;

        // Transfer if bit 7 is 0
        let is_transfer = (reg_hi & 0x80) == 0;
        prop_assert_eq!(vdp.is_dma_transfer(), is_transfer);
    }

    #[test]
    fn test_is_dma_fill_prop(reg_hi in 0u8..=255u8) {
        let mut vdp = Vdp::new();
        vdp.registers[REG_DMA_SRC_HI] = reg_hi;

        // Fill if bits 7,6 are 10 (0x80)
        let is_fill = (reg_hi & 0xC0) == 0x80;
        prop_assert_eq!(vdp.is_dma_fill(), is_fill);
    }
}
