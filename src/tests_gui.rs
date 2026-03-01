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
    fn test_gui_state_serialization() {
        let mut gui_state = GuiState::new(InputMapping::Ergonomic);
        gui_state.set_window_open("M68k Status", true);
        
        let json = serde_json::to_string(&gui_state).unwrap();
        let decoded: GuiState = serde_json::from_str(&json).unwrap();
        
        assert_eq!(decoded.input_mapping, InputMapping::Ergonomic);
        assert!(decoded.is_window_open("M68k Status"));
        assert!(!decoded.is_window_open("Z80 Status"));
    }
}
