# UNIM Autocorrect - GNOME Shell Extension

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

### 3. Preferences UI
- **Toggle Extension**: Easily enable/disable the entire feature.
- **Layout Selection**: Choose your specific Korean and English keyboard layouts.
- **Custom Shortcut**: Define your preferred key combination (default is `<Super>k`).
- **Auto-Paste**: Toggle whether converted text should be automatically pasted back into the active field.

## 🛠️ How to Use

1.  **Select** the text you want to convert (highlight with mouse or keyboard).
2.  Press **`<Super>k`** (or your custom shortcut).
3.  The text will be converted and **automatically pasted** back into the field.

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
| `enable-extension` | boolean | true | Master toggle for the extension |
| `korean-layout` | string | '2bul' | '2bul', '390' or '391' |
| `english-layout` | string | 'qwerty' | 'qwerty' or 'dvorak' |
| `manual-conversion-shortcut` | strv | `['<Super>k']` | The shortcut to trigger conversion |
| `auto-paste` | boolean | true | Automatically paste after conversion |

## � Debugging

To view real-time logs for the extension:
```bash
make log
```

## 📋 License

This extension is part of the **unim** project.
