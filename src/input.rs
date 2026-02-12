//! Input Management for TAS and Debugging
//!
//! This module provides input script loading and playback for tool-assisted
//! speedruns (TAS) and automated testing.
//!
//! ## Script Format
//!
//! Simple CSV format:
//! ```text
//! # frame,p1_buttons,p2_buttons
//! # buttons: UDLRABCS (Up,Down,Left,Right,A,B,C,Start), . = released
//! # For 6-button: UDLRABCSXYZM
//! 0,........,........
//! 60,....A...,........
//! 120,.....B..,........
//! ```

use crate::io::ControllerState;
use std::collections::HashMap;
use std::fs;
use std::path::Path;

/// A single frame's input for both players
#[derive(Debug, Clone, Default)]
pub struct FrameInput {
    pub p1: ControllerState,
    pub p2: ControllerState,
}

/// An input script containing frame-indexed inputs
#[derive(Debug, Default)]
pub struct InputScript {
    /// Map from frame number to input
    frames: HashMap<u64, FrameInput>,
    /// Highest frame number in the script
    pub max_frame: u64,
}

impl InputScript {
    /// Create an empty script
    pub fn new() -> Self {
        Self::default()
    }

    /// Load a script from a file
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, String> {
        let content = fs::read_to_string(path)
            .map_err(|e| format!("Failed to read input script: {}", e))?;
        Self::parse(&content)
    }

    /// Parse a script from a string
    pub fn parse(content: &str) -> Result<Self, String> {
        let mut script = Self::new();

        for (line_num, line) in content.lines().enumerate() {
            let line = line.trim();
            
            // Skip empty lines and comments
            if line.is_empty() || line.starts_with('#') {
                continue;
            }

            let parts: Vec<&str> = line.split(',').collect();
            if parts.len() < 2 {
                return Err(format!("Line {}: expected at least 2 fields", line_num + 1));
            }

            let frame: u64 = parts[0].trim().parse()
                .map_err(|_| format!("Line {}: invalid frame number", line_num + 1))?;

            let p1 = Self::parse_buttons(parts[1].trim())?;
            let p2 = if parts.len() > 2 {
                Self::parse_buttons(parts[2].trim())?
            } else {
                ControllerState::default()
            };

            script.frames.insert(frame, FrameInput { p1, p2 });
            script.max_frame = script.max_frame.max(frame);
        }

        Ok(script)
    }

    /// Parse button string to ControllerState
    /// Format: UDLRABCS for 3-button, UDLRABCSXYZM for 6-button
    /// Use '.' for released buttons
    fn parse_buttons(s: &str) -> Result<ControllerState, String> {
        let mut state = ControllerState::default();
        let chars: Vec<char> = s.chars().collect();

        // Minimum 8 chars for 3-button, 12 for 6-button
        if chars.len() >= 8 {
            state.up    = chars[0] == 'U';
            state.down  = chars[1] == 'D';
            state.left  = chars[2] == 'L';
            state.right = chars[3] == 'R';
            state.a     = chars[4] == 'A';
            state.b     = chars[5] == 'B';
            state.c     = chars[6] == 'C';
            state.start = chars[7] == 'S';
        }

        // 6-button extension
        if chars.len() >= 12 {
            state.x    = chars[8] == 'X';
            state.y    = chars[9] == 'Y';
            state.z    = chars[10] == 'Z';
            state.mode = chars[11] == 'M';
        }

        Ok(state)
    }

    /// Get input for a specific frame
    pub fn get(&self, frame: u64) -> Option<&FrameInput> {
        self.frames.get(&frame)
    }
}

/// Input manager handling script playback and live input
#[derive(Debug)]
pub struct InputManager {
    /// Currently loaded script
    script: Option<InputScript>,
    /// Current frame number
    current_frame: u64,
    /// Last applied input (for hold behavior)
    last_input: FrameInput,
    /// Recording mode
    recording: bool,
    /// Recorded inputs
    recorded: Vec<(u64, FrameInput)>,
}

impl Default for InputManager {
    fn default() -> Self {
        Self::new()
    }
}

impl InputManager {
    /// Create a new input manager
    pub fn new() -> Self {
        Self {
            script: None,
            current_frame: 0,
            last_input: FrameInput::default(),
            recording: false,
            recorded: Vec::new(),
        }
    }

    /// Load an input script
    pub fn load_script<P: AsRef<Path>>(&mut self, path: P) -> Result<(), String> {
        self.script = Some(InputScript::load(path)?);
        self.current_frame = 0;
        self.last_input = FrameInput::default();
        Ok(())
    }

    /// Set script directly
    pub fn set_script(&mut self, script: InputScript) {
        self.script = Some(script);
        self.current_frame = 0;
        self.last_input = FrameInput::default();
    }

    /// Advance to the next frame and return the input
    pub fn advance_frame(&mut self) -> FrameInput {
        let input = if let Some(script) = &self.script {
            if let Some(frame_input) = script.get(self.current_frame) {
                self.last_input = frame_input.clone();
                frame_input.clone()
            } else {
                // No input for this frame - hold last input
                self.last_input.clone()
            }
        } else {
            // No script - return empty input
            FrameInput::default()
        };

        self.current_frame += 1;
        input
    }

    /// Get current frame number
    pub fn frame(&self) -> u64 {
        self.current_frame
    }

    /// Reset to frame 0
    pub fn reset(&mut self) {
        self.current_frame = 0;
        self.last_input = FrameInput::default();
    }

    /// Check if script playback is complete
    pub fn is_complete(&self) -> bool {
        if let Some(script) = &self.script {
            self.current_frame > script.max_frame
        } else {
            false
        }
    }

    /// Start recording
    pub fn start_recording(&mut self) {
        self.recording = true;
        self.recorded.clear();
    }

    /// Record input for current frame
    pub fn record(&mut self, input: FrameInput) {
        if self.recording {
            self.recorded.push((self.current_frame, input));
        }
    }

    /// Stop recording and return the recorded script
    pub fn stop_recording(&mut self) -> InputScript {
        self.recording = false;
        let mut script = InputScript::new();
        for (frame, input) in self.recorded.drain(..) {
            script.frames.insert(frame, input);
            script.max_frame = script.max_frame.max(frame);
        }
        script
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_buttons_basic() {
        let state = InputScript::parse_buttons("....A...").unwrap();
        assert!(state.a);
        assert!(!state.b);
        assert!(!state.up);
    }

    #[test]
    fn test_parse_buttons_multiple() {
        let state = InputScript::parse_buttons("U..RAB..").unwrap();
        assert!(state.up);
        assert!(state.right);
        assert!(state.a);
        assert!(state.b);
        assert!(!state.down);
        assert!(!state.left);
    }

    #[test]
    fn test_parse_buttons_6button() {
        let state = InputScript::parse_buttons("........XYZ.").unwrap();
        assert!(state.x);
        assert!(state.y);
        assert!(state.z);
        assert!(!state.mode);
    }

    #[test]
    fn test_parse_script() {
        let script = InputScript::parse(r#"
# Test script
0,........,........
60,....A...,........
120,.....B..,....A...
"#).unwrap();

        assert_eq!(script.max_frame, 120);
        
        let f0 = script.get(0).unwrap();
        assert!(!f0.p1.a);
        
        let f60 = script.get(60).unwrap();
        assert!(f60.p1.a);
        assert!(!f60.p2.a);
        
        let f120 = script.get(120).unwrap();
        assert!(f120.p1.b);
        assert!(f120.p2.a);
    }

    #[test]
    fn test_input_manager_advance() {
        let mut manager = InputManager::new();
        let script = InputScript::parse("0,....A...,........").unwrap();
        manager.set_script(script);

        let input = manager.advance_frame();
        assert!(input.p1.a);
        assert_eq!(manager.frame(), 1);
    }

    #[test]
    fn test_input_manager_hold() {
        let mut manager = InputManager::new();
        let script = InputScript::parse("0,....A...,........").unwrap();
        manager.set_script(script);

        manager.advance_frame(); // Frame 0 - A pressed
        let input = manager.advance_frame(); // Frame 1 - should hold A
        assert!(input.p1.a);
    }

    #[test]
    fn test_input_recording() {
        let mut manager = InputManager::new();

        manager.start_recording();
        assert!(manager.recording);
        assert!(manager.recorded.is_empty());

        // Frame 0: Press A
        let mut input0 = FrameInput::default();
        input0.p1.a = true;
        manager.record(input0);
        manager.advance_frame();

        // Frame 1: Press B
        let mut input1 = FrameInput::default();
        input1.p1.b = true;
        manager.record(input1);
        manager.advance_frame();

        let script = manager.stop_recording();
        assert!(!manager.recording);

        assert_eq!(script.max_frame, 1);

        let f0 = script.get(0).expect("Frame 0 missing");
        assert!(f0.p1.a);
        assert!(!f0.p1.b);

        let f1 = script.get(1).expect("Frame 1 missing");
        assert!(!f1.p1.a);
        assert!(f1.p1.b);
    }
}
