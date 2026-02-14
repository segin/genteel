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
        let content =
            fs::read_to_string(path).map_err(|e| format!("Failed to read input script: {}", e))?;
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

            let frame: u64 = parts[0]
                .trim()
                .parse()
                .map_err(|_| format!("Line {}: invalid frame number", line_num + 1))?;

            let p1 = Self::parse_buttons(parts[1].trim());
            let p2 = if parts.len() > 2 {
                Self::parse_buttons(parts[2].trim())
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
    fn parse_buttons(s: &str) -> ControllerState {
        let mut state = ControllerState::default();
        let mut chars = s.chars();

        state.up = chars.next() == Some('U');
        state.down = chars.next() == Some('D');
        state.left = chars.next() == Some('L');
        state.right = chars.next() == Some('R');
        state.a = chars.next() == Some('A');
        state.b = chars.next() == Some('B');
        state.c = chars.next() == Some('C');
        state.start = chars.next() == Some('S');

        // 6-button extension
        state.x = chars.next() == Some('X');
        state.y = chars.next() == Some('Y');
        state.z = chars.next() == Some('Z');
        state.mode = chars.next() == Some('M');

        state
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
        let state = InputScript::parse_buttons("....A...");
        assert!(state.a);
        assert!(!state.b);
        assert!(!state.up);
    }

    #[test]
    fn test_parse_buttons_multiple() {
        let state = InputScript::parse_buttons("U..RAB..");
        assert!(state.up);
        assert!(state.right);
        assert!(state.a);
        assert!(state.b);
        assert!(!state.down);
        assert!(!state.left);
    }

    #[test]
    fn test_parse_buttons_6button() {
        let state = InputScript::parse_buttons("........XYZ.");
        assert!(state.x);
        assert!(state.y);
        assert!(state.z);
        assert!(!state.mode);
    }

    #[test]
    fn test_parse_script() {
        let script = InputScript::parse(
            r#"
# Test script
0,........,........
60,....A...,........
120,.....B..,....A...
"#,
        )
        .unwrap();

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
    fn test_parse_buttons_short() {
        let state = InputScript::parse_buttons("short");
        // "short" has 5 chars.
        // U='s', D='h', L='o', R='r', A='t' -> None match UDLRABCSXYZM
        // So all should be false
        assert!(!state.up);
        assert!(!state.down);
        assert!(!state.left);
        assert!(!state.right);
        assert!(!state.a);
        assert!(!state.b);
        assert!(!state.c);
        assert!(!state.start);
    }

    #[test]
    fn test_recording_functionality() {
        let mut manager = InputManager::new();

        // Start recording
        manager.start_recording();
        assert!(manager.recording);
        assert!(manager.recorded.is_empty());

        // Frame 0: Press A
        let mut input0 = FrameInput::default();
        input0.p1.a = true;
        manager.record(input0.clone());
        manager.advance_frame();

        // Frame 1: Press B
        let mut input1 = FrameInput::default();
        input1.p1.b = true;
        manager.record(input1.clone());
        manager.advance_frame();

        // Frame 2: Press Start
        let mut input2 = FrameInput::default();
        input2.p1.start = true;
        manager.record(input2.clone());
        // Don't advance frame after last record

        // Stop recording
        let script = manager.stop_recording();
        assert!(!manager.recording);

        // Verify script content
        assert_eq!(script.max_frame, 2);

        let f0 = script.get(0).unwrap();
        assert!(f0.p1.a);
        assert!(!f0.p1.b);

        let f1 = script.get(1).unwrap();
        assert!(f1.p1.b);
        assert!(!f1.p1.a);

        let f2 = script.get(2).unwrap();
        assert!(f2.p1.start);
    }

    #[test]
    fn test_input_manager_completion() {
        let mut manager = InputManager::new();

        // No script - should not be complete
        assert!(!manager.is_complete());

        // Load a script with 2 frames (0 and 1)
        let script = InputScript::parse("0,........,........\n1,....A...,........").unwrap();
        manager.set_script(script);
        assert_eq!(manager.frame(), 0);

        // Frame 0 - not complete
        assert!(!manager.is_complete());
        manager.advance_frame();
        assert_eq!(manager.frame(), 1);

        // Frame 1 - not complete (this is the max_frame)
        assert!(!manager.is_complete());
        manager.advance_frame();
        assert_eq!(manager.frame(), 2);

        // Now it should be complete (current_frame 2 > max_frame 1)
        assert!(manager.is_complete());

        // Reset should make it not complete again
        manager.reset();
        assert_eq!(manager.frame(), 0);
        assert!(!manager.is_complete());
    }

    #[test]
    fn test_input_manager_completion_edge_cases() {
        let mut manager = InputManager::new();

        // 1. Script with only frame 0
        let script = InputScript::parse("0,........,........").unwrap();
        manager.set_script(script);

        // Frame 0 - not complete
        assert!(
            !manager.is_complete(),
            "Should not be complete at frame 0 (max_frame=0)"
        );

        manager.advance_frame();
        // Frame 1 - complete
        assert!(
            manager.is_complete(),
            "Should be complete at frame 1 for single-frame script"
        );

        // 2. Script with sparse frames (e.g. max_frame=10)
        let script = InputScript::parse("10,........,........").unwrap();
        manager.set_script(script);

        // Advance to frame 10
        for _ in 0..10 {
            assert!(!manager.is_complete());
            manager.advance_frame();
        }
        assert_eq!(manager.frame(), 10);
        assert!(
            !manager.is_complete(),
            "Should not be complete at frame 10 (max_frame=10)"
        );

        manager.advance_frame();
        assert_eq!(manager.frame(), 11);
        assert!(manager.is_complete(), "Should be complete at frame 11");

        // 3. Continued execution
        manager.advance_frame();
        assert!(manager.is_complete(), "Should remain complete at frame 12");

        // 4. Empty script (no frames)
        let script = InputScript::parse("# empty").unwrap();
        manager.set_script(script);
        // max_frame is 0 by default for empty script
        assert!(
            !manager.is_complete(),
            "Empty script should behave like max_frame=0"
        );
        manager.advance_frame();
        assert!(
            manager.is_complete(),
            "Empty script should complete after frame 0"
        );
    }
}
