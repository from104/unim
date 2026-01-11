# UNIM: Universal Next-generation Input Method

**UNIM** is an open-source Korean Input Method Engine (IME) written in Rust. It aims to provide a seamless, high-performance, and extensible typing experience for Korean and English users across all major platforms.

## 🚀 Ultimate Vision

The ultimate goal of UNIM is to become a **complete cross-platform solution** for Korean/English text processing and input, featuring:

1.  **Automatic Status Switching**: Intelligently detecting and switching between Korean and English modes based on context.
2.  **Universal Conversion**: Effortless transformation of mistyped text (English-to-Korean and vice-versa) via shortcuts.
3.  **Tauri-based Tray Application**: A lightweight, native-like experience on **Linux, Windows, and macOS** using a unified Rust core with a modern Tauri frontend.

## 🛠️ Current Status

Currently, the project is structured as follows:

### 1. [UNIM Core](file:///home/from104/work/unim/src/): The Heart of UNIM
Written in **Rust**, the core library handles all Hangul composition and decomposition logic (2-bul, 3-90, 3-91 standards). It is now designed to be zero-dependency and asset-embedded.

### 2. [unim-cli](file:///home/from104/work/unim/unim-cli/): Standalone Engine
A portable Command-Line Interface to the core logic. It can be used as a standalone converter or as a backend for other integrations.

### 3. [GNOME Shell Extension](file:///home/from104/work/unim/unim-gnome-extension/): Native Linux Integration
A production-ready extension for GNOME users that provides `<Super>k` conversion using native Shell APIs (`St.Clipboard`, `Clutter`).

## 🗺️ Long-term Roadmap

1.  **Phase 1 (Current)**: Stabilization of the Rust core and GNOME Shell extension.
2.  **Phase 2 (Expansion)**: Development of a **Tauri-based Tray Application** to provide global shortcut and clipboard support across Linux (X11/Wayland), Windows, and macOS.
3.  **Phase 3 (Intelligence)**: Implementation of context-aware automatic language detection.

## 📚 Examples

The project includes several examples in the `examples/` directory to help you get started with the UNIM library:

- **[Input Simulation (2-Set)](file:///home/from104/work/unim/examples/input_simulation_2bul.rs)**: See how the 2-set/2-bul standard handles real-time composition and "Dokkaebibul".
- **[Input Simulation (3-Set)](file:///home/from104/work/unim/examples/input_simulation_3bul.rs)**: Explore the logic behind 3-set/3-bul layout processing.
- **[Jamo Pattern Search](file:///home/from104/work/unim/examples/jamo_pattern_search.rs)**: An advanced example showing fuzzy search by decomposing text into Jamo components.
- **[String Processing](file:///home/from104/work/unim/examples/string_processing.rs)**: Basic deconstruction of Hangul syllables into their Initial, Middle, and Final components.
- **[Syllable Matrix](file:///home/from104/work/unim/examples/mk_hangul.rs)**: Generates the entire Hangul syllable range programmatically.

To run an example:
```bash
cargo run --example string_processing
```

---

For detailed installation and usage of the GNOME extension, see [unim-gnome-extension/README.md](file:///home/from104/work/unim/unim-gnome-extension/README.md).

For the long-term development plan, see [ROADMAP.md](file:///home/from104/work/unim/ROADMAP.md).
