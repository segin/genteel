use crate::vdp::{Vdp, REG_AUTO_INC, REG_MODE2, MODE2_DISPLAY_ENABLE, MODE2_VINT_ENABLE, REG_MODE1, MODE1_HINT_ENABLE};

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
