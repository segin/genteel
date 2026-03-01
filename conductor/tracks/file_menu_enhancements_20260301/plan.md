# Implementation Plan: File Menu Enhancements and Automated Builds

## Phase 1: CI/CD Pipeline [checkpoint: dda3a76]
- [x] Task: Create GitHub Actions workflow for multi-platform release builds (Linux/Windows). 81041c1
- [x] Task: Verify that the workflow produces downloadable and runnable artifacts for both platforms. 56227a0
- [x] Task: Conductor - User Manual Verification 'Phase 1: CI/CD Pipeline' (Protocol in workflow.md) dda3a76

## Phase 2: Native File Dialogs and Basic Menu
- [x] Task: Add `rfd` dependency and implement a non-blocking wrapper for native file selection. b512382
- [ ] Task: Implement "Open ROM" action in the File menu using the native dialog.
- [ ] Task: Implement "Reset ROM" and "Close ROM" logic, ensuring all volatile state (VRAM, RAM, registers) is cleared.
- [ ] Task: Implement "Open Recent" tracking and persistence in `gui_config.json`.
- [ ] Task: Conductor - User Manual Verification 'Phase 2: Native File Dialogs and Basic Menu' (Protocol in workflow.md)

## Phase 3: SRAM and State Management
- [ ] Task: Implement SRAM persistence logic (loading/saving .srm files automatically based on ROM filename).
- [ ] Task: Add 10 Save/Load State slots to the File menu with automated filename generation.
- [ ] Task: Implement optional Auto-Save on exit and Auto-Load on start functionality.
- [ ] Task: Conductor - User Manual Verification 'Phase 3: SRAM and State Management' (Protocol in workflow.md)

## Phase 4: State Browser and UI Refinement
- [ ] Task: Implement "State Browser" internal window to view, manage, and delete save states.
- [ ] Task: Finalize File menu organization, add dividers, and implement standard keyboard shortcuts (e.g., Ctrl+O, Ctrl+R).
- [ ] Task: Conductor - User Manual Verification 'Phase 4: State Browser and UI Refinement' (Protocol in workflow.md)
