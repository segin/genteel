//! Frontend Module - winit + pixels
//!
//! Provides cross-platform windowing, input handling, and rendering
//! for the Genesis emulator using pure Rust libraries.

#[cfg(any(feature = "gui", feature = "test_headless"))]
use winit::keyboard::KeyCode;

/// Genesis display dimensions
pub const GENESIS_WIDTH: u32 = 320;
pub const GENESIS_HEIGHT: u32 = 240;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum InputMapping {
    #[default]
    Original,
    Ergonomic,
}

/// Key mapping for player 1
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
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_original_mapping_comprehensive() {
        let mapping = InputMapping::Original;

        // Directional keys
        assert_eq!(keycode_to_button(KeyCode::ArrowUp, mapping), Some(("up", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowDown, mapping), Some(("down", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowLeft, mapping), Some(("left", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowRight, mapping), Some(("right", true)));

        // Action keys (ABC)
        assert_eq!(keycode_to_button(KeyCode::KeyZ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyX, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyC, mapping), Some(("c", true)));

        // Start
        assert_eq!(keycode_to_button(KeyCode::Enter, mapping), Some(("start", true)));

        // 6-button extension (XYZ Mode)
        assert_eq!(keycode_to_button(KeyCode::KeyA, mapping), Some(("x", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyS, mapping), Some(("y", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyD, mapping), Some(("z", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyQ, mapping), Some(("mode", true)));
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_ergonomic_mapping_comprehensive() {
        let mapping = InputMapping::Ergonomic;

        // Directional keys (WASD)
        assert_eq!(keycode_to_button(KeyCode::KeyW, mapping), Some(("up", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyS, mapping), Some(("down", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyA, mapping), Some(("left", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyD, mapping), Some(("right", true)));

        // Directional keys (Arrows)
        assert_eq!(keycode_to_button(KeyCode::ArrowUp, mapping), Some(("up", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowDown, mapping), Some(("down", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowLeft, mapping), Some(("left", true)));
        assert_eq!(keycode_to_button(KeyCode::ArrowRight, mapping), Some(("right", true)));

        // Face Buttons (JKL -> ABC)
        assert_eq!(keycode_to_button(KeyCode::KeyJ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyK, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyL, mapping), Some(("c", true)));

        // Face Buttons (UIO -> XYZ)
        assert_eq!(keycode_to_button(KeyCode::KeyU, mapping), Some(("x", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyI, mapping), Some(("y", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyO, mapping), Some(("z", true)));

        // System Buttons
        assert_eq!(keycode_to_button(KeyCode::Enter, mapping), Some(("start", true)));
        assert_eq!(keycode_to_button(KeyCode::Space, mapping), Some(("mode", true)));

        // Legacy/Alternative Mapping (ZX -> AB)
        assert_eq!(keycode_to_button(KeyCode::KeyZ, mapping), Some(("a", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyX, mapping), Some(("b", true)));
        assert_eq!(keycode_to_button(KeyCode::KeyC, mapping), Some(("c", true)));
    }

    #[cfg(any(feature = "gui", feature = "test_headless"))]
    #[test]
    fn test_unmapped_keys() {
        let mappings = [InputMapping::Original, InputMapping::Ergonomic];
        let unmapped_keys = [
            KeyCode::KeyB,
            KeyCode::KeyE,
            KeyCode::KeyF,
            KeyCode::KeyH,
            KeyCode::Digit1,
            KeyCode::F1,
            KeyCode::Escape,
            KeyCode::ShiftLeft,
        ];

        for mapping in mappings {
            for key in unmapped_keys {
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
}
