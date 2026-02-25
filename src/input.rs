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
use std::borrow::Cow;
use std::collections::HashMap;
use std::fs::File;
use std::io::Read;
use std::path::Path;

/// Maximum script size in bytes (50MB) to prevent OOM
const MAX_SCRIPT_SIZE: u64 = 50 * 1024 * 1024;

/// A single frame's input for both players
#[derive(Debug, Clone, Default)]
pub struct FrameInput {
    pub p1: ControllerState,
    pub p2: ControllerState,
    pub command: Option<String>,
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
        let file = File::open(path).map_err(|e| format!("Failed to open input script: {}", e))?;

        // Check metadata size first for quick fail
        if let Ok(metadata) = file.metadata() {
            if metadata.len() > MAX_SCRIPT_SIZE {
                return Err(format!(
                    "Input script too large: {} bytes (max {} bytes)",
                    metadata.len(),
                    MAX_SCRIPT_SIZE
                ));
            }
        }

        // Read with limit to prevent OOM from streams/lying metadata
        let mut buffer = Vec::new();
        file.take(MAX_SCRIPT_SIZE + 1)
            .read_to_end(&mut buffer)
            .map_err(|e| format!("Failed to read input script: {}", e))?;

        if buffer.len() as u64 > MAX_SCRIPT_SIZE {
            return Err(format!(
                "Input script too large: exceeds {} bytes",
                MAX_SCRIPT_SIZE
            ));
        }

        let content = String::from_utf8(buffer)
            .map_err(|e| format!("Input script is not valid UTF-8: {}", e))?;

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

            let command = if parts.len() > 3 {
                let cmd = parts[3].trim();
                if cmd.is_empty() {
                    None
                } else {
                    Some(cmd.to_string())
                }
            } else {
                None
            };

            script.frames.insert(frame, FrameInput { p1, p2, command });
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

    /// Save script to a file
    pub fn save<P: AsRef<Path>>(&self, path: P) -> Result<(), String> {
        use std::io::Write;
        let mut file = File::create(path).map_err(|e| format!("Failed to create input script: {}", e))?;

        writeln!(file, "# frame,p1_buttons,p2_buttons").map_err(|e| e.to_string())?;

        // Sort frames to ensure deterministic output
        let mut frames_list: Vec<_> = self.frames.iter().collect();
        frames_list.sort_by_key(|(&f, _)| f);

        for (frame, input) in frames_list {
            write!(file, "{},{},{}", frame, input.p1, input.p2).map_err(|e| e.to_string())?;
            if let Some(cmd) = &input.command {
                write!(file, ",{}", cmd).map_err(|e| e.to_string())?;
            }
            writeln!(file).map_err(|e| e.to_string())?;
        }

        Ok(())
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
    pub fn advance_frame(&mut self) -> Cow<'_, FrameInput> {
        let input = if let Some(script) = &self.script {
            if let Some(frame_input) = script.get(self.current_frame) {
                // Update last_input but EXCLUDE command for hold behavior
                self.last_input.p1 = frame_input.p1;
                self.last_input.p2 = frame_input.p2;
                self.last_input.command = None;
                Cow::Borrowed(frame_input)
            } else {
                // No input for this frame - hold last input (which has None command)
                Cow::Borrowed(&self.last_input)
            }
        } else {
            // No script - return empty input (last_input is default if no script)
            Cow::Borrowed(&self.last_input)
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
        self.script = None;
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

        // Edge Case 1: Empty script (default InputScript, max_frame = 0)
        let script = InputScript::new();
        manager.set_script(script);
        assert!(
            !manager.is_complete(),
            "Empty script frame 0 should not be complete"
        );
        manager.advance_frame();
        assert!(
            manager.is_complete(),
            "Empty script frame 1 should be complete"
        );

        // Edge Case 2: Single frame script at 0
        let script = InputScript::parse("0,........,........").unwrap();
        manager.set_script(script);
        assert!(!manager.is_complete());
        manager.advance_frame();
        assert!(manager.is_complete());

        // Edge Case 3: Script with gaps
        let script = InputScript::parse("0,........,........\n10,........,........").unwrap();
        assert_eq!(script.max_frame, 10);
        manager.set_script(script);

        // Fast forward to frame 10
        for _ in 0..10 {
            assert!(!manager.is_complete());
            manager.advance_frame();
        }
        assert_eq!(manager.frame(), 10);
        assert!(!manager.is_complete()); // At max_frame

        manager.advance_frame();
        assert_eq!(manager.frame(), 11);
        assert!(manager.is_complete()); // After max_frame

        // Edge Case 4: Unordered frames
        let script = InputScript::parse("10,........,........\n5,........,........").unwrap();
        manager.set_script(script);
        // Correctly calculates max frame despite order
        assert_eq!(manager.script.as_ref().unwrap().max_frame, 10);

        // At frame 0
        assert!(!manager.is_complete());
    }

    #[test]
    fn test_input_manager_reset() {
        let mut manager = InputManager::new();

        // 1. Test manual state reset
        manager.current_frame = 100;
        manager.last_input.p1.a = true;
        manager.last_input.p2.start = true;

        assert_eq!(manager.frame(), 100);
        assert!(manager.last_input.p1.a);

        manager.reset();

        assert_eq!(manager.frame(), 0);
        assert!(!manager.last_input.p1.a);
        assert!(!manager.last_input.p2.start);

        // 2. Test script-based state reset (Basic)
        // Frame 0: default
        // Frame 1: A pressed
        let script = InputScript::parse("0,........,........\n1,....A...,........").unwrap();
        manager.set_script(script);

        // Advance to frame 1
        manager.advance_frame(); // Frame 0 processed
        let input = manager.advance_frame().into_owned(); // Frame 1 processed

        assert_eq!(manager.frame(), 2);
        assert!(input.p1.a);
        assert!(manager.last_input.p1.a);

        manager.reset();

        assert_eq!(manager.frame(), 0);
        assert!(!manager.last_input.p1.a);

        // 3. Test script-based state reset (Multi-frame)
        let script = InputScript::parse("0,....A...,........\n1,.....B..,........").unwrap();
        manager.set_script(script);

        // Advance to frame 1, which should set last_input to have A pressed
        let input = manager.advance_frame();
        assert!(input.p1.a);
        assert_eq!(manager.frame(), 1);

        // Advance to frame 2, which should set last_input to have B pressed
        let input = manager.advance_frame();
        assert!(input.p1.b);
        assert_eq!(manager.frame(), 2);

        // Verify internal state is not default
        assert!(manager.last_input.p1.b);

        manager.reset();

        // Verify frame is 0
        assert_eq!(manager.frame(), 0);

        // Verify last_input is default (no A or B pressed)
        assert!(!manager.last_input.p1.a);
        assert!(!manager.last_input.p1.b);

        // Advance frame 0 again
        let input0 = manager.advance_frame();
        assert!(!input0.p1.a);
    }

    #[test]
    fn test_load_nonexistent_file() {
        let result = InputScript::load("non_existent_file.txt");
        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Failed to open input script"));
    }

    #[test]
    fn test_parse_missing_fields() {
        let content = "0";
        let result = InputScript::parse(content);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Line 1: expected at least 2 fields");
    }

    #[test]
    fn test_parse_invalid_frame_number() {
        let content = "invalid,........";
        let result = InputScript::parse(content);
        assert!(result.is_err());
        assert_eq!(result.unwrap_err(), "Line 1: invalid frame number");
    }

    #[test]
    fn test_parse_whitespace_robustness() {
        let script = InputScript::parse(" 10 , ....A... , .....B.. ").unwrap();
        let frame = script.get(10).unwrap();
        assert!(frame.p1.a);
        assert!(frame.p2.b);
    }

    #[test]
    fn test_parse_comments_and_empty_lines() {
        let script = InputScript::parse(
            "
            # Comment 1
            10, ....A..., ........

            # Comment 2
            20, .....B.., ........
        ",
        )
        .unwrap();

        assert_eq!(script.max_frame, 20);
        assert!(script.get(10).unwrap().p1.a);
        assert!(script.get(20).unwrap().p1.b);
    }

    #[test]
    fn test_parse_error_line_numbering() {
        let content = "
            # Line 2 (comment)
            10, ....A..., ........

            invalid_frame, ....A..., ........
        ";
        let err = InputScript::parse(content).unwrap_err();
        assert!(err.contains("Line 5: invalid frame number"));
    }

    #[test]
    fn test_parse_extra_fields() {
        // Extra fields should be ignored based on split(',') logic
        let script = InputScript::parse("10, ....A..., ........, extra_field").unwrap();
        let frame = script.get(10).unwrap();
        assert!(frame.p1.a);
    }

    #[test]
    fn test_parse_empty_buttons() {
        // "10,," -> split gives ["10", "", ""]
        // parse_buttons("") should return default state (all false)
        let script = InputScript::parse("10,,").unwrap();
        let frame = script.get(10).unwrap();
        assert!(!frame.p1.a);
        assert!(!frame.p2.a);
    }

    #[test]
    fn test_load_too_large_file() {
        use std::fs;
        use std::io::Write;
        let path = "test_large_script.txt";
        let mut file = File::create(path).unwrap();

        // Create a file just over the limit (50MB + 1 byte)
        // We write in chunks to avoid OOM in test runner if it were to hold it all
        let chunk_size = 1024 * 1024;
        let chunk = vec![b' '; chunk_size];
        for _ in 0..50 {
            file.write_all(&chunk).unwrap();
        }
        file.write_all(&[b' ']).unwrap(); // +1 byte

        let result = InputScript::load(path);

        // Cleanup
        let _ = fs::remove_file(path);

        assert!(result.is_err());
        let err = result.unwrap_err();
        assert!(err.contains("Input script too large"));
    }
}
