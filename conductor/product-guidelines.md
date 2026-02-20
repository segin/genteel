# Product Guidelines: Genteel

## Documentation Style
*   **Tone**: Professional, technical, and concise. Documentation should focus on "how it works" and "how to use it" with clear, direct language.
*   **Clarity**: Avoid jargon where possible, but maintain precision when describing hardware behavior (e.g., specific CPU flag behavior).

## Branding and Visual Design
*   **Aesthetic**: Modern and clean. The debugger GUI should use standard modern UI elements (via \`egui\`) with a focus on legibility and clean typography.
*   **Themes**: Support for light and dark modes, defaulting to a high-contrast dark theme suitable for long debugging sessions.

## User Experience (UX) Principles
*   **Discoverable UI**: All features should be easily accessible through well-organized menus and tooltips.
*   **Keyboard Accelerators**: Provide comprehensive keyboard shortcuts for all common debugging actions (Step, Run, Pause, Reset) to enable efficient "hands-on-keys" workflow.
*   **Real-time Interaction**: Ensure the GUI is responsive and provides immediate feedback during emulation, even when running at high speeds.
