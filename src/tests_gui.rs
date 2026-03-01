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
    fn test_gui_state_initialization() {
        let gui_state = GuiState::new(InputMapping::Ergonomic);
        assert_eq!(gui_state.input_mapping, InputMapping::Ergonomic);
        assert!(gui_state.integer_scaling);
        assert!(!gui_state.force_red);
    }
}
