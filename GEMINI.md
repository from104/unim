# Project: unim - A Korean Input Method Engine

## Project Overview

This project, `unim`, is a Korean Input Method Engine (IME). It is primarily written in Rust and designed to be a modular and extensible system for handling Korean character input.

The project has a multi-part architecture and a long-term cross-platform vision:

1.  **UNIM Core**: The core Rust library (`src/`) that contains the core Hangul composition logic.
2.  **GNOME Shell Extension**: A GNOME Shell extension providing native Linux integration for manual conversion via `<Super>k`.
3.  **Command-Line Interface (CLI)**: `unim-cli`, a standalone testing and interaction tool that powers the extension's conversion logic.
4.  **Ultimate Vision (Roadmap)**: Expanding into a **Tauri-based tray application** for Windows, macOS, and Linux to provide global autocorrect and automatic language status switching.

The core logic supports various Hangul input methods, including 2-bul and 3-bul standards.

## Building and Running

The project uses a `Makefile` to streamline the build and installation process.

### Key Commands

*   **Build the project:**
    ```bash
    make build
    ```
    This command compiles the `unim-cli` binary and prepares it for use within the GNOME extension.

*   **Install the GNOME Extension:**
    ```bash
    make install
    ```
    This copies the extension files and the `unim-cli` binary into the local user's GNOME Shell extensions directory.

*   **Enable the Extension:**
    ```bash
    make enable
    ```
    Activates the extension within GNOME Shell.

*   **Disable the Extension:**
    ```bash
    make disable
    ```

*   **Package for Distribution:**
    ```bash
    make pack
    ```
    This creates a distributable `.zip` file of the GNOME extension, including the bundled binary.

*   **View Logs for Debugging:**
    ```bash
    make log
    ```
    This command tails the GNOME Shell logs, which is essential for debugging the extension.

*   **Clean Build Artifacts:**
    ```bash
    make clean
    ```

## Development Conventions

*   The core logic is isolated in the root `src/` directory. Any changes to the fundamental input logic should be made there.
*   The GNOME extension communicates with the Rust engine by executing the `unim-cli` binary as a subprocess. This provides a stable and sandbox-friendly integration.
*   The `Makefile` is the source of truth for the standard build and installation process.
*   **문서 작성 언어**: Walkthrough, 계획(Implementation Plan), 작업 목록(Task) 등 문서는 **한글로 작성**합니다.
