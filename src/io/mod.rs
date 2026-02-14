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

/// Button state for a Genesis controller
#[derive(Debug, Clone, Copy, Default)]
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
}

/// Controller type
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ControllerType {
    /// No controller connected
    None,
    /// 3-button controller (original)
    ThreeButton,
    /// 6-button controller (Fighting Pad)
    SixButton,
}

impl Default for ControllerType {
    fn default() -> Self {
        ControllerType::ThreeButton
    }
}

/// A controller port
#[derive(Debug)]
pub struct ControllerPort {
    /// Type of controller connected
    pub controller_type: ControllerType,
    /// Current button state
    pub state: ControllerState,
    /// Control register (direction: 0=input, 1=output)
    pub control: u8,
    /// TH (bit 6) state from control writes
    pub th_state: bool,
    /// Counter for 6-button protocol
    th_counter: u8,
    /// Timer for 6-button reset (cycles since last TH transition)
    th_timer: u32,
}

impl ControllerPort {
    /// Create a new controller port
    pub fn new(controller_type: ControllerType) -> Self {
        Self {
            controller_type,
            state: ControllerState::new(),
            control: 0x00,
            th_state: true,
            th_counter: 0,
            th_timer: 0,
        }
    }

    /// Reset the port
    pub fn reset(&mut self) {
        self.control = 0x00;
        self.th_state = true;
        self.th_counter = 0;
        self.th_timer = 0;
    }

    /// Read from the data port
    ///
    /// The returned value depends on:
    /// - TH state (directly or via control register)
    /// - 6-button counter
    pub fn read_data(&self) -> u8 {
        match self.controller_type {
            ControllerType::None => 0x7F, // No controller = all inputs high

            ControllerType::ThreeButton => self.read_3button(),

            ControllerType::SixButton => self.read_6button(),
        }
    }

    /// Read 3-button controller data
    fn read_3button(&self) -> u8 {
        // Genesis controllers are active-low: pressed = 0, released = 1
        if self.th_state {
            // TH=1: Return Up, Down, Left, Right, B, C (and TH=1)
            let mut data = 0x7F; // All released + TH high

            if self.state.up {
                data &= !0x01;
            }
            if self.state.down {
                data &= !0x02;
            }
            if self.state.left {
                data &= !0x04;
            }
            if self.state.right {
                data &= !0x08;
            }
            if self.state.b {
                data &= !0x10;
            }
            if self.state.c {
                data &= !0x20;
            }

            data
        } else {
            // TH=0: Return Up, Down, 0, 0, A, Start (TH=0)
            let mut data = 0x33; // Up, Down, A, Start released; bits 2-3 low

            if self.state.up {
                data &= !0x01;
            }
            if self.state.down {
                data &= !0x02;
            }
            // Bits 2-3 are always 0 when TH=0 (used to detect controller type)
            if self.state.a {
                data &= !0x10;
            }
            if self.state.start {
                data &= !0x20;
            }

            data
        }
    }

    /// Read 6-button controller data
    fn read_6button(&self) -> u8 {
        match self.th_counter {
            3 => self.read_cycle3(),
            5 => self.read_extra_buttons(),
            // Cycles 0, 1, 2, 4 and others use standard 3-button logic
            _ => self.read_3button(),
        }
    }

    /// Read data for cycle 3 (controller identification)
    fn read_cycle3(&self) -> u8 {
        // If TH=1, standard 3-button logic applies
        if self.th_state {
            return self.read_3button();
        }

        // Fourth cycle: TH=0 returns controller ID in low nibble
        // Note: Original implementation returns Active High (1=Pressed) for Up/Down
        // and sets bits 2-3 to 1. This behavior is preserved here.
        let mut data = 0x0C; // Bits 2 and 3 set
        if self.state.up {
            data |= 0x01;
        }
        if self.state.down {
            data |= 0x02;
        }
        data
    }

    /// Read data for cycle 5 (extra buttons X, Y, Z, Mode)
    fn read_extra_buttons(&self) -> u8 {
        // If TH=1, standard 3-button logic applies
        if self.th_state {
            return self.read_3button();
        }

        // Sixth cycle: TH=0 returns X, Y, Z, Mode
        // Note: Original implementation returns Active High (1=Pressed) for these buttons.
        // This behavior is preserved here.
        let mut data = 0x70; // High nibble bits 4-6 set
        if self.state.z {
            data |= 0x01;
        }
        if self.state.y {
            data |= 0x02;
        }
        if self.state.x {
            data |= 0x04;
        }
        if self.state.mode {
            data |= 0x08;
        }
        data
    }

    /// Write to the data port (sets TH)
    pub fn write_data(&mut self, value: u8) {
        let new_th = (value & 0x40) != 0;

        // Detect TH 1->0 transition (falling edge)
        if self.th_state && !new_th {
            // Increment counter for 6-button detection
            if self.controller_type == ControllerType::SixButton {
                self.th_counter = (self.th_counter + 1) % 8;
                self.th_timer = 0;
            }
        }

        self.th_state = new_th;
    }

    /// Update timing (call each CPU cycle or frame)
    pub fn update(&mut self, cycles: u32) {
        if self.controller_type == ControllerType::SixButton {
            self.th_timer += cycles;
            // Reset counter if TH hasn't been toggled recently
            if self.th_timer > 1500 {
                self.th_counter = 0;
            }
        }
    }
}

impl Default for ControllerPort {
    fn default() -> Self {
        Self::new(ControllerType::ThreeButton)
    }
}

/// I/O subsystem managing all controller ports
#[derive(Debug)]
pub struct Io {
    /// Controller port 1
    pub port1: ControllerPort,
    /// Controller port 2
    pub port2: ControllerPort,
    /// Expansion port
    pub expansion: ControllerPort,
    /// Version register
    pub version: u8,
}

impl Io {
    /// Create a new I/O subsystem
    pub fn new() -> Self {
        Self {
            port1: ControllerPort::new(ControllerType::ThreeButton),
            port2: ControllerPort::new(ControllerType::ThreeButton),
            expansion: ControllerPort::new(ControllerType::None),
            // Version: 0xA0 = overseas, 0x00 = Japan
            version: 0xA0,
        }
    }

    /// Reset all ports
    pub fn reset(&mut self) {
        self.port1.reset();
        self.port2.reset();
        self.expansion.reset();
    }

    /// Read from an I/O address
    pub fn read(&self, address: u32) -> u8 {
        match address & 0x1F {
            0x01 => self.version,
            0x03 => self.port1.read_data(),
            0x05 => self.port2.read_data(),
            0x07 => self.expansion.read_data(),
            0x09 => self.port1.control,
            0x0B => self.port2.control,
            0x0D => self.expansion.control,
            _ => 0xFF,
        }
    }

    /// Write to an I/O address
    pub fn write(&mut self, address: u32, value: u8) {
        match address & 0x1F {
            0x03 => self.port1.write_data(value),
            0x05 => self.port2.write_data(value),
            0x07 => self.expansion.write_data(value),
            0x09 => self.port1.control = value,
            0x0B => self.port2.control = value,
            0x0D => self.expansion.control = value,
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

    /// Update timing for all ports
    pub fn update(&mut self, cycles: u32) {
        self.port1.update(cycles);
        self.port2.update(cycles);
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

    #[test]
    fn test_controller_default() {
        let state = ControllerState::default();
        assert!(!state.a);
        assert!(!state.b);
        assert!(!state.c);
        assert!(!state.start);
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
    fn test_6button_cycles() {
        let mut port = ControllerPort::new(ControllerType::SixButton);

        // Set state: Up=Pressed, Down=Released, Z=Pressed, Y=Released
        port.state.up = true;
        port.state.down = false;
        port.state.z = true;
        port.state.y = false;
        port.state.x = false;
        port.state.mode = false;

        // Cycle 0 Start: TH=1, Cnt=0
        // Verify TH=1 read
        // read_3button: 0x7F & !0x01 = 0x7E
        assert_eq!(port.read_data(), 0x7E, "Start (TH=1)");

        // Pulse 1 (Fall -> Cnt=1)
        port.write_data(0x00);
        // read_3button(TH=0): 0x33. Up=Pressed -> 0x33 & !0x01 = 0x32.
        assert_eq!(port.read_data(), 0x32, "Pulse 1 Fall (TH=0)");

        // Pulse 1 (Rise)
        port.write_data(0x40);
        assert_eq!(port.read_data(), 0x7E, "Pulse 1 Rise (TH=1)");

        // Pulse 2 (Fall -> Cnt=2)
        port.write_data(0x00);
        assert_eq!(port.read_data(), 0x32, "Pulse 2 Fall (TH=0)");

        // Pulse 2 (Rise)
        port.write_data(0x40);
        assert_eq!(port.read_data(), 0x7E, "Pulse 2 Rise (TH=1)");

        // Pulse 3 (Fall -> Cnt=3) ** ID Check **
        port.write_data(0x00);
        // Logic: Bits 2-3 set (0x0C).
        // Up=Pressed (Active High) -> Bit 0 set.
        // Down=Released (Active High) -> Bit 1 clear.
        // Expected: 0x0C | 0x01 = 0x0D.
        assert_eq!(port.read_data(), 0x0D, "Pulse 3 Fall (ID Check)");

        // Pulse 3 (Rise)
        port.write_data(0x40);
        assert_eq!(port.read_data(), 0x7E, "Pulse 3 Rise (TH=1)");

        // Pulse 4 (Fall -> Cnt=4)
        port.write_data(0x00);
        assert_eq!(port.read_data(), 0x32, "Pulse 4 Fall (TH=0)");

        // Pulse 4 (Rise)
        port.write_data(0x40);
        assert_eq!(port.read_data(), 0x7E, "Pulse 4 Rise (TH=1)");

        // Pulse 5 (Fall -> Cnt=5) ** Extra Buttons **
        port.write_data(0x00);
        // Logic: Bits 4-6 set (0x70).
        // Z=Pressed (Active High) -> Bit 0 set.
        // Y=Released -> Bit 1 clear.
        // X, Mode Released -> Bits 2, 3 clear.
        // Expected: 0x70 | 0x01 = 0x71.
        assert_eq!(port.read_data(), 0x71, "Pulse 5 Fall (Extra Buttons)");
    }

    #[test]
    fn test_set_controller_type() {
        let mut io = Io::new();

        // Default: Both 3-button
        assert_eq!(io.port1.controller_type, ControllerType::ThreeButton);
        assert_eq!(io.port2.controller_type, ControllerType::ThreeButton);

        // Change Port 1 to 6-button
        io.set_controller_type(1, ControllerType::SixButton);
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);
        // Port 2 should remain unchanged
        assert_eq!(io.port2.controller_type, ControllerType::ThreeButton);

        // Change Port 2 to None
        io.set_controller_type(2, ControllerType::None);
        assert_eq!(io.port2.controller_type, ControllerType::None);
        // Port 1 should remain unchanged
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);

        // Invalid port should do nothing
        io.set_controller_type(3, ControllerType::ThreeButton);
        assert_eq!(io.port1.controller_type, ControllerType::SixButton);
        assert_eq!(io.port2.controller_type, ControllerType::None);

        // Change Port 1 back to 3-button
        io.set_controller_type(1, ControllerType::ThreeButton);
        assert_eq!(io.port1.controller_type, ControllerType::ThreeButton);
    }
}
