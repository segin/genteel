use super::*;

#[test]
fn test_register_write_simple() {
    let mut vdp = Vdp::new();
    // Write 0x8144 (Reg 1 = 0x44)
    vdp.write_control(0x8144);

    assert!(!vdp.control_pending);
    assert_eq!(vdp.registers[1], 0x44);
}

#[test]
fn test_vram_write_command_sequence() {
    let mut vdp = Vdp::new();

    // Step 1: Write first word 0x4000
    // Binary: 0100 0000 0000 0000
    // Bits 15-14 are 01 -> VRAM Write (CD0=1)
    vdp.write_control(0x4000);

    assert!(vdp.control_pending);
    // Code should have lower 2 bits set to 01
    assert_eq!(vdp.control_code & 0x03, 0x01);

    // Step 2: Write second word 0x0000
    vdp.write_control(0x0000);

    assert!(!vdp.control_pending);
    // Address should be 0x0000
    assert_eq!(vdp.control_address, 0x0000);
    // Code should be 0x01 (VRAM Write)
    assert_eq!(vdp.control_code, 0x01);
}

#[test]
fn test_cram_write_command_sequence() {
    let mut vdp = Vdp::new();

    // CRAM Write is CD=0011 (0x03)
    // First word: 1100 0000 0000 0000 -> 0xC000
    vdp.write_control(0xC000);

    assert!(vdp.control_pending);
    assert_eq!(vdp.control_code & 0x03, 0x03); // CD1-0 = 11

    // Second word: 0x0000
    vdp.write_control(0x0000);

    assert!(!vdp.control_pending);
    assert_eq!(vdp.control_address, 0x0000);
    assert_eq!(vdp.control_code, 0x03);
}

#[test]
fn test_vsram_write_command_sequence() {
    let mut vdp = Vdp::new();

    // VSRAM Write is CD=0101 (0x05)
    // First word: 0100 0000 0000 0000 -> 0x4000
    // Second word: 0001 0000 0000 0000 -> 0x1000

    // We want CD5-2 = 0001 (value 1), so bit 10 of the original word needs to be set.
    // 0x0400 in the second word.

    vdp.write_control(0x4000);
    vdp.write_control(0x0400); // VSRAM Write

    assert_eq!(vdp.control_code, 0x05);
}

#[test]
fn test_register_write_during_pending() {
    let mut vdp = Vdp::new();

    // Start command
    vdp.write_control(0x4000);
    assert!(vdp.control_pending);

    // Try to write register 1 with 0x44 (0x8144)
    // Since pending is true, this should be interpreted as second word of command,
    // NOT as a register write.
    vdp.write_control(0x8144);

    assert!(!vdp.control_pending);
    // Register 1 should NOT be 0x44 (default is 0)
    assert_eq!(vdp.registers[1], 0x00);

    // It should have updated control code/address based on 0x8144 as 2nd word
    // 0x8144:
    // Address high bits: (0x8144 & 0x0003) << 14 = 0 -> Address 0.
    // Code upper bits: (0x8144 >> 8) & 0x3C = 0x81 & 0x3C = 1000 0001 & 0011 1100 = 0.
    // So code remains 0x01.
    assert_eq!(vdp.control_code, 0x01);
}

#[test]
fn test_dma_pending_flag() {
    let mut vdp = Vdp::new();

    // Enable DMA in Reg 1 (Bit 4 = 0x10)
    vdp.write_control(0x8110);
    assert!(vdp.dma_enabled());

    // Write DMA command
    // First word: 0x4000 (VRAM write base).
    // Second word: 0x2000 (Set bit 5 of code).
    // (0x2000 >> 8) & 0x3C = 0x20.
    // control_code |= 0x20.

    vdp.write_control(0x4000);
    vdp.write_control(0x2000); // 0x2000 has bit 13 set

    assert!(vdp.dma_pending);
}
