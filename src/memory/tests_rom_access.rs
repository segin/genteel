use crate::memory::bus::Bus;

#[test]
fn test_read_long_rom_bounds() {
    let mut bus = Bus::new();
    // Create a 1024 byte ROM to avoid padding logic
    let mut rom = vec![0; 1024];
    for i in 0..1024 {
        rom[i] = (i % 256) as u8;
    }
    bus.load_rom(&rom);

    // Normal read at 0
    // 00 01 02 03
    let val = bus.read_long(0);
    assert_eq!(val, 0x00010203);

    // Read near end: idx = 1020 (0x3FC). idx+3 = 1023. len = 1024.
    // idx+3 < len -> 1023 < 1024 -> True.
    // Should use fast path.
    // Bytes: 1020(FC), 1021(FD), 1022(FE), 1023(FF).
    let val = bus.read_long(1020);
    assert_eq!(val, 0xFCFDFEFF);

    // Read crossing end: idx = 1021 (0x3FD). idx+3 = 1024.
    // idx+3 < len -> 1024 < 1024 -> False.
    // Fallback to read_byte.
    // 1021(FD), 1022(FE), 1023(FF), 1024(Unmapped -> FF).
    let val = bus.read_long(1021);
    assert_eq!(val, 0xFDFEFFFF);
}

#[test]
fn test_read_word_rom_bounds() {
    let mut bus = Bus::new();
    let mut rom = vec![0; 1024];
    for i in 0..1024 {
        rom[i] = (i % 256) as u8;
    }
    bus.load_rom(&rom);

    // Normal read
    let val = bus.read_word(0);
    assert_eq!(val, 0x0001);

    // Read at end: idx = 1022. idx+1 = 1023.
    // idx+1 < len -> 1023 < 1024 -> True.
    // Fast path.
    // 1022(FE), 1023(FF).
    let val = bus.read_word(1022);
    assert_eq!(val, 0xFEFF);

    // Read partial: idx = 1023. idx+1 = 1024.
    // idx+1 < len -> False.
    // idx < len -> True.
    // Special partial logic in current code: rom[idx] << 8 | 0xFF.
    // 1023(FF) << 8 | FF -> FFFF.
    let val = bus.read_word(1023);
    assert_eq!(val, 0xFFFF);
}

#[test]
fn test_oversized_rom_read_long() {
    let mut bus = Bus::new();
    // 8MB ROM (larger than 4MB address space)
    // 4MB = 0x400000.
    // Make it 0x400010 bytes.
    let mut rom = vec![0; 0x400010];
    // Set some data around the boundary
    rom[0x3FFFFC] = 0xAA;
    rom[0x3FFFFD] = 0xBB;
    rom[0x3FFFFE] = 0xCC;
    rom[0x3FFFFF] = 0xDD;
    rom[0x400000] = 0xEE;
    rom[0x400001] = 0xFF;

    bus.load_rom(&rom);

    // Read exactly at boundary of 4MB space.
    // Address 0x3FFFFC. 4 bytes: 3FFFFC, 3FFFFD, 3FFFFE, 3FFFFF.
    // Should be AABBCCDD.
    let val = bus.read_long(0x3FFFFC);
    assert_eq!(val, 0xAABBCCDD);

    // Read crossing boundary?
    // Address 0x3FFFFD. Bytes: 3FFFFD, 3FFFFE, 3FFFFF, 400000.
    // Fast path condition: addr <= 0x3FFFFF.
    // idx = 0x3FFFFD. idx+3 = 0x400000.
    // len = 0x400010.
    // idx+3 < len -> True.
    // So it reads from rom vector directly.
    // Result: BBCCDDEE.
    // This confirms existing behavior reads beyond 4MB if ROM is larger.
    let val = bus.read_long(0x3FFFFD);
    assert_eq!(val, 0xBBCCDDEE);
}
