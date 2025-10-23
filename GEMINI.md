# Project: unim - A Korean Input Method Engine

## Project Overview

This project, `unim`, is a Korean Input Method Engine (IME). It is primarily written in Rust and designed to be a modular and extensible system for handling Korean character input.

The project has a multi-part architecture:

1.  **`unim-core`**: A Rust crate (`unim-core`) that contains the core Hangul composition logic. It is compiled as a C-style dynamic library (`.so`), allowing it to be used by other languages, particularly the JavaScript environment of the GNOME Shell extension.
2.  **GNOME Shell Extension**: A GNOME Shell extension located in `unim-gnome-extension/`. This component is written in JavaScript and provides the front-end integration with the desktop environment. It calls the functions exported by the `unim-core` library to perform the actual input composition.
3.  **Command-Line Interface (CLI)**: A command-line tool, `unim-cli`, for testing and interacting with the core library directly.

The core logic supports various Hangul input methods, including 2-bul and 3-bul standards.

## Building and Running

The project uses a `Makefile` to streamline the build and installation process.

### Key Commands

*   **Build the project:**
    ```bash
    make build
    ```
    This command compiles the `unim-core` Rust library and copies the resulting `libunim_core.so` file into the `unim-gnome-extension/lib/` directory, making it available to the GNOME extension.

*   **Install the GNOME Extension:**
    ```bash
    make install
    ```
    This copies the extension files from `unim-gnome-extension/` into the local user's GNOME Shell extensions directory.

*   **Enable the Extension:**
    ```bash
    make enable
    ```
    Activates the extension within GNOME Shell. You may need to restart the shell (Alt+F2, then `r`, then Enter on X11) for it to take effect.

*   **Disable the Extension:**
    ```bash
    make disable
    ```

*   **Package for Distribution:**
    ```bash
    make pack
    ```
    This creates a distributable `.zip` file of the GNOME extension.

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

*   The core logic is isolated in the `unim-core` crate. Any changes to the fundamental input logic should be made there.
*   The `unim-core` crate is built as a `cdylib`, meaning it exposes a C-compatible Application Binary Interface (ABI). Functions intended to be called from the JavaScript extension must be marked with `#[no_mangle]` and use C-compatible types.
*   The GNOME extension communicates with the Rust library by loading the `libunim_core.so` shared object.
*   The `Makefile` is the source of truth for the standard build and installation process.
