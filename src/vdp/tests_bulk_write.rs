use super::*;

#[test]
fn test_bulk_write_optimized() {
    let mut vdp = Vdp::new();

    // 1. Setup VRAM Write to address 0x1000
    // VRAM Write command: 0x4000 (CD=01) + address
    // Address 0x1000 = 0001 0000 0000 0000
    // Lower 14 bits: 0x1000.
    // Command word 1: 0x4000 | 0x1000 = 0x5000.
    vdp.write_control(0x5000);
    // Command word 2: 0x0000 (completes setup)
    vdp.write_control(0x0000);

    // Verify setup
    assert_eq!(vdp.get_control_address(), 0x1000);
    assert_eq!(vdp.get_control_code() & 0x0F, 0x01); // VRAM_WRITE

    // 2. Set Auto-increment to 2 (Reg 15)
    vdp.registers[15] = 2;

    // 3. Perform bulk write
    let data = vec![0x01, 0x02, 0x03, 0x04, 0x05, 0x06];
    vdp.write_data_bulk(&data);

    // 4. Verify VRAM content
    // Address 0x1000 -> 01
    // Address 0x1001 -> 02
    // Address 0x1002 -> 03
    // ...
    assert_eq!(vdp.vram[0x1000], 0x01);
    assert_eq!(vdp.vram[0x1001], 0x02);
    assert_eq!(vdp.vram[0x1002], 0x03);
    assert_eq!(vdp.vram[0x1003], 0x04);
    assert_eq!(vdp.vram[0x1004], 0x05);
    assert_eq!(vdp.vram[0x1005], 0x06);

    // 5. Verify address update
    // 3 writes of 2 bytes each -> +6
    assert_eq!(vdp.get_control_address(), 0x1006);

    // 6. Verify last_data_write
    assert_eq!(vdp.last_data_write, 0x0506);
}

#[test]
fn test_bulk_write_optimized_wrapping() {
    let mut vdp = Vdp::new();

    // 1. Setup VRAM Write to address 0xFFFE
    // Address 0xFFFE = 1111 1111 1111 1110
    // Lower 14 bits: 11 1111 1111 1110 = 0x3FFE
    // Upper 2 bits: 11 = 3
    // Command word 1: 0x4000 | 0x3FFE = 0x7FFE
    // Command word 2: (3 << 2)? No.
    // WriteControl uses:
    // control_address = (control_address & 0x3FFF) | ((value & 0x03) << 14);
    // So value & 0x03 must be 3.
    // So word 2 is 0x0003.

    vdp.write_control(0x7FFE);
    vdp.write_control(0x0003);

    assert_eq!(vdp.get_control_address(), 0xFFFE);

    // 2. Set Auto-increment to 2
    vdp.registers[15] = 2;

    // 3. Perform bulk write (4 bytes)
    let data = vec![0xAA, 0xBB, 0xCC, 0xDD];
    vdp.write_data_bulk(&data);

    // 4. Verify VRAM content (with wrapping)
    // 1st chunk (AA BB) at 0xFFFE
    assert_eq!(vdp.vram[0xFFFE], 0xAA);
    assert_eq!(vdp.vram[0xFFFF], 0xBB);

    // 2nd chunk (CC DD) at 0x0000 (wrapped)
    assert_eq!(vdp.vram[0x0000], 0xCC);
    assert_eq!(vdp.vram[0x0001], 0xDD);

    // 5. Verify address update
    // 0xFFFE + 2 -> 0x0000 + 2 -> 0x0002
    assert_eq!(vdp.get_control_address(), 0x0002);
}

#[test]
fn test_bulk_write_fallback() {
    let mut vdp = Vdp::new();

    // Setup VRAM Write to 0x2000
    // 0x2000 = 0010 0000 0000 0000
    // Lower 14: 0x2000.
    // Word 1: 0x4000 | 0x2000 = 0x6000.
    // Word 2: 0x0000.
    vdp.write_control(0x6000);
    vdp.write_control(0x0000);

    // Set Auto-increment to 1 (not optimized path)
    vdp.registers[15] = 1;

    let data = vec![0x11, 0x22, 0x33, 0x44];
    vdp.write_data_bulk(&data);

    // Should use fallback `write_data`
    // 1st chunk (11 22): write_data(0x1122). Addr 0x2000.
    // vram[0x2000] = 11, vram[0x2001] = 22.
    // Addr += 1 -> 0x2001.

    // 2nd chunk (33 44): write_data(0x3344). Addr 0x2001.
    // write_data(0x3344) to 0x2001.
    // vram[0x2001] = 33.
    // vram[0x2000] = 44 (addr ^ 1).

    // Result: 0x2000=44, 0x2001=33.

    assert_eq!(vdp.vram[0x2000], 0x44);
    assert_eq!(vdp.vram[0x2001], 0x33);

    // Addr += 1 -> 0x2002.
    assert_eq!(vdp.get_control_address(), 0x2002);
}
