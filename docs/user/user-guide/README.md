# UNIM User Manual (English)

> UNIM 0.2.0 — Universal Next-generation Input Method
> A Rust-based input method engine for fluid Korean/English typing.
> Goal of this document: get a first-time user to type one Korean syllable within five minutes.

---

## 1. What is UNIM (30-second pitch)

- **IME** (Input Method Editor): the system component that translates alphabet keystrokes into Korean syllables and shows them in your application. Other examples: ibus-hangul, fcitx-hangul, kime, nimf.
- **What sets UNIM apart**: a single Rust core plugged into five environments — GTK, Qt, XIM, Wayland, and the GNOME Shell. The exact same composition rules, hanja popup, and AutoTypeFix run regardless of whether you are typing in a terminal, a browser, a text editor, or an IDE.
- **30-second summary**: "Toggle Korean mode → type Korean → Hanja key for Chinese characters → ㄱㄴㄷ for special characters → automatic recovery when you accidentally typed in the wrong mode." That is the entire user-facing value proposition.

> Acronyms expanded: **IM Module** (Input Method Module) — toolkit-specific adapter (one per GTK/Qt/etc) that lets an app delegate keystrokes to the IME. **DBus** (Desktop Bus) — the Linux desktop's inter-process communication bus; UNIM uses it to talk between the daemon and the frontends. **XIM** (X Input Method) — the oldest IME protocol, dating back to X11. **Wayland** — modern display protocol; IM handling differs from X11.

---

## 2. Quick Start (5 minutes)

### 2.1 Install

#### Debian/Ubuntu

```bash
# Install the .deb packages built for UNIM 0.2.0
sudo apt install ./unim_0.2.0_amd64.deb \
                 ./unim-common_0.2.0_amd64.deb \
                 ./unim-im-gtk_0.2.0_amd64.deb \
                 ./unim-im-qt_0.2.0_amd64.deb \
                 ./unim-gui-gtk_0.2.0_amd64.deb

# Remove IBus (avoids conflict on GNOME)
sudo apt remove ibus

# Register UNIM as the system IME (Debian/Ubuntu standard tool)
im-config -n unim
```

Log out and back in once so the new environment variables propagate to your shell.

#### From source (Arch/Fedora/others)

```bash
git clone https://github.com/from104/unim.git
cd unim
make build           # Builds Rust workspace + GTK3/4 + Qt5/6 IM modules in one shot
sudo make install PREFIX=/usr
sudo make install-systemd PREFIX=/usr
systemctl --user daemon-reload
systemctl --user enable --now unim-daemon.service
```

Source builds need `cargo` 1.95+, GTK4/libadwaita headers, and Qt5/Qt6 dev packages. Package names vary by distro; see [troubleshooting/build-failure](../troubleshooting/README.md#build-failure).

### 2.2 Environment variables (any desktop without GNOME extension)

For KDE Plasma, XFCE, Sway, Hyprland, etc., add three lines to `~/.xprofile` or `/etc/environment`:

```bash
export GTK_IM_MODULE=unim
export QT_IM_MODULE=unim
export XMODIFIERS="@im=unim"
```

Log out and back in. On Debian-family distros, `im-config -n unim` does the same in one command.

### 2.3 GNOME + Wayland users

On GNOME Shell, the `unim-gnome-extension` is responsible for key interception and popup rendering. You enable the extension instead of setting environment variables.

```bash
gnome-extensions enable unim-gnome@from104.github.io
```

Disable or remove IBus to avoid conflicts:

```bash
sudo apt remove ibus
```

### 2.4 First Korean keystroke (60 seconds)

1. Open a text editor (e.g. GNOME Text Editor, Kate).
2. Press **Hangul** (or `Shift+Space`, depending on your keyboard) — the tray icon switches to "한".
3. Type `dkssud` → "안녕" appears.
4. Press **Hanja** (or F9) — a popup with `安寧` candidates appears. Pick with digits 1–9.
5. Press Hangul again to switch back to English mode.

If all five steps work, you are done. If not, head to [troubleshooting](../troubleshooting/README.md).

---

## 3. Per-environment setup

| Environment | Install method | IM module | Popup owner | Watch out for |
|------|----------|---------|----------|--------|
| **X11 + GTK apps** | `GTK_IM_MODULE=unim` | gtk3/gtk4 IM module | IM module itself (Embedded) or unim-gui-gtk (Standalone) | Pick via `popup_mode` setting |
| **X11 + Qt apps** | `QT_IM_MODULE=unim` | qt5/qt6 IM plugin | Same | On Plasma, prefer Qt mode |
| **X11 + legacy (Emacs, xterm)** | `XMODIFIERS=@im=unim` | xim frontend | XIM's own Xft popup | over-the-spot mode |
| **GNOME + Wayland** | Enable GNOME extension | (apps speak text-input-v3 directly) | GNOME Extension | IBus removal mandatory |
| **KDE + Wayland** | `QT_IM_MODULE=unim` + Wayland frontend | wayland | unim-gui-gtk Standalone | input-method-v2 |
| **Sway/Hyprland (Wayland)** | env vars + Wayland frontend | wayland | unim-gui-gtk Standalone | Compositor must support input-method-v2 |

> Detect your environment: `echo $XDG_SESSION_TYPE` (x11/wayland), `echo $XDG_CURRENT_DESKTOP` (GNOME/KDE/sway).

### 3.1 Flatpak/Snap apps that fail to type Korean

On GNOME+Wayland, Flatpak/Snap apps (Telegram, VS Code) have no UNIM IM module inside their sandbox. The host's `GTK_IM_MODULE=unim` actually blocks input.

**Automatic handling**: when `unim-daemon` detects GNOME+Wayland, it sets a Flatpak global override at startup that empties the IM environment variables, so Flatpak apps fall back to the Wayland text-input-v3 → GNOME Extension path.

**Manual override** (if the auto setup did not run):

```bash
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
```

Snap has no global override mechanism. Add a conditional snippet to `~/.profile` that empties the IM vars on GNOME+Wayland — see [README §1.7](../../../README.md).

---

## 4. Daily use

### 4.1 Korean/English mode toggle

| Key | Action | Note |
|----|------|------|
| Hangul key | Toggle mode | Key code varies by keyboard |
| `Shift+Space` | Toggle (fallback) | Works on any keyboard |
| Tray icon click | Toggle (mouse) | unim-gui-gtk lives in the tray |

> **Mode share** (`mode_share_mode` setting): "per-window" or "global". Per-window is the default — your terminal stays in English while your text editor stays in Korean. Switch to "global" if you want a single mode across every window.

### 4.2 Hanja conversion

1. Type the Korean to convert (e.g. `한국`).
2. Press **Hanja** (or F9).
3. A 9-cell grid popup appears: `韓國`, `漢國`, …
4. Pick with digits 1–9, navigate with arrow keys, Enter to commit, ESC to cancel.
5. With more than nine candidates, press **`.` (period)** to toggle to a 9×9=81-cell expanded grid. The ⊞/⊟ icon in the corner reflects the current mode.

> **Bookmarks**: you can star frequently used Hanja. With the candidate focused, **Space** toggles ☆ ↔ ★. The `HanjaBookmarkChanged` DBus signal refreshes every open popup across GTK/Qt/XIM/Wayland/GNOME instantly.

### 4.3 Special characters

In Korean mode, type a single jamo (consonant) and then press the Hanja key. The category depends on the consonant.

| Jamo | Category | Examples |
|------|---------|------|
| ㄱ | Symbols | `!`, `@`, `÷`, `≠`, `∞` |
| ㄴ | Brackets | `「」`, `『』`, `≪≫` |
| ㄷ | Math | `∂`, `∇`, `√`, `∫` |
| ㄹ | Units | `＄`, `％`, `℃`, `Å` |
| ㅁ | Shapes | `■`, `□`, `●`, `○` |
| ㅂ | Lines | `─`, `│`, `┌`, `┐` |
| ㅅ | Hangul jamo | `ㄱ`, `ㄴ`, `ㅏ` |
| ㅇ | Circled | `①`, `ⓐ` |
| ㅈ | Parenthesized Hangul | `㈀`, `㈁` |
| ㅊ/ㅋ | Parenthesized digits | `⑴`, `⑵` |
| ㅌ | Parenthesized letters | `⒜`, `⒝` |
| ㅍ | Greek | `Α`, `β`, `γ` |
| ㅎ | Misc | `●`, `♨`, `☏` |

Example: type `ㅁ`, press Hanja, pick `2` from the shapes grid → `□` is committed.

### 4.4 AutoTypeFix

Auto-recovers text typed in the wrong mode. Two directions:

- **Forward (English→Korean)**: you thought you were in Korean mode but were actually in English, so `gksrmf` came out — replaced with `한글` at word boundaries (space/punctuation).
- **Reverse (Korean→English)**: opposite — `ㅈㅐㅍㅁ` becomes `wave`.

#### Suppression dictionary (Blacklist) — user learning

When a particular word keeps getting corrected against your wishes:

1. Press `BackSpace` to undo the correction and switch modes — UNIM marks the word as "Pending".
2. The next time the same word triggers AutoTypeFix, the attempt is suppressed and the word is registered as **Tentative**.
3. In the GUI's "Suppression Words" page, **Confirm** promotes Tentative → **Confirmed** (permanent). After 1 hour without a retrigger, Tentative auto-flips to **Inactive**.

**Storage**: `~/.config/unim/typefix-blacklist.yaml`. The daemon hot-reloads on mtime change.

> **User dictionary (reverse whitelist)** — new in 0.2.0. Select text and use a shortcut to call the `RegisterUserDictFromSelection` DBus method, registering an English-side entry. Manage entries in the GUI's "User Dictionary" page.

### 4.5 Auto-English-Mode

Opt-in feature for vim command mode (`Esc`), CLI slash commands (`/`), etc. Off by default.

- Enable: GUI → "General" → "Auto-English-Mode" group → toggle ON.
- Trigger keys: defaults to `Escape`, `Slash`. Add virtual names like `ShiftSemicolon` (:) or `ShiftSlash` (?) if you need them.
- Behavior: in Korean mode, pressing a trigger key (1) commits the current preedit, (2) permanently switches to English mode, (3) forwards the trigger key itself to the application.

> If your toggle key collides with a trigger key, the toggle wins (its branch comes first in `press_key`). Password fields are unaffected (they force English already).

---

## 5. Settings GUI Tour

Two GUIs ship: `unim-gui-gtk` (GTK4 + libadwaita) or `unim-gui-qt` (Qt6 cxx-qt). GTK is the default.

```bash
unim-gtk-settings &
unim-qt-settings &     # alternative
```

Five pages (GTK):

### 5.1 Page 1 — General

<!-- screenshot: settings-general -->

| Group | Widget | Recommended |
|------|------|---------|
| **Layouts & keymaps** | Korean layout (ComboRow) | `ko_2bulstd` (Dubeolsik standard) |
|  | English layout (ComboRow) | `qwerty` |
|  | Emoji input (Switch) | ON — type aliases like `:smile:` |
| **Korean layout options** | Dynamic SwitchRows | Reconfigured when layout changes. E.g. `ko_3bul390` exposes the `sun_arae_batchim` toggle |
| **Input mode** | Initial mode (ComboRow) | Korean / English on daemon start |
|  | Mode share (ComboRow) | `per-window` (recommended) / `global` |
|  | Popup mode (ComboRow) | `Standalone` (default) / `Embedded` (X11 only) |
| **Auto-English-Mode** | Enable (Switch) | OFF (default). Vim users may want ON |

### 5.2 Page 2 — Type Correction

<!-- screenshot: settings-typefix -->

| Group | Widget | Meaning |
|------|------|------|
| **Common** | Enabled (Switch) | Master toggle |
|  | Rollback detection (Switch) | Auto-learn on BS+mode-switch. ON by default |
|  | Observation window (sec) (Slider) | Default 10, range 5–15 |
|  | Tentative expiry (h) (Slider) | Default 1, range 1–12 |
| **Forward (en→ko)** | Enable (Switch) | `gksrmf`→`한글` |
|  | Skip in English mode (Switch) | ON recommended |
| **Reverse (ko→en)** | Enable (Switch) | `ㅈㅐㅍㅁ`→`wave` |
|  | Skip incomplete syllables (Switch) | ON recommended |
|  | User-dictionary only (Switch) | OFF: also use built-in mappings |

### 5.3 Page 3 — Suppression Words

<!-- screenshot: settings-blacklist -->

Three sections: **Tentative** / **Confirmed** / **Inactive**, each with [Confirm]/[Deactivate]/[Remove]/[Reactivate] row buttons.

> Even if the daemon updates the file, the GUI polls mtime every 2 s and refreshes automatically. No manual reload.

### 5.4 Page 4 — User Dictionary (reverse whitelist)

<!-- screenshot: settings-userdict -->

Map English ↔ Korean jamo sequences directly. E.g. `wave` ↔ `ㅈㅐㅍㅁ`. Reverse correction prefers user-dict entries.

### 5.5 Page 5 — GNOME Shell

<!-- screenshot: settings-gnome -->

Visible only in a GNOME session. Indicator and key-interception options for the extension.

---

## 6. Key cheat sheet

| Situation | Key | Result |
|------|----|------|
| Anywhere | Hangul (or Shift+Space) | Toggle mode |
| Korean mode, after typed jamos | Hanja (F9) | Hanja popup |
| Hanja popup | 1–9 | Direct select |
| Hanja popup | Arrows | Move focus |
| Hanja popup | Enter | Commit focused |
| Hanja popup | ESC | Cancel |
| Hanja popup | `.` | 9 ↔ 81 grid toggle |
| Hanja popup | Space | Bookmark ☆/★ |
| Korean mode, lone consonant | Hanja (F9) | Special-char popup |
| Composing | BackSpace | Delete last jamo |
| After unwanted forward correction | BS + Hangul | Trigger Tentative learning |
| With Auto-English on | `Esc` or `/` | Force English + pass key |

---

## 7. CLI usage (`unim-cli`)

Two purposes: (1) Korean↔English conversion filter, (2) settings management.

### 7.1 Conversion filter

```bash
echo "dkssudgktpdy" | unim-cli                # English → Korean (default)
echo "안녕하세요"   | unim-cli -d              # Korean → English
echo "ekswn"       | unim-cli -k 2bul         # Dubeolsik (default)
echo "j;ax"        | unim-cli -k 390          # Sebeolsik 390
unim-cli -o out.txt input.txt                 # File I/O
```

Supported layouts:
- Korean: `2bul`, `390`, `391`, `noshift`
- English: `qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`

### 7.2 Settings management

```bash
unim-cli config list
unim-cli config get auto_typefix.enabled
unim-cli config set auto_typefix.tentative_expiry_hours 6
unim-cli config set engine.auto_english.enabled true

unim-cli config layout list
unim-cli config layout describe ko_3bul390
unim-cli config layout validate my.json
```

> Setting changes apply to the daemon immediately. config.yaml ↔ `unim-cli` ↔ GTK GUI are synchronized at three points by design.

---

## 8. Config files / backup

| File | Purpose | Back up? |
|------|------|----------|
| `~/.config/unim/config.yaml` | General settings | YES |
| `~/.config/unim/typefix-blacklist.yaml` | Learned suppressions | YES |
| `~/.config/unim/userdict.yaml` | Reverse user dict | YES |
| `~/.config/unim/layouts/*.json` | Custom v1 layouts | YES |
| `~/.unim-errors.log` | Debug log (`UNIM_DEVELOP=1`) | NO |

```bash
tar -czf unim-backup-$(date +%F).tar.gz -C ~/.config unim
tar -xzf unim-backup-2026-04-26.tar.gz -C ~/.config
systemctl --user restart unim-daemon
```

---

## 9. Next steps

- Something off → [troubleshooting](../troubleshooting/README.md)
- Compare with other IMEs / migration → [FAQ](../faq/README.md)
- 0.2.0 changes / migration → [release notes](../release-notes/0.2.0/RELEASE_NOTES.md)
- Want to contribute → [`CONTRIBUTING.md`](../../../CONTRIBUTING.md)
- Behavior spec → [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md), [`POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)

---

Doc version: 0.2.0 / 2026-04-26 / License: same as the project.
