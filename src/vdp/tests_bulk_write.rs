use super::*;

#[test]
fn test_bulk_write_optimization() {
    let mut vdp = Vdp::new();

    // Setup:
    // 1. Set Auto-increment to 2 (Reg 15 = 0x02)
    // Command: 0x8F02
    vdp.write_control(0x8F02);
    // Directly verify register was set
    assert_eq!(vdp.registers[REG_AUTO_INC], 2);

    // 2. Set VRAM Write Address to 0x0000
    // Command: 0x40000000 (split into 0x4000, 0x0000)
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);
    assert_eq!(vdp.control_address, 0x0000);
    assert_eq!(vdp.control_code & 0x0F, VRAM_WRITE);

    // 3. Prepare data
    // We'll write 4 bytes: 0x11, 0x22, 0x33, 0x44
    // This simulates 2 word writes: 0x1122 and 0x3344
    let data = [0x11, 0x22, 0x33, 0x44];

    // Execute
    // This should trigger the optimized path because auto-inc is 2 and mode is VRAM Write
    vdp.write_data_bulk(&data);

    // Verify VRAM content
    // Optimized path writes chunks directly to VRAM
    assert_eq!(vdp.vram[0], 0x11);
    assert_eq!(vdp.vram[1], 0x22);
    assert_eq!(vdp.vram[2], 0x33);
    assert_eq!(vdp.vram[3], 0x44);

    // Verify Control Address update
    // Should increment by 2 for each word (total 4)
    assert_eq!(vdp.control_address, 0x0004);

    // Verify last_data_write update
    // Should be the last word written (0x3344)
    assert_eq!(vdp.last_data_write, 0x3344);

    // Verify control pending is cleared
    assert!(!vdp.control_pending);
}

#[test]
fn test_bulk_write_fallback() {
    let mut vdp = Vdp::new();

    // Setup:
    // 1. Set Auto-increment to 1 (Reg 15 = 0x01) - forces fallback path
    vdp.write_control(0x8F01);
    assert_eq!(vdp.registers[REG_AUTO_INC], 1);

    // 2. Set VRAM Write Address to 0x0000
    vdp.write_control(0x4000);
    vdp.write_control(0x0000);

    // 3. Prepare data
    // We'll write 4 bytes: 0xAA, 0xBB, 0xCC, 0xDD
    // Fallback path treats these as word writes (0xAABB, 0xCCDD) but auto-inc is 1.
    let data = [0xAA, 0xBB, 0xCC, 0xDD];

    // Execute
    vdp.write_data_bulk(&data);

    // Verify behavior with auto-inc=1
    // 1st word (0xAABB):
    //   Addr 0: vram[0]=AA, vram[1]=BB. (Addr ^ 1 logic)
    //   Addr increments by 1 -> 1.
    // 2nd word (0xCCDD):
    //   Addr 1: vram[1]=CC, vram[0]=DD. (Addr ^ 1 logic: 1^1=0)
    //   Addr increments by 1 -> 2.

    // So final state: vram[0]=DD, vram[1]=CC
    // vram[2] and vram[3] should remain 0
    assert_eq!(vdp.vram[0], 0xDD);
    assert_eq!(vdp.vram[1], 0xCC);
    assert_eq!(vdp.vram[2], 0x00);
    assert_eq!(vdp.vram[3], 0x00);

    // Verify Control Address update
    // Incremented by 1 twice -> 2
    assert_eq!(vdp.control_address, 0x0002);

    // Verify last_data_write update
    // Should be the last word written (0xCCDD)
    assert_eq!(vdp.last_data_write, 0xCCDD);
}

#[test]
fn test_bulk_write_wrapping() {
    let mut vdp = Vdp::new();

    // Setup: Auto-inc 2, VRAM Write
    vdp.write_control(0x8F02);

    // Set address to near end of VRAM: 0xFFFE
    // Command: 0x4000FFFF (wait, 0xFFFE needs correct command)
    // 0xFFFE = 1111 1111 1111 1110
    // A13-0: 11 1111 1111 1110 (0x3FFE)
    // A15-14: 11 (0x3)
    // CD=0001 (VRAM Write)
    // Word 1: 01AA AAAA AAAA AAAA -> 0x4000 | 0x3FFE = 0x7FFE.
    // Word 2: 00BB 0000 0000 0000 -> 0x0003 << 2? No.
    // Addr hi: A15-14 are in bits 1-0 of second word.
    // So 0x0003.
    vdp.write_control(0x7FFE);
    vdp.write_control(0x0003);

    assert_eq!(vdp.control_address, 0xFFFE);

    // Write 4 bytes: 0x11, 0x22, 0x33, 0x44
    let data = [0x11, 0x22, 0x33, 0x44];

    // Execute
    vdp.write_data_bulk(&data);

    // Verify wrapping
    // 1st word at 0xFFFE: vram[0xFFFE]=11, vram[0xFFFF]=22.
    // Addr -> 0x0000 (wrapping)
    // 2nd word at 0x0000: vram[0x0000]=33, vram[0x0001]=44.

    assert_eq!(vdp.vram[0xFFFE], 0x11);
    assert_eq!(vdp.vram[0xFFFF], 0x22);
    assert_eq!(vdp.vram[0x0000], 0x33);
    assert_eq!(vdp.vram[0x0001], 0x44);

    assert_eq!(vdp.control_address, 0x0002);
}
