# Changelog

All notable changes to the UNIM (Universal Next-generation Input Method) project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.0.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added
- **UI Frontend Separation Preparation (Phase 3.5 Planning)**: Initiated plans to decouple the engine (daemon) and UI (popup/indicator/settings) based on DBus signals, enabling toolkit-specific native GUI support.
- **XIM and Wayland Protocol Reference Documentation**: Added comprehensive documentation referencing `input-method-v2` and `virtual-keyboard-v1` protocol specifications and architectural details.
- **Wayland Frontend Specifics**: Implemented foundational support for KDE Plasma environments utilizing the Wayland protocol.
- **XIM Compatibility Verification**: Completed protocol conformity verification against the X11R7.6 XIM specification (11 conformance items).

### Changed
- Refactored `unim-gui` tray icons and popups to synchronize immediately upon receiving the `GlobalModeChanged` signal from `unim-daemon`.

## [0.0.1] - 2024-03-XX (Initial Foundation)

### Added
- **Core Engine (`src/`)**: Pure Rust core library capable of handling all Hangul composition and decomposition logic (2-bul, 3-bul 390, 391 standards) without external dependencies.
- **DBus Daemon Architecture**: Centralized engine server (`unim-daemon`) and DBus library (`unim-dbus`) to manage system-wide input states.
- **C-API Wrapper (`unim-capi`)**: FFI binding to expose the Rust core engine to C/C++ frontends.
- **Standalone CLI (`unim-cli`)**: Portable command-line interface for the core logic, usable as an independent converter.
- **Configuration Tool (`unim-config`)**: CLI utility for managing engine settings.
- **GUI Settings and Popup (`unim-gui`)**: Centralized module for the system tray, Hanja/Special character popups, and settings.
- **GTK Frontends**: C-based IM modules for GTK3 (`unim-frontends/gtk3/`) and GTK4 (`unim-frontends/gtk4/`) with shared components (`unim-frontends/gtk-common/`).
- **Qt Frontends**: C++-based QPlatformInputContext plugins for Qt5 (`unim-frontends/qt5/`) and Qt6 (`unim-frontends/qt6/`) with shared components (`unim-frontends/qt-common/`).
- **XIM Frontend (`unim-frontends/xim/`)**: Native Rust-based X11 XIM protocol frontend with Over-The-Spot Preedit support.
- **Wayland Frontend (`unim-frontends/wayland/`)**: Native Rust-based Wayland frontend supporting `input-method-v2` and `virtual-keyboard-v1` protocols.
- **GNOME Shell Extension (`unim-gnome-extension/`)**: JavaScript extension for native integration, including layout conversion shortcuts (e.g., 'gksrmf' ↔ '한국어') and terminal-aware paste modes.
- **Comprehensive Documentation**: Included 12 component-specific `SPEC.md` files, `IME_BEHAVIOR.md` for consistent frontend behavior, and `POPUP_SPEC.md` for unified popup design.
