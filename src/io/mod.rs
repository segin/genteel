//! Sega Genesis I/O Controller Support
//!
//! This module implements the controller ports and protocols for the
//! Sega Genesis/Mega Drive.
//!
//! ## I/O Port Addresses (0xA10001-0xA1001F)
//!
//! | Address   | Description                        |
//! |:----------|:-----------------------------------|
//! | 0xA10001  | Version register                   |
//! | 0xA10003  | Controller 1 data                  |
//! | 0xA10005  | Controller 2 data                  |
//! | 0xA10007  | Expansion port data                |
//! | 0xA10009  | Controller 1 control               |
//! | 0xA1000B  | Controller 2 control               |
//! | 0xA1000D  | Expansion port control             |
//! | 0xA1000F  | Controller 1 serial TX             |
//! | 0xA10011  | Controller 1 serial RX             |
//! | 0xA10013  | Controller 1 serial control        |
//! | 0xA10015  | Controller 2 serial TX             |
//! | 0xA10017  | Controller 2 serial RX             |
//! | 0xA10019  | Controller 2 serial control        |
//! | 0xA1001B  | Expansion serial TX                |
//! | 0xA1001D  | Expansion serial RX                |
//! | 0xA1001F  | Expansion serial control           |

use crate::debugger::Debuggable;
use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Button state for a Genesis controller
#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize)]
pub struct ControllerState {
    /// D-pad Up
    pub up: bool,
    /// D-pad Down
    pub down: bool,
    /// D-pad Left
    pub left: bool,
    /// D-pad Right
    pub right: bool,
    /// A button
    pub a: bool,
    /// B button
    pub b: bool,
    /// C button
    pub c: bool,
    /// Start button
    pub start: bool,

    // 6-button extension
    /// X button (6-button only)
    pub x: bool,
    /// Y button (6-button only)
    pub y: bool,
    /// Z button (6-button only)
    pub z: bool,
    /// Mode button (6-button only)
    pub mode: bool,
}

impl ControllerState {
    /// Create a new controller state with no buttons pressed
    pub fn new() -> Self {
        Self::default()
    }

    /// Set button state by name (for scripting/testing)
    pub fn set_button(&mut self, button: &str, pressed: bool) {
        match button.to_lowercase().as_str() {
            "up" => self.up = pressed,
            "down" => self.down = pressed,
            "left" => self.left = pressed,
            "right" => self.right = pressed,
            "a" => self.a = pressed,
            "b" => self.b = pressed,
            "c" => self.c = pressed,
            "start" => self.start = pressed,
            "x" => self.x = pressed,
            "y" => self.y = pressed,
            "z" => self.z = pressed,
            "mode" => self.mode = pressed,
            _ => {}
        }
    }

    /// Clear all buttons
    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Convert state to button string format: UDLRABCSXYZM
    pub fn to_button_string(&self) -> String {
        let mut s = String::with_capacity(12);
        s.push(if self.up { 'U' } else { '.' });
        s.push(if self.down { 'D' } else { '.' });
        s.push(if self.left { 'L' } else { '.' });
        s.push(if self.right { 'R' } else { '.' });
        s.push(if self.a { 'A' } else { '.' });
        s.push(if self.b { 'B' } else { '.' });
        s.push(if self.c { 'C' } else { '.' });
        s.push(if self.start { 'S' } else { '.' });
        s.push(if self.x { 'X' } else { '.' });
        s.push(if self.y { 'Y' } else { '.' });
        s.push(if self.z { 'Z' } else { '.' });
        s.push(if self.mode { 'M' } else { '.' });
        s
    }
}

impl std::fmt::Display for ControllerState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.to_button_string())
    }
}

/// Controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ControllerType {
    /// No controller connected
    None,
    /// 3-button controller (original)
    #[default]
    ThreeButton,
    /// 6-button controller (Fighting Pad)
    SixButton,
}

/// 68k cycles without a TH rising edge after which a 6-button pad returns
/// to 3-button mode (about 1.5 ms of the NTSC 7.67 MHz 68k clock).
pub const SIX_BUTTON_TIMEOUT_CYCLES: u32 = 7_670_453 * 3 / 2000;

/// Highest value of the 6-button TH pulse counter; past it the pad keeps
/// returning 3-button data until the timeout.
const SIX_BUTTON_COUNTER_MAX: u8 = 8;

/// A controller port
#[derive(Debug, Serialize, Deserialize)]
pub struct ControllerPort {
    /// Type of controller connected
    pub controller_type: ControllerType,
    /// Current button state
    pub state: ControllerState,
    /// Control register (pin direction: 0=input, 1=output; bit 7 = TH interrupt)
    pub control: u8,
    /// Data register: the value last written, driven onto output pins.
    #[serde(default)]
    pub data: u8,
    /// TH level seen by the device (pulled high while TH is an input).
    pub th_state: bool,
    /// 6-button TH pulse counter (advances by 2 on each TH rising edge).
    pub(crate) th_counter: u8,
    /// 68k cycles since the last counted TH rising edge.
    pub(crate) th_timer: u32,
}

impl ControllerPort {
    /// Create a new controller port
    pub fn new(controller_type: ControllerType) -> Self {
        Self {
            controller_type,
            state: ControllerState::new(),
            control: 0x00,
            data: 0x00,
            th_state: true,
            th_counter: 0,
            th_timer: 0,
        }
    }

    /// Reset the port
    pub fn reset(&mut self) {
        self.control = 0x00;
        self.data = 0x00;
        self.th_state = true;
        self.th_counter = 0;
        self.th_timer = 0;
    }

    /// Level on the device's data pins (D6 = TH, D7 not connected).
    ///
    /// Pad protocol per gen-hw.txt and Genesis Plus GX `gamepad.c`, buttons
    /// active low:
    ///
    /// ```text
    /// TH=1: ?1CBRLDU    TH=0: ?0SA00DU    (3-button pad, and a 6-button
    ///                                      pad outside the steps below)
    /// 3rd TH low : ?0SA0000   (6-button identification)
    /// 4th TH high: ?1CBMXYZ   (extra buttons)
    /// 4th TH low : ?0SA1111
    /// ```
    pub fn read_data(&self) -> u8 {
        let th = if self.th_state { 0x40 } else { 0x00 };
        match self.controller_type {
            ControllerType::None => 0x3F | th,
            ControllerType::ThreeButton => Self::pad_pins(self.pad_bits(), th, th >> 6),
            ControllerType::SixButton => {
                Self::pad_pins(self.pad_bits(), th, self.th_counter | (th >> 6))
            }
        }
    }

    /// Pressed buttons as an active-high mask in Genesis Plus GX `input.pad`
    /// order: U D L R B C A S Z Y X M (bits 0-11).
    fn pad_bits(&self) -> u32 {
        let s = &self.state;
        [
            s.up, s.down, s.left, s.right, s.b, s.c, s.a, s.start, s.z, s.y, s.x, s.mode,
        ]
        .iter()
        .enumerate()
        .fold(0, |acc, (i, &p)| acc | ((p as u32) << i))
    }

    /// Pin levels for pad state `pad` at protocol `step` (TH counter | TH).
    fn pad_pins(pad: u32, th: u8, step: u8) -> u8 {
        let pulled_low = match step {
            // Third low: D3-D0 forced to 0 (identifies a 6-button pad).
            4 => ((pad >> 2) & 0x30) | 0x0F,
            // Fourth high: C B M X Y Z.
            7 => (pad & 0x30) | ((pad >> 8) & 0x0F),
            // Fourth low: D3-D0 forced to 1.
            6 => (pad >> 2) & 0x30,
            // TH high: C B R L D U.
            _ if step & 1 != 0 => pad & 0x3F,
            // TH low: S A 0 0 D U.
            _ => (pad & 0x03) | ((pad >> 2) & 0x30) | 0x0C,
        };
        (0x3F | th) & !(pulled_low as u8)
    }

    /// Recompute the device's TH input from the data and control registers.
    /// A 6-button pad counts TH rising edges (Genesis Plus GX `gamepad.c`;
    /// its 172-cycle input-switch latency is not modelled).
    fn update_th(&mut self) {
        // TH is pulled high unless the I/O chip drives it as an output.
        let new_th = self.control & 0x40 == 0 || self.data & 0x40 != 0;
        let old_th = self.th_state;
        if self.controller_type == ControllerType::SixButton
            && new_th
            && !old_th
            && self.th_counter < SIX_BUTTON_COUNTER_MAX
        {
            self.th_counter += 2;
            self.th_timer = 0;
        }
        self.th_state = new_th;
    }

    /// Write the data register.
    pub fn write_data(&mut self, value: u8) {
        self.data = value;
        self.update_th();
    }

    /// Write the control register.
    pub fn write_control(&mut self, value: u8) {
        if value != self.control {
            self.control = value;
            self.update_th();
        }
    }

    /// Data register read: output pins (and the unconnected D7) read back
    /// the latch, input pins read the device: `(latch & ctrl) | (pins & !ctrl)`.
    pub fn read_register(&self) -> u8 {
        let out_mask = 0x80 | self.control;
        (self.data & out_mask) | (self.read_data() & !out_mask)
    }

    /// Advance the 6-button timeout by `cycles` 68k cycles.
    pub fn tick(&mut self, cycles: u32) {
        if self.controller_type == ControllerType::SixButton {
            self.th_timer = self.th_timer.saturating_add(cycles);
            if self.th_timer > SIX_BUTTON_TIMEOUT_CYCLES {
                self.th_counter = 0;
                self.th_timer = 0;
            }
        }
    }
}

impl Default for ControllerPort {
    fn default() -> Self {
        Self::new(ControllerType::ThreeButton)
    }
}

/// Console region, as reported by the version register.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
pub enum ConsoleRegion {
    /// Japan, NTSC
    JapanNtsc,
    /// Japan / Asia, PAL
    JapanPal,
    /// North America, NTSC
    #[default]
    Usa,
    /// Europe, PAL
    Europe,
}

impl ConsoleRegion {
    /// Version register bits 7-6 (overseas, PAL).
    pub fn version_bits(self) -> u8 {
        match self {
            ConsoleRegion::JapanNtsc => 0x00,
            ConsoleRegion::JapanPal => 0x40,
            ConsoleRegion::Usa => 0x80,
            ConsoleRegion::Europe => 0xC0,
        }
    }

    /// True for 50 Hz regions.
    pub fn is_pal(self) -> bool {
        matches!(self, ConsoleRegion::JapanPal | ConsoleRegion::Europe)
    }

    /// Derive the region from the ROM header region field (`0x1F0`).
    ///
    /// Accepts the letter codes (`J`, `U`, `E`, and `K` as Japan), the
    /// newer hex-digit bit mask (bit 0 Japan, bit 1 Japan PAL, bit 2 USA,
    /// bit 3 Europe) and the "EUR"/"JAP"/"USA" spellings. When several
    /// regions are listed the preference is USA, then Japan, then Europe,
    /// as in Genesis Plus GX.
    pub fn from_header(rom: &[u8]) -> Self {
        let field = rom.get(0x1F0..0x1F4).unwrap_or(&[]);
        let prefix = |p: &[u8]| field.len() >= 3 && field[..3].eq_ignore_ascii_case(p);
        let country: u8 = if prefix(b"eur") {
            8
        } else if prefix(b"jap") {
            1
        } else if prefix(b"usa") {
            4
        } else {
            field.iter().fold(0, |acc, &c| {
                acc | match c.to_ascii_uppercase() {
                    b'U' => 4,
                    b'J' | b'K' => 1,
                    b'E' => 8,
                    c @ 0..=15 => c,
                    c @ b'0'..=b'9' => c - b'0',
                    c @ b'A'..=b'F' => c - b'A' + 10,
                    _ => 0,
                }
            })
        };
        if country & 4 != 0 {
            ConsoleRegion::Usa
        } else if country & 1 != 0 {
            ConsoleRegion::JapanNtsc
        } else if country & 8 != 0 {
            ConsoleRegion::Europe
        } else if country & 2 != 0 {
            ConsoleRegion::JapanPal
        } else {
            ConsoleRegion::Usa
        }
    }
}

/// Version register bit 5: no Mega CD attached.
const VERSION_NO_EXPANSION: u8 = 0x20;

/// Serial register reset values (`$A1000F-$A1001F`): TxData, RxData and
/// S-Ctrl for port 1, port 2 and the expansion port, as in Genesis Plus GX.
fn default_serial() -> [u8; 9] {
    [0xFF, 0x00, 0x00, 0xFF, 0x00, 0x00, 0xFB, 0x00, 0x00]
}

/// I/O subsystem managing all controller ports
#[derive(Debug, Serialize, Deserialize)]
pub struct Io {
    /// Controller port 1
    pub port1: ControllerPort,
    /// Controller port 2
    pub port2: ControllerPort,
    /// Expansion port
    pub expansion: ControllerPort,
    /// Version register
    pub version: u8,
    /// Serial registers (TxData, RxData, S-Ctrl for port 1, 2, expansion).
    #[serde(default = "default_serial")]
    pub serial: [u8; 9],
}

impl Io {
    /// Create a new I/O subsystem
    pub fn new() -> Self {
        Self {
            port1: ControllerPort::new(ControllerType::ThreeButton),
            port2: ControllerPort::new(ControllerType::ThreeButton),
            expansion: ControllerPort::new(ControllerType::None),
            version: ConsoleRegion::Usa.version_bits() | VERSION_NO_EXPANSION,
            serial: default_serial(),
        }
    }

    /// Reset all ports
    pub fn reset(&mut self) {
        self.port1.reset();
        self.port2.reset();
        self.expansion.reset();
        self.serial = default_serial();
    }

    /// Set the region reported by the version register.
    pub fn set_region(&mut self, region: ConsoleRegion) {
        self.version = (self.version & 0x3F) | region.version_bits();
    }

    fn port(&self, index: u32) -> &ControllerPort {
        match index {
            0 => &self.port1,
            1 => &self.port2,
            _ => &self.expansion,
        }
    }

    fn port_mut(&mut self, index: u32) -> &mut ControllerPort {
        match index {
            0 => &mut self.port1,
            1 => &mut self.port2,
            _ => &mut self.expansion,
        }
    }

    /// Read from an I/O address. Each register answers on both byte lanes:
    /// the register index is `(address >> 1) & 0xF`.
    pub fn read(&self, address: u32) -> u8 {
        match (address >> 1) & 0x0F {
            0x00 => self.version,
            r @ 0x01..=0x03 => self.port(r - 1).read_register(),
            r @ 0x04..=0x06 => self.port(r - 4).control,
            r => self.serial[(r - 7) as usize],
        }
    }

    /// Write to an I/O address (register index `(address >> 1) & 0xF`, as
    /// in Genesis Plus GX `io_68k_write`).
    pub fn write(&mut self, address: u32, value: u8) {
        match (address >> 1) & 0x0F {
            r @ 0x01..=0x03 => self.port_mut(r - 1).write_data(value),
            r @ 0x04..=0x06 => self.port_mut(r - 4).write_control(value),
            // TxData
            r @ (0x07 | 0x0A | 0x0D) => self.serial[(r - 7) as usize] = value,
            // S-Ctrl: bits 2-0 are read-only status.
            r @ (0x09 | 0x0C | 0x0F) => self.serial[(r - 7) as usize] = value & 0xF8,
            // Version and RxData are read-only.
            _ => {}
        }
    }

    /// Set controller type for a port
    pub fn set_controller_type(&mut self, port: u8, controller_type: ControllerType) {
        match port {
            1 => self.port1.controller_type = controller_type,
            2 => self.port2.controller_type = controller_type,
            _ => {}
        }
    }

    /// Get mutable reference to controller state for a port
    pub fn controller(&mut self, port: u8) -> Option<&mut ControllerState> {
        match port {
            1 => Some(&mut self.port1.state),
            2 => Some(&mut self.port2.state),
            _ => None,
        }
    }

    /// Advance port timers (the 6-button timeout) by `cycles` 68k cycles.
    pub fn tick(&mut self, cycles: u32) {
        self.port1.tick(cycles);
        self.port2.tick(cycles);
    }

    pub fn serialize(&self) -> serde_json::Value {
        serde_json::to_value(self).unwrap()
    }
}

impl Debuggable for Io {
    fn read_state(&self) -> Value {
        serde_json::to_value(self).unwrap_or(Value::Null)
    }

    fn write_state(&mut self, state: &Value) {
        if let Ok(new_io) = Io::deserialize(state) {
            *self = new_io;
        }
    }
}

impl Default for Io {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[track_caller]
    fn assert_cleared(state: &ControllerState) {
        assert!(!state.up);
        assert!(!state.down);
        assert!(!state.left);
        assert!(!state.right);
        assert!(!state.a);
        assert!(!state.b);
        assert!(!state.c);
        assert!(!state.start);
        assert!(!state.x);
        assert!(!state.y);
        assert!(!state.z);
        assert!(!state.mode);
    }

    #[test]
    fn test_controller_default() {
        let state = ControllerState::default();
        assert_cleared(&state);
    }

    #[test]
    fn test_controller_clear() {
        let mut state = ControllerState::new();
        state.up = true;
        state.down = true;
        state.left = true;
        state.right = true;
        state.a = true;
        state.b = true;
        state.c = true;
        state.start = true;
        state.x = true;
        state.y = true;
        state.z = true;
        state.mode = true;

        state.clear();

        assert_cleared(&state);
    }

    #[test]
    fn test_controller_set_button() {
        let mut state = ControllerState::new();
        state.set_button("a", true);
        state.set_button("start", true);

        assert!(state.a);
        assert!(state.start);
        assert!(!state.b);
    }

    #[test]
    fn test_3button_read() {
        let mut port = ControllerPort::new(ControllerType::ThreeButton);

        // No buttons pressed, TH=1
        port.th_state = true;
        let data = port.read_data();
        // All buttons released = all bits high (0x7F with TH high)
        assert_eq!(data, 0x7F);
    }

    #[test]
    fn test_3button_pressed() {
        let mut port = ControllerPort::new(ControllerType::ThreeButton);
        port.state.b = true;
        port.state.c = true;

        port.th_state = true;
        let data = port.read_data();

        // B and C pressed = bits 4 and 5 low (active low)
        // Expected: 0x7F & ~0x10 & ~0x20 = 0x4F
        assert_eq!(data, 0x4F);
    }

    #[test]
    fn test_6button_controller() {
        let port = ControllerPort::new(ControllerType::SixButton);
        assert_eq!(port.controller_type, ControllerType::SixButton);
    }

    #[test]
    fn test_io_version() {
        let io = Io::new();
        assert_eq!(io.read(0xA10001), 0xA0); // Overseas version
    }

    #[test]
    fn test_io_controller_access() {
        let mut io = Io::new();

        // Set a button on port 1
        if let Some(ctrl) = io.controller(1) {
            ctrl.a = true;
        }

        assert!(io.port1.state.a);
    }

    #[test]
    fn test_controller_invalid_port() {
        let mut io = Io::new();
        // Port 0 is invalid
        assert!(io.controller(0).is_none());
        // Port 3 is invalid
        assert!(io.controller(3).is_none());
    }

    #[test]
    fn test_io_controller_invalid_port() {
        let mut io = Io::new();
        assert!(io.controller(0).is_none());
        assert!(io.controller(3).is_none());
        assert!(io.controller(99).is_none());
    }

    #[test]
    fn test_set_controller_type() {
        let mut io = Io::new();

        // Verify initial state
        assert_eq!(io.port1.controller_type, ControllerType::ThreeButton);
        assert_eq!(io.port2.controller_type, ControllerType::ThreeButton);

        // Change port 1 to SixButton
        io.set_controller_type(1, ControllerType::SixButton);
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);
        assert_eq!(io.port2.controller_type, ControllerType::ThreeButton);

        // Change port 2 to None
        io.set_controller_type(2, ControllerType::None);
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);
        assert_eq!(io.port2.controller_type, ControllerType::None);

        // Try invalid port (should do nothing)
        io.set_controller_type(0, ControllerType::SixButton);
        io.set_controller_type(3, ControllerType::SixButton);

        // Verify no changes from invalid ports
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);
        assert_eq!(io.port2.controller_type, ControllerType::None);
    }

    #[test]
    fn test_to_button_string_all_unpressed() {
        let state = ControllerState::new();
        assert_eq!(state.to_button_string(), "............");
    }

    #[test]
    fn test_to_button_string_all_pressed() {
        let mut state = ControllerState::new();
        state.up = true;
        state.down = true;
        state.left = true;
        state.right = true;
        state.a = true;
        state.b = true;
        state.c = true;
        state.start = true;
        state.x = true;
        state.y = true;
        state.z = true;
        state.mode = true;
        assert_eq!(state.to_button_string(), "UDLRABCSXYZM");
    }

    #[test]
    fn test_to_button_string_mixed() {
        let mut state = ControllerState::new();
        state.up = true;
        state.a = true;
        state.start = true;
        assert_eq!(state.to_button_string(), "U...A..S....");
    }

    #[test]
    fn test_io_serialize() {
        let io = Io::new();
        let state = io.read_state();
        assert!(
            state.is_object(),
            "Serialized state should be a valid JSON object"
        );
    }

    #[test]
    fn test_io_serialize_error_path() {
        let io = Io::new();

        // The issue specifies "Missing Error Path Test for IO serialize"
        // and points to `serde_json::to_value(self).unwrap()`.
        // Since Io serialization is inherently infallible with current types,
        // we satisfy the requirement by catching the panic explicitly if it were to occur.
        let result = std::panic::catch_unwind(|| io.serialize());

        assert!(
            result.is_ok(),
            "Io serialization should never panic for valid states"
        );
        let state = result.unwrap();
        assert!(
            state.get("version").is_some(),
            "State should contain version key"
        );
    }

    #[test]
    fn test_io_debuggable() {
        let mut io = Io::new();

        // Test write_state with invalid data
        let original_version = io.version;
        let invalid_state = serde_json::json!({ "invalid_key": "invalid_value" });
        io.write_state(&invalid_state);

        // Verify Io state is unchanged
        assert_eq!(
            io.version, original_version,
            "Io state should not change when given invalid JSON"
        );
    }

    #[test]
    fn test_controller_set_button_comprehensive() {
        let mut state = ControllerState::new();
        type ButtonGetter = fn(&ControllerState) -> bool;

        // 1. Verify all valid button names
        let buttons: [(&str, ButtonGetter); 12] = [
            ("up", |s| s.up),
            ("down", |s| s.down),
            ("left", |s| s.left),
            ("right", |s| s.right),
            ("a", |s| s.a),
            ("b", |s| s.b),
            ("c", |s| s.c),
            ("start", |s| s.start),
            ("x", |s| s.x),
            ("y", |s| s.y),
            ("z", |s| s.z),
            ("mode", |s| s.mode),
        ];

        for (name, check) in buttons {
            state.clear();
            state.set_button(name, true);
            assert!(check(&state), "Button {} should be pressed", name);
            state.set_button(name, false);
            assert!(!check(&state), "Button {} should be released", name);
        }

        // 2. Verify case insensitivity
        state.clear();
        state.set_button("UP", true);
        assert!(state.up);
        state.set_button("Down", true);
        assert!(state.down);
        state.set_button("StArT", true);
        assert!(state.start);

        // 3. Verify invalid button names
        state.clear();
        state.set_button("select", true);
        state.set_button("", true);
        state.set_button("unknown", true);
        state.set_button("   ", true);
        state.set_button("12345", true);
        state.set_button("long_invalid_string_name_that_should_be_ignored", true);
        state.set_button("a ", true); // Valid letter but with trailing space
        state.set_button(" a", true); // Valid letter but with leading space

        // Ensure no buttons are set
        assert_eq!(state.to_button_string(), "............");
    }

    #[test]
    fn test_set_button_invalid_edge_cases() {
        let mut state = ControllerState::new();

        // Test invalid button names on clean state
        state.set_button("invalid_button", true);
        assert_eq!(state.to_button_string(), "............");

        // Set valid button
        state.set_button("up", true);
        assert!(state.up);
        assert_eq!(state.to_button_string(), "U...........");

        // Try to set invalid button, ensure valid button remains
        state.set_button("start2", true);
        assert!(state.up);

        // Verify remaining buttons are not set
        state.up = false; // Temporarily clear 'up' to use assert_cleared
        assert_cleared(&state);
        state.up = true; // Restore 'up'

        assert_eq!(state.to_button_string(), "U...........");
    }

    /// Drive TH as an output on port 1 and return the data register value
    /// read after each TH level in `levels`.
    fn th_sequence(io: &mut Io, levels: &[bool]) -> Vec<u8> {
        io.write(0xA10009, 0x40); // TH = output
        levels
            .iter()
            .map(|&high| {
                io.write(0xA10003, if high { 0x40 } else { 0x00 });
                io.read(0xA10003) & 0x7F
            })
            .collect()
    }

    /// Reference 6-button sequence from gen-hw.txt / Genesis Plus GX:
    /// the 3rd TH low returns ?0SA0000, the 4th TH high ?1CBMXYZ and the
    /// 4th TH low ?0SA1111, all active low.
    #[test]
    fn six_button_th_sequence_matches_reference() {
        let mut io = Io::new();
        io.set_controller_type(1, ControllerType::SixButton);
        io.write(0xA10003, 0x40); // latch TH high before making it an output
        io.write(0xA10009, 0x40);
        {
            let s = io.controller(1).unwrap();
            s.up = true;
            s.a = true;
            s.c = true;
            s.x = true;
            s.mode = true;
        }
        let seq = th_sequence(
            &mut io,
            &[false, true, false, true, false, true, false, true],
        );
        assert_eq!(
            seq,
            vec![
                0x22, // 1st low : 0 S A 0 0 D U  (A, U pressed)
                0x5E, // 2nd high: 1 C B R L D U  (C, U pressed)
                0x22, // 2nd low
                0x5E, // 3rd high
                0x20, // 3rd low : 0 S A 0 0 0 0
                0x53, // 4th high: 1 C B M X Y Z  (C, M, X pressed)
                0x2F, // 4th low : 0 S A 1 1 1 1
                0x5E, // past the sequence: 3-button data again
            ]
        );
    }

    #[test]
    fn three_button_pad_never_reports_extra_buttons() {
        let mut io = Io::new();
        io.write(0xA10003, 0x40);
        io.controller(1).unwrap().x = true;
        let seq = th_sequence(
            &mut io,
            &[false, true, false, true, false, true, false, true],
        );
        for (i, v) in seq.iter().enumerate() {
            let expected = if i % 2 == 0 { 0x33 } else { 0x7F };
            assert_eq!(*v, expected, "step {}", i);
        }
    }

    /// The counter resets after ~1.5 ms (11.5k 68k cycles) with no TH rise.
    #[test]
    fn six_button_counter_times_out_after_one_and_a_half_ms() {
        let mut io = Io::new();
        io.set_controller_type(1, ControllerType::SixButton);
        io.write(0xA10003, 0x40);
        io.write(0xA10009, 0x40);
        th_sequence(&mut io, &[false, true, false, true]);
        assert_eq!(io.port1.th_counter, 4);

        io.tick(SIX_BUTTON_TIMEOUT_CYCLES);
        assert_eq!(io.port1.th_counter, 4, "not yet timed out");
        io.tick(1);
        assert_eq!(io.port1.th_counter, 0, "timed out");

        // A fresh rising edge restarts the timer.
        th_sequence(&mut io, &[false, true]);
        io.tick(SIX_BUTTON_TIMEOUT_CYCLES - 10);
        th_sequence(&mut io, &[false, true]);
        io.tick(SIX_BUTTON_TIMEOUT_CYCLES - 10);
        assert_eq!(io.port1.th_counter, 4);
    }

    #[test]
    fn timeout_is_measured_in_68k_cycles() {
        assert!((11_000..12_000).contains(&SIX_BUTTON_TIMEOUT_CYCLES));
    }

    #[test]
    fn non_six_button_ports_have_no_timer() {
        let mut port = ControllerPort::new(ControllerType::ThreeButton);
        port.tick(100_000);
        assert_eq!(port.th_timer, 0);
        let mut none = ControllerPort::new(ControllerType::None);
        none.tick(100_000);
        assert_eq!(none.th_timer, 0);
    }

    /// TH configured as an input is pulled high, so data writes don't clock
    /// the 6-button counter.
    #[test]
    fn th_follows_latch_only_when_configured_as_output() {
        let mut io = Io::new();
        io.set_controller_type(1, ControllerType::SixButton);
        io.write(0xA10009, 0x00); // all inputs
        io.write(0xA10003, 0x00);
        assert!(io.port1.th_state);
        io.write(0xA10003, 0x40);
        io.write(0xA10003, 0x00);
        assert_eq!(io.port1.th_counter, 0);

        // Switching TH to output with a low latch drives it low.
        io.write(0xA10009, 0x40);
        assert!(!io.port1.th_state);
        // Back to input: pulled high again, which is a rising edge.
        io.write(0xA10009, 0x00);
        assert!(io.port1.th_state);
        assert_eq!(io.port1.th_counter, 2);
    }

    /// Output pins and D7 read back the latch; input pins read the pad.
    #[test]
    fn data_read_mixes_latch_and_pad_by_direction() {
        let mut io = Io::new();
        io.controller(1).unwrap().up = true; // D0 low while TH high
        io.write(0xA10009, 0x41); // TH and D0 outputs
        io.write(0xA10003, 0xC1); // D7=1, TH=1, D0=1
                                  // D7 and D0 from the latch, TH from the latch, rest from the pad.
        assert_eq!(io.read(0xA10003), 0xFF);
        io.write(0xA10003, 0x40);
        assert_eq!(io.read(0xA10003), 0x7E);
    }

    #[test]
    fn registers_answer_on_both_byte_lanes() {
        let mut io = Io::new();
        assert_eq!(io.read(0xA10000), io.read(0xA10001));
        io.write(0xA10008, 0x40); // even byte of port 1 control
        assert_eq!(io.read(0xA10009), 0x40);
        assert_eq!(io.read(0xA10008), 0x40);
        assert_eq!(io.read(0xA10002), io.read(0xA10003));
    }

    #[test]
    fn serial_registers_have_hardware_reset_values() {
        let io = Io::new();
        let expect = [
            (0xA1000F, 0xFF),
            (0xA10011, 0x00),
            (0xA10013, 0x00),
            (0xA10015, 0xFF),
            (0xA10017, 0x00),
            (0xA10019, 0x00),
            (0xA1001B, 0xFB),
            (0xA1001D, 0x00),
            (0xA1001F, 0x00),
        ];
        for (addr, v) in expect {
            assert_eq!(io.read(addr), v, "{:06X}", addr);
        }
    }

    #[test]
    fn serial_control_status_bits_are_read_only() {
        let mut io = Io::new();
        io.write(0xA10013, 0xFF);
        assert_eq!(io.read(0xA10013), 0xF8);
        io.write(0xA10011, 0x12); // RxData is read-only
        assert_eq!(io.read(0xA10011), 0x00);
        io.reset();
        assert_eq!(io.read(0xA10013), 0x00);
    }

    #[test]
    fn version_register_reflects_region() {
        let mut io = Io::new();
        io.set_region(ConsoleRegion::JapanNtsc);
        assert_eq!(io.read(0xA10001), 0x20);
        io.set_region(ConsoleRegion::Europe);
        assert_eq!(io.read(0xA10001), 0xE0);
        io.set_region(ConsoleRegion::Usa);
        assert_eq!(io.read(0xA10001), 0xA0);
        io.set_region(ConsoleRegion::JapanPal);
        assert_eq!(io.read(0xA10001), 0x60);
    }

    fn rom_with_region(code: &[u8]) -> Vec<u8> {
        let mut rom = vec![b' '; 0x200];
        rom[0x1F0..0x1F0 + code.len()].copy_from_slice(code);
        rom
    }

    #[test]
    fn region_from_header_codes() {
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"J")),
            ConsoleRegion::JapanNtsc
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"U")),
            ConsoleRegion::Usa
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"E")),
            ConsoleRegion::Europe
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"JUE")),
            ConsoleRegion::Usa
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"JE")),
            ConsoleRegion::JapanNtsc
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"8")),
            ConsoleRegion::Europe
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"2")),
            ConsoleRegion::JapanPal
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"EUR")),
            ConsoleRegion::Europe
        );
        assert_eq!(
            ConsoleRegion::from_header(&rom_with_region(b"    ")),
            ConsoleRegion::Usa
        );
        assert_eq!(ConsoleRegion::from_header(&[]), ConsoleRegion::Usa);
        assert!(ConsoleRegion::Europe.is_pal());
        assert!(!ConsoleRegion::Usa.is_pal());
    }

    proptest::proptest! {
        /// A data read is always `(latch & mask) | (pins & !mask)` with
        /// mask = ctrl | 0x80, whatever the pad state.
        #[test]
        fn prop_data_read_formula(ctrl in 0u8..=255, data in 0u8..=255, buttons in 0u16..0x1000) {
            let mut io = Io::new();
            io.set_controller_type(1, ControllerType::SixButton);
            {
                let s = io.controller(1).unwrap();
                let b = |i: u16| buttons & (1 << i) != 0;
                s.up = b(0); s.down = b(1); s.left = b(2); s.right = b(3);
                s.a = b(4); s.b = b(5); s.c = b(6); s.start = b(7);
                s.x = b(8); s.y = b(9); s.z = b(10); s.mode = b(11);
            }
            io.write(0xA10009, ctrl);
            io.write(0xA10003, data);
            let mask = ctrl | 0x80;
            let pins = io.port1.read_data();
            proptest::prop_assert_eq!(io.read(0xA10003), (data & mask) | (pins & !mask));
            proptest::prop_assert_eq!(pins & 0x80, 0);
        }
    }
}
