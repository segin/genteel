#[cfg(all(feature = "gui", test))]
mod tests {
    use crate::frontend::InputMapping;
    use crate::gui::{GuiState, WindowState};
    use std::path::PathBuf;
    use std::sync::{Arc, Mutex};
    use std::thread;

    #[test]
    fn test_window_state_default() {
        let state = WindowState::default();
        assert!(!state.open);
    }

    #[test]
    fn test_window_state_modification() {
        let mut state = WindowState::default();
        state.open = true;
        assert!(state.open);
        state.open = false;
        assert!(!state.open);
    }

    #[test]
    fn test_gui_state_window_management() {
        let mut gui_state = GuiState::new(InputMapping::Original);

        // Initially no windows should be open besides the defaults if any
        assert!(!gui_state.is_window_open("M68k Status"));

        gui_state.toggle_window("M68k Status");
        assert!(gui_state.is_window_open("M68k Status"));

        gui_state.toggle_window("M68k Status");
        assert!(!gui_state.is_window_open("M68k Status"));
    }

    #[test]
    fn test_gui_state_set_window_open() {
        let mut gui_state = GuiState::new(InputMapping::Original);

        // Test setting existing window to open
        gui_state.set_window_open("M68k Status", true);
        assert!(gui_state.is_window_open("M68k Status"));

        // Test setting existing window to closed
        gui_state.set_window_open("M68k Status", false);
        assert!(!gui_state.is_window_open("M68k Status"));

        // Test setting non-existent window to open (should be added)
        assert!(!gui_state.is_window_open("Non Existent"));
        gui_state.set_window_open("Non Existent", true);
        assert!(gui_state.is_window_open("Non Existent"));
    }

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

    #[cfg(feature = "gilrs")]
    #[test]
    fn test_init_gilrs_error_path() {
        use crate::gui::init_gilrs_with_builder;

        let builder = || -> Result<gilrs::Gilrs, &'static str> { Err("simulated OS failure") };

        let result = init_gilrs_with_builder(builder);
        assert!(result.is_none());
    }

    #[test]
    fn test_gui_pending_rom_path_mutex_poison() {
        let pending_rom_path: Arc<Mutex<Option<PathBuf>>> = Arc::new(Mutex::new(None));
        let pending_clone = Arc::clone(&pending_rom_path);

        // Intentionally poison the mutex by panicking while holding the lock
        let handle = thread::spawn(move || {
            let _lock = pending_clone.lock().unwrap();
            panic!("Intentionally poisoning the mutex!");
        });

        // Wait for the thread to panic
        let _ = handle.join();

        // The mutex should now be poisoned
        let lock_result = pending_rom_path.lock();
        assert!(
            lock_result.is_err(),
            "Mutex should be poisoned after a panic while holding the lock"
        );
    }

    #[test]
    fn test_gui_run_event_loop_failure() {
        // Remove display environment variables to force EventLoop creation to fail
        std::env::remove_var("DISPLAY");
        std::env::remove_var("WAYLAND_DISPLAY");

        let emulator = crate::Emulator::new();
        let result = crate::gui::run(emulator, None);

        // We expect an error because there is no display server available in this headless environment
        assert!(
            result.is_err(),
            "Expected run to fail without a display server"
        );
    }
}
