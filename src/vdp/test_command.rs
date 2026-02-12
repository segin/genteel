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

    // We want CD5-2 = 0001 (value 1).
    // New bit mapping: CD5..CD2 are in bits 5..2 of the *result*, which comes from bits 7..4 of the *word*.
    // Or wait, `(value >> 2) & 0x3C`.
    // If we want result 0x04 (bit 2 set), we need bit 4 set in value.
    // 0x0010.

    vdp.write_control(0x4000);
    vdp.write_control(0x0010); // VSRAM Write

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
    // 0x8144 = 1000 0001 0100 0100
    // Bits 7..2 affect CD5..CD0? No.
    // Logic: `let cd_upper = ((value >> 2) & 0x3C) as u8;`
    // 0x8144 >> 2 = 0x2051.
    // 0x51 & 0x3C = 0101 0001 & 0011 1100 = 0001 0000 = 0x10.
    // So cd_upper = 0x10.
    // control_code = (0x01 & 0x03) | 0x10 = 0x11.

    vdp.write_control(0x8144);

    assert!(!vdp.control_pending);
    // Register 1 should NOT be 0x44 (default is 0)
    assert_eq!(vdp.registers[1], 0x00);

    assert_eq!(vdp.control_code, 0x11);
}

#[test]
fn test_dma_pending_flag() {
    let mut vdp = Vdp::new();

    // Enable DMA in Reg 1 (Bit 4 = 0x10)
    vdp.write_control(0x8110);
    assert!(vdp.dma_enabled());

    // Write DMA command
    // First word: 0x4000 (VRAM write base).
    // Second word: Need to set bit 5 of code (0x20).
    // Logic: `(value >> 2) & 0x3C`.
    // We want result 0x20 (bit 5).
    // So `value >> 2` must have bit 5 set.
    // `value` must have bit 7 set.
    // 0x0080.

    vdp.write_control(0x4000);
    vdp.write_control(0x0080); // Sets DMA bit

    assert!(vdp.dma_pending);
}
