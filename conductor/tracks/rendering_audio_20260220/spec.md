# Track Specification: Rendering and Audio Expansion

## Overview
The goal of this track is to fix the "white screen" issue by implementing the VDP background rendering logic and to improve audio output by implementing missing channels in the APU (PSG and YM2612).

## Objectives
1.  **VDP Background Rendering**: Implement the logic to render Plane A and Plane B tiles.
2.  **PSG Audio**: Implement all four PSG channels (3 square wave, 1 noise).
3.  **YM2612 Audio**: Expand YM2612 implementation to support more channels and operators.

## Success Criteria
*   The emulator renders game backgrounds instead of a solid white screen.
*   Audio output includes multiple channels from both the PSG and YM2612.
*   All new functionality is covered by unit tests.
