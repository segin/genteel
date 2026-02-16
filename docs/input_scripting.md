# Input Scripting & TAS System

Genteel features a frame-accurate input scripting system designed for high-fidelity reproduction of gameplay, automated regression testing, and Tool-Assisted Speedruns (TAS).

## Overview

The system allows developers and players to define exact button states for every frame of emulation. It ensures determinism by injecting inputs directly into the virtual I/O ports, bypassing operating system keyboard delays and frontend mapping overhead.

## Script Format

Scripts use a line-based CSV format with support for comments and sparse frame definitions.

**File Extension:** `.txt` or `.csv` (typically referenced via the `--script` flag)

### Syntax
`frame,p1_buttons,p2_buttons`

*   **frame**: A 64-bit unsigned integer representing the emulation frame (starting at 0).
*   **p1_buttons**: Button state string for Player 1.
*   **p2_buttons**: Button state string for Player 2 (optional, defaults to all released).

### Button String Format
The button string represents the physical state of the Genesis controller.

| Position | 3-Button Controller | 6-Button Controller | Char (Pressed) | Char (Released) |
| :------- | :------------------ | :------------------ | :------------- | :-------------- |
| 1        | Up                  | Up                  | `U`            | `.`             |
| 2        | Down                | Down                | `D`            | `.`             |
| 3        | Left                | Left                | `L`            | `.`             |
| 4        | Right               | Right               | `R`            | `.`             |
| 5        | A                   | A                   | `A`            | `.`             |
| 6        | B                   | B                   | `B`            | `.`             |
| 7        | C                   | C                   | `C`            | `.`             |
| 8        | Start               | Start               | `S`            | `.`             |
| 9        | -                   | X                   | `X`            | `.`             |
| 10       | -                   | Y                   | `Y`            | `.`             |
| 11       | -                   | Z                   | `Z`            | `.`             |
| 12       | -                   | Mode                | `M`            | `.`             |

### Example Script
```text
# This is a comment
# frame, p1, p2
0,........,........    # Frame 0: Both players neutral
60,....A...,........   # Frame 60: Player 1 presses A
65,........,........   # Frame 65: Player 1 releases A
120,U...B...,....S...  # Frame 120: P1 presses Up+B, P2 presses Start
```

## Behavior & Determinism

### Input Holding (Sparse Scripts)
If a frame is not explicitly defined in the script, the `InputManager` will **hold the state of the last defined frame**. This allows for concise scripts where only changes in input need to be recorded.

### Frame Latching
Inputs are latched at the beginning of each emulation frame (during the VBlank interval). This ensures that the M68k and Z80 CPUs see a consistent state throughout the frame's execution.

### Deterministic Playback
Because the emulator uses a discrete frame-based loop, running the same script against the same ROM starting from a power-on state will always result in the exact same internal machine state.

## Usage

### Playing a Script
To run the emulator with a TAS script:
```bash
genteel --script my_tas.txt <ROM_PATH>
```

### Headless Verification
For automated testing or brute-forcing segments without the overhead of rendering:
```bash
genteel --script my_tas.txt --headless 1000 <ROM_PATH>
```
This will execute 1000 frames at maximum speed and then terminate.

## Keyboard Mapping (Player 1)

When running in interactive mode (GUI), the following default keyboard mappings are used for Player 1:

| Genesis Button | Keyboard Key |
| :------------- | :----------- |
| **D-pad Up**   | Arrow Up     |
| **D-pad Down** | Arrow Down   |
| **D-pad Left** | Arrow Left   |
| **D-pad Right**| Arrow Right  |
| **A**          | Z            |
| **B**          | X            |
| **C**          | C            |
| **Start**      | Enter        |
| **X**          | A            |
| **Y**          | S            |
| **Z**          | D            |
| **Mode**       | Q            |

*Note: Live keyboard input is combined with script playback if both are active. Keyboard input uses physical key codes (layout-independent).*

## Internal API (For Developers)

The system is implemented across two primary modules:
1.  `src/input.rs`: Contains `InputScript` (parsing/storage) and `InputManager` (state tracking/playback).
2.  `src/io/mod.rs`: Defines `ControllerState`, which receives the injected inputs and handles the hardware-level 3-button and 6-button multiplexing protocols.

### Recording
The `InputManager` includes a recording API (`start_recording`, `record`, `stop_recording`) that can be used to generate scripts from live play sessions, though this is currently not exposed via CLI flags.
