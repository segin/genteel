# Specification: File Menu Enhancements and Automated Builds

## Overview
This track focuses on improving the user experience for file operations within the `genteel` emulator and automating the build process for Linux and Windows. Key additions include native file dialogs, a comprehensive File menu, SRAM/State management, and a CI/CD pipeline using GitHub Actions.

## Functional Requirements
1.  **File Menu Enhancements**:
    -   **Open ROM**: Use `rfd` to provide a native file dialog for selecting ROM files (.bin, .md, .gen, .zip).
    -   **Open Recent**: Maintain a list of recently opened ROMs in the configuration (serialized in `gui_config.json`).
    -   **Close ROM**: Unload the current ROM and return the emulator to a clean, idle state.
    -   **Reset ROM**: Perform a hard reset of the system while keeping the ROM loaded.
2.  **State and Save Management**:
    -   **SRAM Management**: Implement loading/saving of battery-backed RAM (SRAM) to persistent files (.srm). Provide menu options to import/export these.
    -   **Save/Load State**: Expose save and load state functionality in the File menu with support for multiple slots.
    -   **State Browser**: A dedicated window to view and delete existing save states (showing timestamps and potentially metadata).
    -   **Auto-Save/Load**: Add a setting to automatically save state on exit and reload the last state when opening the same ROM.
3.  **Automated Builds (CI/CD)**:
    -   Implement a GitHub Actions workflow to automatically build release binaries for Linux (x86_64) and Windows (x86_64) on every push to the `main` branch or when a new tag is created.
    -   Ensure the Windows build correctly bundles any necessary assets or uses static linking for dependencies.

## Non-Functional Requirements
-   **Native Experience**: The file dialog should feel native to the user's operating system.
-   **Reliability**: Automated builds must be verified to run on fresh installations of the target OS.
-   **Robustness**: Handling of missing or corrupted save/SRAM files should be graceful.

## Acceptance Criteria
-   The user can select a ROM via a native OS dialog.
-   The "File" menu contains: Open, Open Recent, Close, Reset, Save State, Load State, and Exit.
-   SRAM is automatically saved to disk when updated (or on exit) and reloaded on game start.
-   A GitHub Actions run successfully produces downloadable Linux and Windows artifacts.
-   Opening a new ROM resets all volatile system state (Z80 RAM, VRAM, Registers).

## Out of Scope
-   Netplay or cloud save synchronization.
-   Support for obscure ROM formats beyond standard Genesis formats.
-   Full implementation of Sega CD/32X save hardware.
