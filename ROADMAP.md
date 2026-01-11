# UNIM Project Roadmap

This document outlines the long-term goals and development phases of the **UNIM** project.

## 🎯 High-Level Objective
To create a unified, cross-platform (Windows, macOS, Linux) Korean Input Method Engine (IME) that handles automatic language status detection and manual text conversion.

## 🛣️ Development Phases

### Phase 1: Foundation & Linux Native (Completed)
- [x] Robust Rust core library for Hangul composition.
- [x] Portable `unim-cli` with embedded assets.
- [x] Native GNOME Shell extension using `St.Clipboard` and `Clutter`.
- [x] Hybrid architecture (CLI + Native APIs) for stability.

### Phase 2: Cross-Platform Bridge (Tauri Expansion)
- [ ] **Tauri Integration**: Design a background tray application using [Tauri](https://tauri.app/).
- [ ] **Global Shortcuts**: Implement cross-platform global shortcut listeners (using `tau-input` or similar Rust crates).
- [ ] **Clipboard Management**: Implement a secure, cross-platform clipboard handler in Rust to replace environment-specific APIs.
- [ ] **Windows Support**: Build and test `.exe` installers.
- [ ] **macOS Support**: Build and test `.app` bundles with appropriate permissions.

### Phase 3: Automatic Status Switching (Intelligence)
- [ ] **Context Detection**: Research methods to detect the current input field state or language context.
- [ ] **Auto-Correct Engine**: Implement real-time "mistyping" detection (e.g., typing `gksrmf` and automatically converting it to `한글` as you type).
- [ ] **User Learning**: Optional local dictionary to learn user-specific typing patterns.

### Phase 4: Modern IME Implementation
- [ ] **Input Context Integration**: Move from a "conversion tool" to a full-fledged IME provider (`ibus`, `fcitx5` for Linux, TSF for Windows).

---

## 💡 Why Tauri?
Tauri allows us to leverage our existing Rust core (`unim`) for the heavy lifting while provides a lightweight, secure web-based frontend for the settings UI and tray interactions. This dramatically reduces the memory footprint compared to Electron while maintaining the same level of cross-platform ease.
