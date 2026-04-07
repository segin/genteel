//! Frontend Module - winit + pixels
//!
//! Provides cross-platform windowing, input handling, and rendering
//! for the Genesis emulator using pure Rust libraries.

use std::path::PathBuf;
#[cfg(any(feature = "gui", feature = "test_headless"))]
use winit::keyboard::{Key, KeyCode};

/// Genesis display dimensions
pub const GENESIS_WIDTH: u32 = 320;
pub const GENESIS_HEIGHT: u32 = 240;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub enum InputMapping {
    #[default]
    Original,
    Ergonomic,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsgToneInfo {
    pub frequency: u16,
    pub volume: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PsgNoiseInfo {
    pub volume: u8,
    pub white: bool,
    pub rate: u8,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DebugInfo {
    pub frame_count: u64,
    pub m68k_pc: u32,
    pub z80_pc: u16,
    pub display_enabled: bool,
    pub vdp_status: u16,
    pub bg_color_index: usize,
    pub cram: Vec<u16>,
    pub cram_raw: Vec<u16>,
    pub vram: Vec<u8>,
    pub vsram: Vec<u8>,
    pub wram: Vec<u8>,
    pub z80_ram: Vec<u8>,
    pub psg_tone: [PsgToneInfo; 3],
    pub psg_noise: PsgNoiseInfo,
    pub vdp_registers: [u8; 24],
    pub m68k_disasm: Vec<(u32, String)>,
    pub z80_disasm: Vec<(u16, String)>,
    pub port1_type: crate::io::ControllerType,
    pub port2_type: crate::io::ControllerType,
    pub has_rom: bool,
    pub current_rom_path: Option<PathBuf>,
}

use serde::{Deserialize, Serialize};

/// Key mapping for player 1 (Physical KeyCode)
#[cfg(any(feature = "gui", feature = "test_headless"))]
pub fn keycode_to_button(keycode: KeyCode, mapping: InputMapping) -> Option<(&'static str, bool)> {
    match mapping {
        InputMapping::Original => match keycode {
            // Player 1 - Arrow keys + ZXC/Enter
            KeyCode::ArrowUp => Some(("up", true)),
            KeyCode::ArrowDown => Some(("down", true)),
            KeyCode::ArrowLeft => Some(("left", true)),
            KeyCode::ArrowRight => Some(("right", true)),
            KeyCode::KeyZ => Some(("a", true)),
            KeyCode::KeyX => Some(("b", true)),
            KeyCode::KeyC => Some(("c", true)),
            KeyCode::Enter => Some(("start", true)),
            // 6-button extension
            KeyCode::KeyA => Some(("x", true)),
            KeyCode::KeyS => Some(("y", true)),
            KeyCode::KeyD => Some(("z", true)),
            KeyCode::KeyQ => Some(("mode", true)),
            _ => None,
        },
        InputMapping::Ergonomic => match keycode {
            // Player 1 - D-pad (WASD physical or Arrow keys)
            KeyCode::KeyW | KeyCode::ArrowUp => Some(("up", true)),
            KeyCode::KeyS | KeyCode::ArrowDown => Some(("down", true)),
            KeyCode::KeyA | KeyCode::ArrowLeft => Some(("left", true)),
            KeyCode::KeyD | KeyCode::ArrowRight => Some(("right", true)),

            // Face Buttons (Bottom Row: J, K, L -> A, B, C)
            KeyCode::KeyJ => Some(("a", true)),
            KeyCode::KeyK => Some(("b", true)),
            KeyCode::KeyL => Some(("c", true)),

            // Face Buttons (Top Row: U, I, O -> X, Y, Z)
            KeyCode::KeyU => Some(("x", true)),
            KeyCode::KeyI => Some(("y", true)),
            KeyCode::KeyO => Some(("z", true)),

            // System Buttons
            KeyCode::Enter => Some(("start", true)),
            KeyCode::Space => Some(("mode", true)),

            // Legacy/Alternative Mapping (ZX for A/B)
            KeyCode::KeyZ => Some(("a", true)),
            KeyCode::KeyX => Some(("b", true)),
            KeyCode::KeyC => Some(("c", true)),
            _ => None,
        },
    }
}

/// Key mapping for player 1 (Logical Key)
#[cfg(any(feature = "gui", feature = "test_headless"))]
pub fn key_to_button(key: &Key, mapping: InputMapping) -> Option<(&'static str, bool)> {
    match mapping {
        InputMapping::Original => match key {
            Key::Named(winit::keyboard::NamedKey::ArrowUp) => Some(("up", true)),
            Key::Named(winit::keyboard::NamedKey::ArrowDown) => Some(("down", true)),
            Key::Named(winit::keyboard::NamedKey::ArrowLeft) => Some(("left", true)),
            Key::Named(winit::keyboard::NamedKey::ArrowRight) => Some(("right", true)),
            Key::Character(s) if s == "z" || s == "Z" => Some(("a", true)),
            Key::Character(s) if s == "x" || s == "X" => Some(("b", true)),
            Key::Character(s) if s == "c" || s == "C" => Some(("c", true)),
            Key::Named(winit::keyboard::NamedKey::Enter) => Some(("start", true)),
            _ => None,
        },
        InputMapping::Ergonomic => match key {
            Key::Character(s) if s == "w" || s == "W" => Some(("up", true)),
            Key::Character(s) if s == "s" || s == "S" => Some(("down", true)),
            Key::Character(s) if s == "a" || s == "A" => Some(("left", true)),
            Key::Character(s) if s == "d" || s == "D" => Some(("right", true)),
            Key::Character(s) if s == "j" || s == "J" => Some(("a", true)),
            Key::Character(s) if s == "k" || s == "K" => Some(("b", true)),
            Key::Character(s) if s == "l" || s == "L" => Some(("c", true)),
            Key::Character(s) if s == "u" || s == "U" => Some(("x", true)),
            Key::Character(s) if s == "i" || s == "I" => Some(("y", true)),
            Key::Character(s) if s == "o" || s == "O" => Some(("z", true)),
            Key::Named(winit::keyboard::NamedKey::Enter) => Some(("start", true)),
            Key::Named(winit::keyboard::NamedKey::Space) => Some(("mode", true)),
            _ => None,
        },
    }
}

/// Convert RGB565 framebuffer to RGBA8 for pixels crate
pub fn rgb565_to_rgba8(framebuffer_565: &[u16], output: &mut [u8]) {
    for (&pixel, chunk) in framebuffer_565.iter().zip(output.chunks_exact_mut(4)) {
        // Extract RGB565 components
        let r5 = ((pixel >> 11) & 0x1F) as u8;
        let g6 = ((pixel >> 5) & 0x3F) as u8;
        let b5 = (pixel & 0x1F) as u8;

        // Scale to 8-bit
        chunk[0] = (r5 << 3) | (r5 >> 2); // R
        chunk[1] = (g6 << 2) | (g6 >> 4); // G
        chunk[2] = (b5 << 3) | (b5 >> 2); // B
        chunk[3] = 255; // A
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_keycode_mapping() {
        assert_eq!(
            keycode_to_button(KeyCode::KeyZ, InputMapping::Original),
            Some(("a", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyX, InputMapping::Original),
            Some(("b", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowUp, InputMapping::Original),
            Some(("up", true))
        );

        // Ensure other basic mappings map properly
        assert_eq!(
            keycode_to_button(KeyCode::ArrowDown, InputMapping::Original),
            Some(("down", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyC, InputMapping::Original),
            Some(("c", true))
        );
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_original_mapping_comprehensive() {
        let mapping = InputMapping::Original;

        // Directional keys
        assert_eq!(
            keycode_to_button(KeyCode::ArrowUp, mapping),
            Some(("up", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowDown, mapping),
            Some(("down", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowLeft, mapping),
            Some(("left", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowRight, mapping),
            Some(("right", true))
        );

        // Action keys (ABC)
        assert_eq!(keycode_to_button(KeyCode::KeyZ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyX, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyC, mapping), Some(("c", true)));

        // Start
        assert_eq!(
            keycode_to_button(KeyCode::Enter, mapping),
            Some(("start", true))
        );

        // 6-button extension (XYZ Mode)
        assert_eq!(keycode_to_button(KeyCode::KeyA, mapping), Some(("x", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyS, mapping), Some(("y", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyD, mapping), Some(("z", true)));
        assert_eq!(
            keycode_to_button(KeyCode::KeyQ, mapping),
            Some(("mode", true))
        );
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_ergonomic_mapping_comprehensive() {
        let mapping = InputMapping::Ergonomic;

        // Directional keys (WASD)
        assert_eq!(
            keycode_to_button(KeyCode::KeyW, mapping),
            Some(("up", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyS, mapping),
            Some(("down", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyA, mapping),
            Some(("left", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyD, mapping),
            Some(("right", true))
        );

        // Directional keys (Arrows)
        assert_eq!(
            keycode_to_button(KeyCode::ArrowUp, mapping),
            Some(("up", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowDown, mapping),
            Some(("down", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowLeft, mapping),
            Some(("left", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::ArrowRight, mapping),
            Some(("right", true))
        );

        // Face Buttons (JKL -> ABC)
        assert_eq!(keycode_to_button(KeyCode::KeyJ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyK, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyL, mapping), Some(("c", true)));

        // Face Buttons (UIO -> XYZ)
        assert_eq!(keycode_to_button(KeyCode::KeyU, mapping), Some(("x", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyI, mapping), Some(("y", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyO, mapping), Some(("z", true)));

        // System Buttons
        assert_eq!(
            keycode_to_button(KeyCode::Enter, mapping),
            Some(("start", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::Space, mapping),
            Some(("mode", true))
        );

        // Legacy/Alternative Mapping (ZX -> AB)
        assert_eq!(keycode_to_button(KeyCode::KeyZ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyX, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyC, mapping), Some(("c", true)));
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_unmapped_keys() {
        let mappings = [InputMapping::Original, InputMapping::Ergonomic];

        // Comprehensive list of commonly unmapped keys to ensure they don't accidentally trigger actions
        let unmapped_keys = [
            // Unmapped letters (B, E, F, G, H, M, N, P, R, T, V, X/Y/Z depending on mapping)
            KeyCode::KeyB,
            KeyCode::KeyE,
            KeyCode::KeyF,
            KeyCode::KeyG,
            KeyCode::KeyH,
            KeyCode::KeyM,
            KeyCode::KeyN,
            KeyCode::KeyP,
            KeyCode::KeyR,
            KeyCode::KeyT,
            KeyCode::KeyV,
            // Digits
            KeyCode::Digit0,
            KeyCode::Digit1,
            KeyCode::Digit2,
            KeyCode::Digit3,
            KeyCode::Digit4,
            KeyCode::Digit5,
            KeyCode::Digit6,
            KeyCode::Digit7,
            KeyCode::Digit8,
            KeyCode::Digit9,
            // Function keys
            KeyCode::F1,
            KeyCode::F2,
            KeyCode::F3,
            KeyCode::F4,
            KeyCode::F5,
            KeyCode::F6,
            KeyCode::F7,
            KeyCode::F8,
            KeyCode::F9,
            KeyCode::F10,
            KeyCode::F11,
            KeyCode::F12,
            // Special keys
            KeyCode::Escape,
            KeyCode::ShiftLeft,
            KeyCode::ShiftRight,
            KeyCode::ControlLeft,
            KeyCode::ControlRight,
            KeyCode::AltLeft,
            KeyCode::AltRight,
            KeyCode::Tab,
            KeyCode::Backspace,
        ];

        for mapping in mappings {
            for key in unmapped_keys {
                // Some keys are unmapped in one mode but mapped in another
                // We test keys that should be unmapped in BOTH modes here.
                assert_eq!(
                    keycode_to_button(key, mapping),
                    None,
                    "Key {:?} should not be mapped in {:?} mode",
                    key,
                    mapping
                );
            }
        }
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_overlapping_keys() {
        // Verify keys that map to different things or same things across mappings

        // Enter is start in both
        assert_eq!(
            keycode_to_button(KeyCode::Enter, InputMapping::Original),
            Some(("start", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::Enter, InputMapping::Ergonomic),
            Some(("start", true))
        );

        // 'Z' is 'a' in Original, and also 'a' in Ergonomic (legacy)
        assert_eq!(
            keycode_to_button(KeyCode::KeyZ, InputMapping::Original),
            Some(("a", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyZ, InputMapping::Ergonomic),
            Some(("a", true))
        );

        // 'A' is 'x' in Original, but 'left' in Ergonomic
        assert_eq!(
            keycode_to_button(KeyCode::KeyA, InputMapping::Original),
            Some(("x", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyA, InputMapping::Ergonomic),
            Some(("left", true))
        );

        // 'S' is 'y' in Original, but 'down' in Ergonomic
        assert_eq!(
            keycode_to_button(KeyCode::KeyS, InputMapping::Original),
            Some(("y", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyS, InputMapping::Ergonomic),
            Some(("down", true))
        );

        // 'D' is 'z' in Original, but 'right' in Ergonomic
        assert_eq!(
            keycode_to_button(KeyCode::KeyD, InputMapping::Original),
            Some(("z", true))
        );
        assert_eq!(
            keycode_to_button(KeyCode::KeyD, InputMapping::Ergonomic),
            Some(("right", true))
        );
    }

    #[test]
    fn test_rgb565_to_rgba8_black() {
        let input = [0x0000u16];
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&input, &mut output);
        assert_eq!(output, [0, 0, 0, 255]);
    }

    #[test]
    fn test_rgb565_to_rgba8_white() {
        let input = [0xFFFFu16];
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&input, &mut output);
        assert_eq!(output, [255, 255, 255, 255]);
    }

    #[test]
    fn test_rgb565_to_rgba8_colors() {
        // Red: 0xF800 (11111 000000 00000)
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&[0xF800], &mut output);
        assert_eq!(output, [255, 0, 0, 255]);

        // Green: 0x07E0 (00000 111111 00000)
        rgb565_to_rgba8(&[0x07E0], &mut output);
        assert_eq!(output, [0, 255, 0, 255]);

        // Blue: 0x001F (00000 000000 11111)
        rgb565_to_rgba8(&[0x001F], &mut output);
        assert_eq!(output, [0, 0, 255, 255]);

        // A mid gray
        rgb565_to_rgba8(&[0x8410], &mut output); // 10000 100000 10000
                                                 // r: (16 << 3) | (16 >> 2) = 128 | 4 = 132
                                                 // g: (32 << 2) | (32 >> 4) = 128 | 2 = 130
                                                 // b: (16 << 3) | (16 >> 2) = 128 | 4 = 132
        assert_eq!(output, [132, 130, 132, 255]);
    }

    #[test]
    fn test_rgb565_to_rgba8_edge_cases() {
        // Test empty input/output
        let empty_input: [u16; 0] = [];
        let mut empty_output: [u8; 0] = [];
        rgb565_to_rgba8(&empty_input, &mut empty_output); // Should not panic

        // Mismatched lengths: Output too small (should process as much as it can)
        let input = [0xFFFF, 0xFFFF];
        let mut short_output = [0u8; 4]; // Only room for 1 pixel
        rgb565_to_rgba8(&input, &mut short_output);
        assert_eq!(short_output, [255, 255, 255, 255]); // First pixel processed correctly

        // Mismatched lengths: Output < 4 bytes (should not process at all due to chunks_exact_mut(4))
        let mut very_short_output = [1, 2, 3];
        rgb565_to_rgba8(&input, &mut very_short_output);
        assert_eq!(very_short_output, [1, 2, 3]); // Should be entirely untouched

        // Mismatched lengths: Output too large (should leave remainder untouched)
        let input_short = [0x0000]; // 1 pixel (black)
        let mut long_output = [255u8; 8]; // Room for 2 pixels, initialized to white
        rgb565_to_rgba8(&input_short, &mut long_output);
        // First pixel is black, second pixel remains white
        assert_eq!(long_output, [0, 0, 0, 255, 255, 255, 255, 255]);

        // Partial chunks at the end of output should be ignored safely
        let input_partial = [0xFFFF, 0xFFFF];
        let mut partial_output = [0u8; 7]; // Room for 1 pixel + 3 bytes
        rgb565_to_rgba8(&input_partial, &mut partial_output);
        // First pixel processed, remaining 3 bytes untouched
        assert_eq!(partial_output, [255, 255, 255, 255, 0, 0, 0]);

        // Test near-zero edge case (LSB set)
        // R:1, G:0, B:0 -> 0x0800 (00001 000000 00000)
        let mut output = [0u8; 4];
        rgb565_to_rgba8(&[0x0800], &mut output);
        // r: (1 << 3) | (1 >> 2) = 8 | 0 = 8
        assert_eq!(output, [8, 0, 0, 255]);

        // R:0, G:1, B:0 -> 0x0020 (00000 000001 00000)
        rgb565_to_rgba8(&[0x0020], &mut output);
        // g: (1 << 2) | (1 >> 4) = 4 | 0 = 4
        assert_eq!(output, [0, 4, 0, 255]);

        // R:0, G:0, B:1 -> 0x0001 (00000 000000 00001)
        rgb565_to_rgba8(&[0x0001], &mut output);
        // b: (1 << 3) | (1 >> 2) = 8 | 0 = 8
        assert_eq!(output, [0, 0, 8, 255]);

        // Test max-1 edge case (MSB cleared)
        // R:30, G:63, B:31 -> 0xF7FF (11110 111111 11111)
        rgb565_to_rgba8(&[0xF7FF], &mut output);
        // r: (30 << 3) | (30 >> 2) = 240 | 7 = 247
        assert_eq!(output, [247, 255, 255, 255]);

        // R:31, G:62, B:31 -> 0xFFDF (11111 111110 11111)
        rgb565_to_rgba8(&[0xFFDF], &mut output);
        // g: (62 << 2) | (62 >> 4) = 248 | 3 = 251
        assert_eq!(output, [255, 251, 255, 255]);

        // R:31, G:63, B:30 -> 0xFFFE (11111 111111 11110)
        rgb565_to_rgba8(&[0xFFFE], &mut output);
        // b: (30 << 3) | (30 >> 2) = 240 | 7 = 247
        assert_eq!(output, [255, 255, 247, 255]);
    }

    #[test]
    fn test_rgb565_to_rgba8_exhaustive() {
        let all_pixels: Vec<u16> = (0..=65535).collect();
        let mut output = vec![0u8; 65536 * 4];
        rgb565_to_rgba8(&all_pixels, &mut output);

        for (i, &pixel) in all_pixels.iter().enumerate() {
            let chunk = &output[i * 4..(i + 1) * 4];
            let r5 = ((pixel >> 11) & 0x1F) as u8;
            let g6 = ((pixel >> 5) & 0x3F) as u8;
            let b5 = (pixel & 0x1F) as u8;

            let expected_r = (r5 << 3) | (r5 >> 2);
            let expected_g = (g6 << 2) | (g6 >> 4);
            let expected_b = (b5 << 3) | (b5 >> 2);

            assert_eq!(chunk[0], expected_r);
            assert_eq!(chunk[1], expected_g);
            assert_eq!(chunk[2], expected_b);
            assert_eq!(chunk[3], 255);
        }
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_keycode_to_button_exhaustive_table() {
        let test_cases = [
            // Original Mapping
            (InputMapping::Original, KeyCode::ArrowUp, Some(("up", true))),
            (
                InputMapping::Original,
                KeyCode::ArrowDown,
                Some(("down", true)),
            ),
            (
                InputMapping::Original,
                KeyCode::ArrowLeft,
                Some(("left", true)),
            ),
            (
                InputMapping::Original,
                KeyCode::ArrowRight,
                Some(("right", true)),
            ),
            (InputMapping::Original, KeyCode::KeyZ, Some(("a", true))),
            (InputMapping::Original, KeyCode::KeyX, Some(("b", true))),
            (InputMapping::Original, KeyCode::KeyC, Some(("c", true))),
            (
                InputMapping::Original,
                KeyCode::Enter,
                Some(("start", true)),
            ),
            (InputMapping::Original, KeyCode::KeyA, Some(("x", true))),
            (InputMapping::Original, KeyCode::KeyS, Some(("y", true))),
            (InputMapping::Original, KeyCode::KeyD, Some(("z", true))),
            (InputMapping::Original, KeyCode::KeyQ, Some(("mode", true))),
            (InputMapping::Original, KeyCode::KeyB, None),
            // Ergonomic Mapping
            (InputMapping::Ergonomic, KeyCode::KeyW, Some(("up", true))),
            (InputMapping::Ergonomic, KeyCode::KeyS, Some(("down", true))),
            (InputMapping::Ergonomic, KeyCode::KeyA, Some(("left", true))),
            (
                InputMapping::Ergonomic,
                KeyCode::KeyD,
                Some(("right", true)),
            ),
            (
                InputMapping::Ergonomic,
                KeyCode::ArrowUp,
                Some(("up", true)),
            ),
            (InputMapping::Ergonomic, KeyCode::KeyJ, Some(("a", true))),
            (InputMapping::Ergonomic, KeyCode::KeyK, Some(("b", true))),
            (InputMapping::Ergonomic, KeyCode::KeyL, Some(("c", true))),
            (InputMapping::Ergonomic, KeyCode::KeyU, Some(("x", true))),
            (InputMapping::Ergonomic, KeyCode::KeyI, Some(("y", true))),
            (InputMapping::Ergonomic, KeyCode::KeyO, Some(("z", true))),
            (
                InputMapping::Ergonomic,
                KeyCode::Enter,
                Some(("start", true)),
            ),
            (
                InputMapping::Ergonomic,
                KeyCode::Space,
                Some(("mode", true)),
            ),
            (InputMapping::Ergonomic, KeyCode::KeyZ, Some(("a", true))),
            (InputMapping::Ergonomic, KeyCode::KeyX, Some(("b", true))),
            (InputMapping::Ergonomic, KeyCode::KeyC, Some(("c", true))),
            (InputMapping::Ergonomic, KeyCode::KeyE, None),
        ];

        for (mapping, keycode, expected) in test_cases {
            let result = keycode_to_button(keycode, mapping);
            assert_eq!(
                result, expected,
                "Failed for mapping {:?} and keycode {:?}",
                mapping, keycode
            );
        }
    }
}
