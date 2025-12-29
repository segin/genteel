# AI Agent Integration (`AGENTS.md`)

This document outlines the role and capabilities of AI agents in the `genteel` project.

## Purpose

The primary purpose of integrating AI agents with the `genteel` emulator is to enable automated testing and exploration of Sega Mega Drive/Genesis software. By providing a machine-readable API to the emulator, we allow agents to:

-   **Play games**: The agent can send controller inputs to play through games, discovering bugs and unexpected behaviors.
-   **Perform targeted testing**: The agent can be instructed to test specific parts of a game, such as a particular level or menu.
-   **Fuzzing**: The agent can generate random inputs to test the robustness of the emulator and the game.
-   **Analyze game states**: The agent can read the emulator's memory to analyze the game's internal state and make decisions based on it.

## Agent API

The `genteel` emulator will expose a simple API for agents to interact with the emulation. This API is described in more detail in the `ARCHITECTURE.md` file.

## Usage

To use an AI agent with `genteel`, the agent will need to be able to call the public functions of the `genteel` library. The exact implementation of this will depend on the agent's capabilities.
