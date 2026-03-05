# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

UNIM (Universal Next-generation Input Method) is a Korean IME written in Rust with a **3-layer architecture**: Core Engine (Rust) → DBus Daemon → Frontend IM Modules. It supports GTK3/4, Qt5/6, XIM, Wayland, and GNOME Shell.

## Build Commands

```bash
make build              # Full build (Rust workspace + C/C++ frontends via CMake)
make build-rust         # Rust workspace only (cargo build --release --workspace)
make build-frontends    # GTK3/4 + Qt5/6 IM modules only
cargo test --workspace  # Run all Rust unit tests
make build-tests        # Build all test applications
make deb                # Build Debian packages
```

Quick dev cycle (after initial `make build && sudo make install PREFIX=/usr`):
```bash
make dev-gtk4           # Rebuild + deploy GTK4 module
make dev-daemon         # Rebuild + deploy daemon
make dev-extension      # Deploy GNOME extension to ~/.local/share/
make sandbox-gtk4       # Test in Xephyr sandbox
```

## Architecture

```
User keypress → [IM Module (GTK/Qt/XIM/Wayland)] →DBus→ [unim-daemon] → [Core Engine (src/)]
                         ↑                                                        ↓
                         └──── DBus Signal (commit/preedit) ←────────────────────┘
```

- **DBus service**: `org.atit.unim.InputMethod` on session bus
- **Config file**: `~/.config/unim/config.yaml`
- **Debug log**: `~/.unim-errors.log` (active when `UNIM_DEVELOP=1`)

## Key Source Locations

| Area | Path | Language |
|------|------|----------|
| Core engine (hangul composition/decomposition) | `src/` | Rust |
| Hangul composers (2-set, 3-set layouts) | `src/hangul/` | Rust |
| Key processing & mode switching | `src/input_engine.rs` | Rust |
| Keyboard layout mappings | `src/keystroke/` | Rust |
| Config struct (source of truth for all settings) | `src/config.rs` | Rust |
| C-API FFI wrapper | `unim-capi/` | Rust |
| DBus daemon (worker thread architecture) | `unim-daemon/` | Rust |
| DBus service/client library | `unim-dbus/` | Rust |
| GTK3/4 IM modules | `unim-frontends/gtk3/`, `gtk4/` | C |
| GTK shared code (hanja popup etc.) | `unim-frontends/gtk-common/` | C |
| Qt5/6 IM plugins | `unim-frontends/qt5/`, `qt6/` | C++ |
| Qt shared code | `unim-frontends/qt-common/` | C++ |
| XIM frontend | `unim-frontends/xim/` | Rust |
| Wayland frontend | `unim-frontends/wayland/` | Rust |
| GUI tray/settings (GTK) | `unim-gui-gtk/` | Rust |
| GUI tray/settings (Qt6) | `unim-gui-qt/` | Rust (cxx-qt) |
| GNOME Shell extension | `unim-gnome-extension/` | JavaScript |

## Strict Quality Rules (Zero Tolerance)

- `cargo build --workspace` must produce **zero warnings**
- `cargo test --workspace` must have **all tests passing**
- `make build` (including C/C++ frontends) must complete warning-free
- Always run build and tests after code changes; fix any new warnings immediately

## Development Conventions

- **Makefile** is the source of truth for build/install processes
- Core logic is strictly isolated in `src/` — no UI or platform dependencies allowed there
- Frontends communicate with the engine only through DBus (via `unim-daemon`), never direct memory sharing
- External C/C++ access to the core must go through `unim-capi/` FFI layer
- Documentation, plans, and walkthroughs are written in **Korean**
- Git commit messages are written in **English** (e.g., `feat: Add Wayland popup support`)

## Logging

Use `unim_log!` macro (Rust), `unim_log_message()` (C/C++), or `unimLog()`/`unimError()` (JS). Do NOT use `log::*` crate, `println!`, or `console.log`. Logs activate only when `UNIM_DEVELOP=1` is set.

Format: `[YYYY/MM/DD HH:MM:SS] - [MODULE] - message`

Module names: `ENGINE`, `HANGUL`, `DAEMON`, `DBUS`, `XIM`, `WAYLAND`, `CLI`, `INDICATOR`, `GTK_IM`, `QT_IM`, `EXTENSION`

## Settings Synchronization

When adding/changing settings in `src/config.rs`, all these must be updated in sync:
1. `src/config.rs` — struct field (source of truth)
2. `unim-config/src/main.rs` — CLI ConfigKey enum
3. `unim-config/locales/*.yml` — translations
4. `unim-dbus/src/service.rs` — get_config/set_config methods
5. `unim-gui-gtk/src/gtk_ui.rs` — GTK UI widgets
6. `unim-gnome-extension/prefs.js` + `*.gschema.xml` — GNOME prefs

## Debugging

Set `UNIM_DEVELOP=1` to enable logging across all components. Clear `~/.unim-errors.log` before reproducing, then analyze the log. Run daemon in foreground for debugging:
```bash
UNIM_DEVELOP=1 target/debug/unim-daemon -n
```

## Reference Documents

- `AGENTS.md` — Full component map and architecture details
- `GEMINI.md` — Development conventions, settings guide, logging system
- `IME_BEHAVIOR.md` — Hangul input behavior specification (all frontends)
- `docs/POPUP_SPEC.md` — Hanja/special character popup design
- Each component has a `SPEC.md` in its directory
