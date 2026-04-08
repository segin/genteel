#[cfg(test)]
mod tests {
    use crate::gui::GuiState;
    use crate::frontend::InputMapping;

    #[test]
    fn test_gui_state_serialization() {
        let mut state = GuiState::new(InputMapping::Original);
        state.set_window_open("Disassembly", true);

        let json = serde_json::to_string(&state).unwrap();
        let loaded: GuiState = serde_json::from_str(&json).unwrap();

        assert_eq!(loaded.input_mapping, InputMapping::Original);
        assert!(loaded.is_window_open("Disassembly"));
    }

    #[test]
    fn test_gui_state_toggle_window() {
        let mut state = GuiState::new(InputMapping::Original);

        assert!(!state.is_window_open("Memory Viewer"));
        state.toggle_window("Memory Viewer");
        assert!(state.is_window_open("Memory Viewer"));
        state.toggle_window("Memory Viewer");
        assert!(!state.is_window_open("Memory Viewer"));
    }
}

    #[test]
    fn test_gui_run_event_loop_failure() {
        // Remove display environment variables to force EventLoop creation to fail
        std::env::remove_var("DISPLAY");
        std::env::remove_var("WAYLAND_DISPLAY");

        let emulator = crate::Emulator::new();
        let result = crate::gui::run(emulator, None);

        // We expect an error because there is no display server available in this headless environment
        assert!(result.is_err(), "Expected run to fail without a display server");
    }
