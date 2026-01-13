# UNIM TypeFIX - GNOME Shell Extension

A GNOME Shell extension for **seamless Korean-English text conversion** triggered by a single shortcut.

## 📋 Features

### 1. Hybrid Native Conversion
- **Direct Interaction**: Uses GNOME Shell's native APIs (`St.Clipboard` and `Clutter`) to read and write text.
- **No Manual Copy/Paste**: Simply highlight the text (Primary Selection) and press the shortcut. The extension handles reading, converting, and pasting automatically.
- **Standalone Engine**: Powered by a Rust-based `unim-cli` binary embedded with its own keymap assets, ensuring portability and reliability.

### 2. Broad Layout Support
**Korean:**
- 2-bul (Standard)
- 3-bul (390)
- 3-bul (391)

**English:**
- QWERTY
- Dvorak

### 3. Multiple Conversion Modes
- **Normal Mode**: Standard English-to-Korean and Korean-to-English conversion.
- **Terminal Mode**: Terminal-friendly conversion that sends backspaces before pasting.
- **Copy-only Mode**: Converts text and saves to clipboard without automatic pasting.

### 4. Preferences UI
- **Toggle Extension**: Easily enable/disable the entire feature.
- **Layout Selection**: Choose your specific Korean and English keyboard layouts.
- **Notification**: Toggle completion notifications.

## 🛠️ How to Use

1.  **Select** text (highlight with mouse or keyboard).
2.  Press the shortcut for your desired mode:
    - **`<Super>k`**: English → Korean
    - **`<Shift><Super>k`**: Korean → English
    - **`<Ctrl><Super>k`**: Terminal (E → K)
    - **`<Shift><Ctrl><Super>k`**: Terminal (K → E)
    - **`<Alt><Super>k`**: Copy only (E → K)
    - **`<Shift><Alt><Super>k`**: Copy only (K → E)
3.  The text will be converted and handled according to the mode.

## 🛠️ Installation

### Build
```bash
make build
```
This compiles the Rust CLI (`unim-cli`) and prepares the extension files.

### Install
```bash
make install
```
Installs the extension to your local GNOME Shell directory (`~/.local/share/gnome-shell/extensions/`).

### Enable
```bash
make enable
```
*Note: You may need to logout and log back in (or restart GNOME Shell on X11) for the changes to take effect.*

## 📝 Project Structure

```
unim-gnome-extension/
├── extension.js      # Core extension logic (Native API integration)
├── vkbd.js           # Native virtual keyboard wrapper (Clutter)
├── prefs.js          # Settings UI
├── metadata.json     # Extension metadata
├── bin/              # Standalone unim-cli binary
└── schemas/          # GSettings schema files
```

## 🔧 GSettings Configuration

| Key | Type | Default | Description |
|-----|------|---------|-------------|
| `korean-layout` | string | '2bul' | '2bul', '390', or '391' |
| `english-layout` | string | 'qwerty' | 'qwerty' or 'dvorak' |
| `shortcut-normal` | strv | `['<Super>k']` | English to Korean + Paste |
| `shortcut-normal-reverse` | strv | `['<Shift><Super>k']` | Korean to English + Paste |
| `shortcut-terminal` | strv | `['<Ctrl><Super>k']` | E to K + Backspace + Paste |
| `shortcut-terminal-reverse` | strv | `['<Shift><Ctrl><Super>k']` | K to E + Backspace + Paste |
| `shortcut-copy-only` | strv | `['<Alt><Super>k']` | E to K + Copy Only |
| `shortcut-copy-only-reverse` | strv | `['<Shift><Alt><Super>k']` | K to E + Copy Only |

## � Debugging

To view real-time logs for the extension:
```bash
make log
```

## 📋 License

This extension is part of the **unim** project.
