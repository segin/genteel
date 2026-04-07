use crate::vdp::{
    Vdp, MODE1_HINT_ENABLE, MODE2_DISPLAY_ENABLE, MODE2_VINT_ENABLE, REG_AUTO_INC, REG_MODE1,
    REG_MODE2,
};

#[test]
fn test_vdp_auto_increment_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert_eq!(vdp.auto_increment(), 0);

    // Test set value
    vdp.registers[REG_AUTO_INC] = 0x42;
    assert_eq!(vdp.auto_increment(), 0x42);

    vdp.registers[REG_AUTO_INC] = 0x02;
    assert_eq!(vdp.auto_increment(), 0x02);
}

#[test]
fn test_vdp_is_control_pending_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert!(!vdp.is_control_pending());

    // Test pending true
    vdp.command.pending = true;
    assert!(vdp.is_control_pending());

    // Test pending false
    vdp.command.pending = false;
    assert!(!vdp.is_control_pending());
}

#[test]
fn test_vdp_display_enabled_getter() {
    let mut vdp = Vdp::new();

    // Test default (usually disabled after reset)
    assert!(!vdp.display_enabled());

    // Enable display
    vdp.registers[REG_MODE2] |= MODE2_DISPLAY_ENABLE;
    assert!(vdp.display_enabled());

    // Disable display
    vdp.registers[REG_MODE2] &= !MODE2_DISPLAY_ENABLE;
    assert!(!vdp.display_enabled());
}

#[test]
fn test_vdp_vint_enabled_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert!(!vdp.vint_enabled());

    // Enable VInt
    vdp.registers[REG_MODE2] |= MODE2_VINT_ENABLE;
    assert!(vdp.vint_enabled());

    // Disable VInt
    vdp.registers[REG_MODE2] &= !MODE2_VINT_ENABLE;
    assert!(!vdp.vint_enabled());
}

#[test]
fn test_vdp_mode1_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert_eq!(vdp.mode1(), 0x00);

    // Test set value
    vdp.registers[REG_MODE1] = 0x42;
    assert_eq!(vdp.mode1(), 0x42);

    vdp.registers[REG_MODE1] = 0xFF;
    assert_eq!(vdp.mode1(), 0xFF);
}

#[test]
fn test_vdp_hint_enabled_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert!(!vdp.hint_enabled());

    // Enable HInt
    vdp.registers[REG_MODE1] |= MODE1_HINT_ENABLE;
    assert!(vdp.hint_enabled());

    // Disable HInt
    vdp.registers[REG_MODE1] &= !MODE1_HINT_ENABLE;
    assert!(!vdp.hint_enabled());
}

#[test]
fn test_vdp_mode2_getter() {
    let mut vdp = Vdp::new();

    // Test default
    assert_eq!(vdp.mode2(), 0);

    // Set arbitrary values
    vdp.registers[REG_MODE2] = 0x00;
    assert_eq!(vdp.mode2(), 0x00);

    vdp.registers[REG_MODE2] = 0xFF;
    assert_eq!(vdp.mode2(), 0xFF);

    // Test with specific known flags for this register
    let test_val = MODE2_DISPLAY_ENABLE | MODE2_VINT_ENABLE;
    vdp.registers[REG_MODE2] = test_val;
    assert_eq!(vdp.mode2(), test_val);
}
