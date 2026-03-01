#[cfg(all(feature = "gui", test))]
mod tests {
    use crate::gui::GuiState;
    use crate::frontend::InputMapping;

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
    fn test_gui_state_all_windows() {
        let gui_state = GuiState::new(InputMapping::Original);
        let window_names: Vec<_> = gui_state.windows.keys().collect();
        assert!(window_names.contains(&&"Settings".to_string()));
        assert!(window_names.contains(&&"Performance & Debug".to_string()));
        assert!(window_names.contains(&&"M68k Status".to_string()));
        assert!(window_names.contains(&&"Z80 Status".to_string()));
    }
}
