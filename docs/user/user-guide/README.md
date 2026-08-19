# UNIM User Manual (English)

> UNIM 0.4.0 — Universal Next-generation Input Method
> A Rust-based input method engine for fluid Korean/English typing.
> Goal of this document: get a first-time user to type one Korean syllable within five minutes.

---

## 1. What is UNIM (30-second pitch)

<!-- @platform:linux -->
- **IME** (Input Method Editor): the system component that translates alphabet keystrokes into Korean syllables and shows them in your application. Other examples: ibus-hangul, fcitx-hangul, kime, nimf.
<!-- @endplatform -->
<!-- @platform:windows -->
- **IME** (Input Method Editor): the system component that translates alphabet keystrokes into Korean syllables and shows them in your application. The Microsoft IME bundled with Windows is the obvious example; UNIM is an alternative you can use in its place.
<!-- @endplatform -->
<!-- @platform:linux -->
- **🐧 What sets UNIM apart on Linux**: a single Rust core plugged into five environments — GTK, Qt, XIM, Wayland, and the GNOME Shell. The exact same composition rules, hanja popup, and AutoTypeFix run regardless of whether you are typing in a terminal, a browser, a text editor, or an IDE.
<!-- @endplatform -->
<!-- @platform:windows -->
- **🪟 What sets UNIM apart on Windows**: the very same Rust core, plugged into Windows through **TSF**. Notepad, Chrome, Word, your terminal — the same composition rules, the same hanja popup, the same AutoTypeFix, and the same `config.yaml` format as the Linux build. Two copies of the input method are installed, one for 64-bit apps and one for 32-bit apps, so 32-bit-only programs such as KakaoTalk and Hancom Office work too.
<!-- @endplatform -->
- **30-second summary**: "Toggle Korean mode → type Korean → Hanja key for Chinese characters → ㄱㄴㄷ for special characters → automatic recovery when you accidentally typed in the wrong mode." That is the entire user-facing value proposition.

<!-- @platform:linux -->
> Acronyms expanded: **IM Module** (Input Method Module) — toolkit-specific adapter (one per GTK/Qt/etc) that lets an app delegate keystrokes to the IME. **DBus** (Desktop Bus) — the Linux desktop's inter-process communication bus; UNIM uses it to talk between the daemon and the frontends. **XIM** (X Input Method) — the oldest IME protocol, dating back to X11. **Wayland** — modern display protocol; IM handling differs from X11.
<!-- @endplatform -->
<!-- @platform:windows -->
> Acronyms expanded: **TSF** (Text Services Framework) — the standard input-method framework on Windows since XP. An input method is built as a COM component called a **TIP** (Text Input Processor) and registered with the OS; UNIM ships two of them, `unim_tsf.dll` for 64-bit apps and `unim_tsf32.dll` for 32-bit apps. **IMM32** (Input Method Manager 32) — the older, pre-TSF input API. Console-style apps (WezTerm, for instance) and apps that do their own input handling still go through it. **CUAS** (Cicero Unaware Application Support) — the Windows compatibility layer that relays those IMM32 calls to TSF. UNIM honors that contract, which is why Korean composition stays intact in apps like WezTerm and Telegram.
<!-- @endplatform -->

---

## 2. Quick Start (5 minutes)

### 2.1 Install

<!-- @platform:linux -->
> **🐧 Linux** — supported: **Ubuntu 24.04 (noble) or newer / equivalent Debian, amd64.** The release `.deb`s are built on noble.

#### Method 1 — automatic install script (recommended)

A single line downloads every UNIM `.deb` from GitHub Releases and installs them via `apt`. Every `.deb` is **SHA256-verified**, isolated in a `mktemp` working directory, and external runtime dependencies are resolved automatically. On any checksum mismatch it aborts without installing anything (no partial install).

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

To pin a specific version:

```bash
UNIM_VERSION=v0.4.0 curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

If you don't trust `curl | bash`, download the script first, read it, then run it:

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh -o install.sh
less install.sh && bash install.sh
```

#### Method 2 — manual download from Releases

Grab every `unim*_<version>-1_{amd64,all}.deb` plus `SHA256SUMS` from [Releases](https://github.com/from104/unim/releases) into the same directory, verify, and install.

```bash
# Verify checksums (e.g. 0.4.0-1 — 11 packages)
sha256sum -c SHA256SUMS

# Install (apt resolves dependencies automatically)
sudo apt install ./unim*.deb

# Remove IBus (avoids conflict on GNOME)
sudo apt remove ibus

# Register UNIM as the system IME (Debian/Ubuntu standard tool)
im-config -n unim
```

Log out and back in once so the new environment variables propagate to your shell.

#### Method 3 — from source (Arch/Fedora/others)

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
<!-- @endplatform -->

<!-- @platform:windows -->
> **🪟 Windows** — supported: **Windows 10 / 11 (64-bit).** A single administrator (UAC) approval is needed at install time.
>
> Windows support grew substantially in 0.4.0 on top of TSF, and it is used daily on the maintainer's own machine. It has not, however, been through the same breadth of hardware and application combinations as the Linux side. Please report anything you run into on [GitHub Issues](https://github.com/from104/unim/issues).

#### Method A — install script (recommended)

One line in PowerShell (or Windows Terminal):

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

This downloads the latest MSI (`unim-<version>-x64.msi`) from GitHub Releases, **SHA256-verifies it against `SHA256SUMS-msi`** (re-checked once more inside the elevated process, which substantially narrows the verify→install tamper window), and installs it via `msiexec`. On any checksum mismatch it aborts without installing anything.

```powershell
# Update (skips the download if already up to date)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/from104/unim/main/install.ps1))) -Update

# Check install state / latest version only (changes nothing)
& ([scriptblock]::Create((irm https://raw.githubusercontent.com/from104/unim/main/install.ps1))) -Check

# Pin a specific version (that release must ship SHA256SUMS-msi)
$env:UNIM_VERSION='v0.4.0'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

#### Method B — run the MSI yourself

Download `unim-<version>-x64.msi` from [Releases](https://github.com/from104/unim/releases) and double-click it. To check the download by hand, compare it against `SHA256SUMS-msi` from the same release.

```powershell
# Hash the downloaded MSI (must match the value in SHA256SUMS-msi)
Get-FileHash .\unim-0.4.0-x64.msi -Algorithm SHA256
```

> **SmartScreen warning**: the MSI is not code-signed yet, so you may see "Windows protected your PC". Choose **More info → Run anyway**.

#### What gets installed, and what to do next

Everything lands in `C:\Program Files\UNIM\`:

| File | Role |
|------|------|
| `unim_tsf.dll` | The input method itself, for 64-bit apps (TSF TIP) |
| `unim_tsf32.dll` | The same, for 32-bit apps — KakaoTalk, Hancom Office, etc. |
| `unim-settings.exe` | The settings window (see §5) |
| `unim-popup-win.exe` | Renderer for the hanja / special-character / emoji popups (starts at login) |
| `help\unim-help-en.html`, `help\unim-help-ko.html` | This offline manual |
| `register-tsf.bat`, `unregister-tsf.bat` | Helpers for re-registering the input method by hand |
| `LICENSE.txt`, `NOTICE.txt`, `LICENSES\` | License notices |

**Reboot (or log out and back in)** once the install finishes — apps that were already running need it to pick up the new input method. On that first login the **first-run wizard (`unim-settings`) opens automatically** and walks you through setting UNIM as your default input method. If you close it early it reappears at the next login; to reopen it by hand, launch **UNIM → UNIM Settings** from the Start menu, or run `"C:\Program Files\UNIM\unim-settings.exe" --first-run`.

The Start menu gains a **UNIM** folder with two entries:

- **UNIM Settings** — the settings window
- **Uninstall UNIM** — removal

#### Updating

The `-Update` one-liner above is the easiest route. Running a newer MSI directly works too — it is the same product, so **you do not need to uninstall the old version first** (it upgrades in place).

#### Uninstalling

Any of these three works; each removes the files, the Start-menu entries, and the input-method registration together.

- **Settings → Apps → Installed apps**, then remove UNIM
- Start menu → **UNIM → Uninstall UNIM**
- `msiexec /x` from an elevated PowerShell (rarely needed — the two options above figure out the product code for you)

> `unregister-tsf.bat` is **not an uninstaller.** It only unregisters the input method's COM entries; your files and settings stay put. For full removal, use one of the three methods above.
<!-- @endplatform -->

<!-- @platform:linux -->
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
<!-- @endplatform -->

<!-- @platform:windows -->
### 2.2 Windows — picking UNIM from the input-method list

Unlike Linux, Windows needs no environment variables. The MSI registers the input method with the OS, so after a reboot **UNIM shows up in your Korean input-method list.**

1. Click the language indicator near the clock (`한` / `A`, or `KOR`), or press **`Windows` + `Space`**.
2. Pick **UNIM** from the list.
3. If UNIM is not listed, open **Settings → Time & language → Language & region** and confirm **Korean** is installed. Add it if it is missing, then log out and back in.

> **Making UNIM your default**: if picking it every time gets old, use **Set as Default Input Method** — either in the first-run wizard (§5) or from the language-bar right-click menu.

### 2.3 Windows — the language bar (tray indicator)

Your current mode is shown as a letter near the clock — **`한`** in Korean mode, **`A`** in English mode.

- **Left-click** → toggle Korean/English
- **Right-click** → menu: **Switch Korean/English** · **Set as Default Input Method** · **Settings** · **Help** · **About**

> ⚠️ Easy to trip over: the settings window is on **right-click → Settings**. Left-click toggles the mode, it does not open settings.

> On Windows 11 the language bar appears as a tray icon, which may be tucked into the overflow area (the `^` button). To keep it visible, turn it on under **Settings → Personalization → Taskbar → Other system tray icons**.
<!-- @endplatform -->

### 2.4 First Korean keystroke (60 seconds)

<!-- @platform:linux -->
1. Open a text editor (e.g. GNOME Text Editor, Kate).
2. Press **Hangul** (or `Shift+Space`, depending on your keyboard) — the tray icon switches to "한".
3. Type `dkssud` → "안녕" appears.
4. Press **Hanja** (or F9) — a popup with `安寧` candidates appears. Pick with digits 1–9.
5. Press Hangul again to switch back to English mode.
<!-- @endplatform -->
<!-- @platform:windows -->
1. Open Notepad.
2. Press **Hangul** (or **right Alt**) — the language bar switches from `A` to "한".
3. Type `dkssud` → "안녕" appears.
4. Press **Hanja** (or F9) — a popup with `安寧` candidates appears. Pick with digits 1–9.
5. Press Hangul again to switch back to English mode.
<!-- @endplatform -->

If all five steps work, you are done. If not, head to [troubleshooting](../troubleshooting/README.md).

---

## 2.5 Popup behavior overview

<!-- @platform:linux -->
**🐧 Linux** — since UNIM 0.3.0, hanja, special-character, and emoji popups are rendered by a single service: **`unim-popup-service`**.

| Environment | Popup renderer | Notes |
|-------------|---------------|-------|
| GNOME Wayland | GNOME Extension `popup_view.js` (St widget) | Mutter does not support wlr-layer-shell; extension renders directly |
| GNOME X11 / KDE / Xfce / X11 WM | `unim-popup-service` GTK4 window | Auto-launched via D-Bus activation |
| Wayland (KDE Plasma 6 / Sway / Hyprland) | `unim-popup-service` GTK4 window (wayland-backend) | Requires `libgtk4-layer-shell` |

**Single source of truth**: regardless of environment, the daemon's `PopupRender` payload (cells, header, footer, tabs, highlight) is the single view-model delivered to all renderers. Only the rendering implementation differs.

**Outside-click dismiss**: clicking outside the popup closes it, and the click event is passed through to the window below. If a popup closes unexpectedly, this is intended behavior — see [troubleshooting](../troubleshooting/README.md).

**KDE Plasma 5.x Wayland — unsupported**: `gtk4-layer-shell` is not available in the Ubuntu 24.04 standard repository, so popups do not appear. Workaround: use an X11 session or switch to GNOME.

**KDE Plasma 6 Wayland / Sway / Hyprland / river — experimental, undertested**: builds with the `wayland-backend` cargo feature and `libgtk4-layer-shell` installed are theoretically functional, but this experimental status has **not changed since the 0.3.0 QA cycle** — no additional verification was done for v0.4.0. Expect possible regressions in popup placement, IME focus handover, and layer-shell coordinate translation. For the full per-environment support matrix, see [troubleshooting §G environment matrix](../troubleshooting/README.md#g-environment-matrix-reconfirmed-for-040--originally-written-for-030).
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — the input method itself (`unim_tsf.dll`) does not draw the hanja, special-character, or emoji popups. A **separate process, `unim-popup-win.exe`**, does. The input method only says *what* to show (candidates, header, footer, tabs, highlight) and the renderer draws it, which is why the popup looks and behaves the same no matter which app you summon it from.

- The renderer **starts automatically when you log in** (an autostart entry named `UnimPopupRenderer` is registered at install time). Seeing `unim-popup-win.exe` resident in Task Manager is normal.
- If it is not running for any reason, the input method relaunches it the moment a popup is first needed. So killing the process is recoverable — the next Hanja keypress brings it back.
- Stopping it only removes the popups; **Korean composition itself keeps working.**

> **Can I disable it in Task Manager?** Removing it from startup does no harm to typing, but the first Hanja keypress then has a short delay while the renderer launches. Leaving it alone is recommended.
<!-- @endplatform -->

---

## 3. Per-environment setup

<!-- @platform:linux -->
**🐧 Linux** — which path UNIM takes depends on your desktop environment and the app's toolkit.

| Environment | Install method | IM module | Popup owner | Watch out for |
|------|----------|---------|----------|--------|
| **X11 + GTK apps** | `GTK_IM_MODULE=unim` | gtk3/gtk4 IM module | `unim-popup-service` (GTK4, D-Bus auto-activation) | — |
| **X11 + Qt apps** | `QT_IM_MODULE=unim` | qt5/qt6 IM plugin | Same | On Plasma, prefer Qt mode |
| **X11 + legacy (Emacs, xterm)** | `XMODIFIERS=@im=unim` | xim frontend | XIM's own Xft popup | over-the-spot mode |
| **GNOME + Wayland** | Enable GNOME extension | (apps speak text-input-v3 directly) | GNOME Extension | IBus removal mandatory |
| **KDE + Wayland** | `QT_IM_MODULE=unim` + Wayland frontend | wayland | `unim-popup-service` (wayland-backend) | input-method-v2 |
| **Sway/Hyprland (Wayland)** | env vars + Wayland frontend | wayland | `unim-popup-service` (wayland-backend) | Compositor must support input-method-v2 |

> Detect your environment: `echo $XDG_SESSION_TYPE` (x11/wayland), `echo $XDG_CURRENT_DESKTOP` (GNOME/KDE/sway).

### 3.1 Flatpak/Snap apps that fail to type Korean

On GNOME+Wayland, Flatpak/Snap apps (Telegram, VS Code) have no UNIM IM module inside their sandbox. The host's `GTK_IM_MODULE=unim` actually blocks input.

**Automatic handling**: when `unim-daemon` detects GNOME+Wayland, it sets a Flatpak global override at startup that empties the IM environment variables, so Flatpak apps fall back to the Wayland text-input-v3 → GNOME Extension path.

**Manual override** (if the auto setup did not run):

```bash
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
```

Snap has no global override mechanism. Add a conditional snippet to `~/.profile` that empties the IM vars on GNOME+Wayland — see [README §1.7](../../../README.md).
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — there are no environment variables and no per-desktop setup. What varies is **which input API the app uses**, and that decides which part of UNIM handles it. All three cases are automatic; there is nothing for you to choose.

| App type | Examples | Handled by |
|----------|----------|-----------|
| **64-bit apps** (most of them) | Notepad, Chrome, Word, Windows Terminal | `unim_tsf.dll` (TSF) |
| **32-bit apps** | KakaoTalk, Hancom Office | `unim_tsf32.dll` (32-bit TSF) |
| **Console / IMM32-style apps** | WezTerm, Telegram | TSF via the CUAS compatibility layer |

> **Why two copies?** Windows loads a different input-method component for 64-bit and 32-bit applications. Getting Korean to work in 32-bit-only programs (KakaoTalk, Hancom, …) therefore requires registering a separate 32-bit input method. UNIM registers both at install time, so this is invisible to you.

> **The IMM32 `.ime` route is not used**: registering an input method the very old way (as an `.ime` file) became meaningless on Windows 11, so it was dropped in 0.4.0. Apps that still call the old IMM32 API reach UNIM through the third row above — the compatibility layer (CUAS) that Windows provides.

### 3.1 When Korean fails in one particular app

Apps handle input differently, so a problem can show up in just one of them. Narrow it down in this order.

1. **Does it work in Notepad?** If it does, UNIM itself is fine and the issue is with that specific app.
2. **Is the app 32-bit?** Open Task Manager → **Details** tab and look for a `(32 bit)` suffix on the process name. If it is 32-bit and not working, the 32-bit registration may have come undone — run `C:\Program Files\UNIM\register-tsf.bat` from an elevated command prompt, then log out and back in.
3. **Still stuck?** Check [troubleshooting](../troubleshooting/README.md), and if that does not help, report the app name and version on [GitHub Issues](https://github.com/from104/unim/issues). On Windows, compatibility varies from app to app, so per-app reports are especially useful.
<!-- @endplatform -->

---

## 4. Daily use

### 4.1 Korean/English mode toggle

<!-- @platform:linux -->
| Key | Action | Note |
|----|------|------|
| Hangul key | Toggle mode | Key code varies by keyboard |
| `Shift+Space` | Toggle (fallback) | Works on any keyboard |
| Right Alt (RightAlt) | Toggle mode (when added to `toggle_keys`) | Now works on every environment, including GTK/Qt/GNOME |
| Tray icon click | Toggle (mouse) | `unim-indicator` lives in the tray |
<!-- @endplatform -->
<!-- @platform:windows -->
| Key | Action | Note |
|----|------|------|
| Hangul key | Toggle mode | Key code varies by keyboard |
| Right Alt (RightAlt) | Toggle mode | On by default; handy on keyboards with no Hangul key |
| Language-bar left-click | Toggle (mouse) | The `한` / `A` indicator near the clock |
<!-- @endplatform -->

> **Mode share** (`mode_sharing` setting, CLI key `mode-sharing`): "Global" or "Per-app". **Global is the default** — switching mode in one window instantly syncs every window. Switch to "Per-app" if you want your terminal to stay in English while your text editor stays in Korean.

<!-- @platform:linux -->
> **Accessibility — turn on the toggle beep if you type without watching the screen**: Linux has no screen-reader announcement for mode switches, and the toggle beep itself **defaults to off** (a deliberate choice to avoid false positives). If you type without watching the screen, we recommend turning it on:
> ```bash
> unim-cli config set toggle-announce-beep true
> ```
> On and off are announced with different pitches.
>
> **Note: this beep can go silent depending on the mode-share setting.** The beep that fires from a tray-icon or GNOME-extension mode change only plays in "Global" mode (`unim-dbus/src/engine_worker.rs:1766-1772`). Switch to "Per-app" and a tray toggle no longer actually changes the focused window's mode (by design — it avoids sending a false signal), so this beep path goes silent. If you rely on the beep to confirm a language switch, be aware that switching to "Per-app" costs you this signal from tray/extension interactions.
<!-- @endplatform -->
<!-- @platform:windows -->
> **Accessibility — screen readers and the toggle sound**: on Windows, a Korean/English switch **is announced to screen readers (NVDA, Narrator)**, and the text you are composing along with the candidate window are exposed so a screen reader can read them.
>
> On top of that you can enable a **mode-switch sound** — a higher pitch for Korean, a lower one for English. It **defaults to off** (a deliberate choice to avoid false positives); turn it on under **General → Accessibility** in the settings window (§5). If you type without watching the screen, turning it on is recommended.
<!-- @endplatform -->

<!-- @platform:linux -->
> **Toggling with Right Alt**: Add `RightAlt` to the `toggle_keys` setting to switch Korean/English with the right Alt key. GTK, Qt, and the GNOME extension used to filter Right Alt themselves, so it did nothing there (XIM and pure Wayland already worked); the toggle decision is now unified in the daemon, so it behaves the same everywhere. AltGr layouts (right Alt used as AltGr) are unaffected. Note that at the moment of toggling the application may also receive the Alt input (e.g. menu-bar focus in some apps); if you don't want that side effect, remove `RightAlt` from `toggle_keys`.
<!-- @endplatform -->
<!-- @platform:windows -->
> **Toggling with Right Alt**: on keyboards without a Hangul key (many US-layout laptops), **right Alt** switches Korean/English. AltGr layouts (right Alt used as AltGr) are unaffected. Note that at the moment of toggling the application may also receive the Alt input (e.g. menu-bar focus in some apps); if you don't want that side effect, remove `RightAlt` from the `toggle_keys` list in your settings.
<!-- @endplatform -->

> **Key-name spelling**: `toggle_keys` takes key names UNIM knows, **one name per entry** (defaults: `Korean`, `RightAlt`). Unlike the AutoTypeFix hotkeys (§4.4), the toggle key does not accept modifier combinations (`Ctrl+X`). The CLI and the settings app warn about names they cannot parse when saving, and the daemon logs the parse failure — the value is still stored, but the dead key no longer disappears silently.

### 4.2 Hanja conversion

1. Type the Korean to convert (e.g. `한국`).
2. Press **Hanja** (or F9).
3. A 9-cell grid popup appears: `韓國`, `漢國`, …
4. Pick with digits 1–9, navigate with arrow keys, Enter to commit, ESC to cancel.
5. With more than nine candidates, press **`.` (period)** to toggle to a 9×9=81-cell expanded grid. The ⊞/⊟ icon in the corner reflects the current mode.

> **Choosing the Hanja key**: `hanja_keys` defaults to `Hanja`, `F9`. Like the toggle key (§4.1) it takes single key names only — no modifier combinations. The CLI and the settings app warn about names they cannot parse when saving, and the daemon logs the parse failure.

#### Page navigation (mouse / keyboard)

When candidates exceed one page (9 cells or 81 cells), the footer shows **◀ / ▶** buttons.

```text
[◀]  page 2 / 5  [▶]  ⊞
```

- **Mouse left-click** ◀ : previous page, ▶ : next page. Pressing ▶ on the last page wraps to the first; ◀ on the first page wraps to the last.
- **Keyboard** `←` / `Page Up` : previous page, `→` / `Page Down` : next page. Same wrap-around.
- If all candidates fit on a single page, the ◀/▶ buttons are **hidden** (avoids the "controls visible but inactive" confusion).
- The cursor's row/column is preserved across page changes — e.g. with the 81-cell grid, focusing cell (3, 4) and pressing ▶ lands you at cell (3, 4) of the next page.

> **Acronym**: *cursor* here means the highlighted cell currently holding keyboard focus (rendered as a background highlight).

##### Right-click semantics — frontend differences

<!-- @platform:linux -->
The ◀ / ▶ page buttons behave identically across every frontend, but **right-clicking on the grid body itself** carries different meaning depending on the frontend you are using. If you want to use it as a mouse shortcut, learn your environment's mapping; otherwise stick to the keyboard shortcuts which are uniform.
<!-- @endplatform -->
<!-- @platform:windows -->
Here is everything the mouse can do in the popup.
<!-- @endplatform -->

<!-- @platform:linux -->
- **GNOME Shell extension** (Wayland and X11): **toggle ★ bookmark** — equivalent to keyboard Space.
- **GTK IM modules** (gtk3 / gtk4): **next page** (wrap-around) — equivalent to `→` / Page Down.
- **Qt IM modules** (qt5 / qt6): **next page** (wrap-around).
- **XIM** (hanja / special-character / emoji popups alike): **next page** (wrap-around).
- **Other frontends** (`unim-popup-service` Standalone, raw Wayland): no action (undefined).

> **Why is GNOME different?** GNOME Shell exposes each candidate cell as a `Clutter.Actor`, so per-cell right-click hit-testing is natural — mapping right-click to the bookmark toggle saves a hand move. GTK/Qt IM modules and XIM run in X11/Wayland override-redirect windows where per-cell hit-testing is more limited, so they keep the classical IME convention of right-click = next page.
>
> **In short**: ◀ / ▶, keyboard `←` / `→`, and Space mean the same thing everywhere. Only the right-click on the grid body varies. If in doubt, the keyboard alone covers every action.
<!-- @endplatform -->
<!-- @platform:windows -->
On Windows a single process draws every popup (§2.5), so nothing varies from app to app.

- **Left-click**: clicking a cell commits that candidate. Clicking ◀ / ▶ in the footer changes page.
- **Right-click**: **does nothing at all** — deliberately. Clicking it by accident will not close the popup or commit a candidate.

> On Linux, right-clicking the grid means either "toggle bookmark" or "next page" depending on the desktop environment. Windows has no such split. Use **Space** to bookmark and **`←` / `→`** (or the ◀ / ▶ buttons) to change page.
<!-- @endplatform -->

#### Bookmarks ☆/★

You can star frequently used Hanja. With a candidate focused, **Space** toggles ☆ (unbookmarked) ↔ ★ (bookmarked).

<!-- @platform:linux -->
The `HanjaCandidatesReordered` DBus signal refreshes every open popup across GTK/Qt/XIM/Wayland/GNOME/Windows instantly.
<!-- @endplatform -->
<!-- @platform:windows -->
Every open popup refreshes instantly.
<!-- @endplatform -->

When you toggle, the candidate list reorders and the cursor follows the affected hanja:

- **★ on**: the hanja is *promoted* to the top of page 1 (the ★-group), and the cursor moves to that position (page 1, row 1).
- **☆ off**: the hanja is *demoted* back to its lexicographic home and the cursor jumps to that home position (often on a different page). The destination cell **flashes Catppuccin yellow `#f9e2af` for 140 ms** so you immediately notice "I unstarred this and it landed here".

> **Why is there flash on un-bookmark but not on bookmark?** Bookmarking always lands on page 1 row 1, which is a predictable, eye-catching location — no extra hint needed. Un-bookmarking can jump to *any* page, so the flash is what tells you where the candidate went.

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
3. In the GUI's "Suppression Words" page, select an entry and press **Activate Permanently** to promote Tentative → **Confirmed**. After 4 hours without a retrigger (the default; adjustable between 1 and 12), Tentative auto-flips to **Inactive**.

<!-- @platform:linux -->
**Storage**: `~/.config/unim/typefix-blacklist.yaml`. The daemon hot-reloads on mtime change.
<!-- @endplatform -->
<!-- @platform:windows -->
**Storage**: `%APPDATA%\unim\typefix-blacklist.yaml`. Changes to the file are picked up automatically a moment later.
<!-- @endplatform -->

> **User dictionary (reverse whitelist)** — new in 0.2.0. Select text and use a shortcut to call the `RegisterUserDictFromSelection` DBus method, registering an English-side entry. Manage entries in the GUI's "User Dictionary" page.

#### Toggle hotkeys — turn correction on/off with a single key

A shortcut can turn AutoTypeFix on or off instantly. **The master toggle defaults to `Shift+F8`**; the forward- and reverse-only shortcuts are empty (assign them only if you want them). Three of them, set separately:

- **Master toggle**: turns all of AutoTypeFix (the master switch) on/off.
- **Forward toggle** (a.k.a. "정방향" in some Korean labels): turns only forward (English→Korean) correction on/off.
- **Reverse toggle**: turns only reverse (Korean→English) correction on/off.

<!-- @platform:linux -->
Set them from the CLI:

```bash
# The default (master toggle = Shift+F8)
unim-cli config set auto-typefix-toggle-keys "Shift+F8"

# Forward / reverse on F10 and F11 respectively
unim-cli config set auto-typefix-forward-toggle-keys F10
unim-cli config set auto-typefix-reverse-toggle-keys F11

# Multiple keys, comma-separated (any of them toggles)
unim-cli config set auto-typefix-toggle-keys "Shift+F8,Ctrl+Left"

# Clear — an empty value consumes no key
unim-cli config set auto-typefix-toggle-keys ""
```

In the GTK settings GUI the three fields are not grouped together but distributed across each feature group — the **Master toggle** sits in the "Type Correction" master group (next to the overall on/off switch), the **Forward toggle** in the "Forward" group, and the **Reverse toggle** in the "Reverse" group. Type a key name into each field; leave one empty to disable it.
<!-- @endplatform -->
<!-- @platform:windows -->
Set them in the settings window — **Settings (§5) → Type Correction → "Toggle hotkeys"**, which holds all three fields. Separate multiple keys with commas (`Shift+F8,Ctrl+Left`); leave a field empty to disable that shortcut.
<!-- @endplatform -->

In the Slint settings app (`unim-settings`, including Windows) the three fields are gathered side by side in the **"Toggle hotkeys"** group on the **Type Correction** page (5.2).

> - **Modifier combinations are supported** — write them as `Shift+F8`, `Ctrl+Left`, `Ctrl+Shift+F7` (`Ctrl`/`Control`, `Alt`, `Super`/`Win`/`Meta`, `Shift`; case- and order-insensitive). A bare name like `F10` fires on that key alone, as before. `+` is the canonical separator; when a spec contains no `+` at all, `-` is accepted too (`Ctrl-F8` = `Ctrl+F8`). Mixing them (`Ctrl+Shift-F8`) is invalid.
> - **It fires only on an exact modifier match.** Combinations you did not configure (e.g. `Shift+F10` for the context menu) are not intercepted and reach the application unchanged.
> - The default is `Shift+F8` — F9–F11 are avoided because some keyboards lose them to media functions or remapping (cut/copy/paste) before they reach the OS. It does not clash with the hanja/emoji key (bare `F9`), and an F9 combo like `Shift+F9` leaves the bare-`F9` popup intact.
> - Key names must be ones UNIM knows, such as `F1`–`F12` (`ScrollLock`, `Pause`, `PrintScreen`, and `Menu` are not recognized). The CLI and the settings app warn about names they cannot parse when saving, and the daemon logs the parse failure — nothing is dropped silently.
> - Clear the list (`""`) to disable a shortcut; it then consumes no key.
> - On GNOME (Wayland), Ctrl/Alt/Super combinations such as `Ctrl+Left` work too — the UNIM extension forwards toggle combos to the engine (re-login after updating the extension).
> - On Windows (TSF), combination specs behave exactly as on Linux — the default `Shift+F8` works as-is.
> - Toggling only forward on while the master switch is off flips the flag but produces no correction until the master switch is on again (the master gates everything). The forward/reverse toggles change only each direction's flag.
> - Accessibility note: with `toggle-announce-beep` enabled, this toggle also announces its state with a differential beep (rising pitch = on / falling pitch = off), just like the Korean/English switch sound — **on both Windows and Linux** (on Linux the daemon plays the tone via whichever of `paplay`/`pw-cat`/`aplay` it finds on PATH; if none exist, it's a silent no-op). Default is off — see the accessibility note in [4.1](#41-koreanenglish-mode-toggle).

#### Password-field auto-protection

In password and PIN fields, AutoTypeFix **turns off automatically.** When the app reports "this field is a password" (`content_purpose`), UNIM stops both forward and reverse correction while you are in that field, and also clears any keystroke-observation buffer and undo history already accumulated. This keeps a password typed like `dkssud` from being auto-corrected into Korean and corrupted.

- It returns to normal the moment you leave the field. Any on/off state you set manually is preserved — password protection is only a temporary safety layer laid on top of it.
<!-- @platform:linux -->
- This protection works when the app reports the field as a password. Some environments do not report it (legacy XIM apps, and some Wayland compositors/web forms that do not send content-purpose), so auto-detection may fail there → see [FAQ](../faq/README.md) Q9.
- **Reports are wanted.** Which applications report password fields correctly, and which do not, is not yet well mapped — on Linux and Windows alike — so per-application handling is incomplete. If you find an app where correction fires inside a password field, report it with the **app name and version** on [GitHub Issues](https://github.com/from104/unim/issues) and it can be added to the handled list.
<!-- @endplatform -->
<!-- @platform:windows -->
- This protection works when the app reports the field as a password. Some apps do not report it (programs that handle input themselves, and some web forms that do not mark the field as a password), so auto-detection may fail there → see [FAQ](../faq/README.md) Q9.
- **Reports are wanted.** Which applications report password fields correctly, and which do not, is not yet well mapped — on Windows and Linux alike — so per-application handling is incomplete. If you find an app where correction fires inside a password field, report it with the **app name and version** on [GitHub Issues](https://github.com/from104/unim/issues) and it can be added to the handled list.
<!-- @endplatform -->

### 4.5 Auto-English-Mode

Opt-in feature for vim command mode (`Esc`), CLI slash commands (`/`), etc. Off by default.

- Enable: GUI → "General" → "Auto-English-Mode" group → toggle ON.
- Trigger keys: defaults to `Escape`, `Slash`. Add virtual names like `ShiftSemicolon` (:) or `ShiftSlash` (?) if you need them.
- Behavior: in Korean mode, pressing a trigger key (1) commits the current preedit, (2) permanently switches to English mode, (3) forwards the trigger key itself to the application.

> If your toggle key collides with a trigger key, the toggle wins (its branch comes first in `press_key`). Password fields are unaffected (they force English already).

> **Trigger spelling**: write triggers as `key:<key name>` or `char:<character>` (e.g. `key:Escape`, `char:/`). The older prefix-less form (`Escape`) is still recognized, and modifier combinations such as `key:Ctrl+B` are allowed. The CLI and the settings app warn about specs they cannot parse when saving, and the daemon logs the parse failure — nothing is dropped silently.

### 4.6 Word-unit commit (composition-commit granularity)

By default, UNIM commits **per syllable** — a syllable is finalized as soon as it is complete. Set this to **word unit** instead and composition accumulates as underlined preedit through a word boundary (space, punctuation) before committing all at once. `BackSpace` during composition still steps back one jamo at a time, and this meshes more naturally with AutoTypeFix's reverse correction (4.4).

There are three values.

- **Syllable**: always commit per syllable. Most predictable.
- **Word**: accumulate per word in every target app.
- **Smart (default)**: word-unit only in apps listed in `word-mode-apps`, syllable-unit everywhere else. The default list is just `winword.exe` (Windows), so on Linux, with nothing added, this is effectively **syllable-unit with no regression**.

> **This does not apply to English input — by design.** The commit unit governs **Hangul
> composition** only. So when you mix the two, only the Hangul stretches are underlined and
> English commits the moment you type it. Uneven underlining is that, not a broken setting.
>
> The underline under Hangul shows a **composition in progress** — `ㅎ+ㅏ+ㄴ` becoming `한`.
> English has nothing to compose, since one key is already one letter, and so nothing to show.
> Underlining English too would make the application treat those letters as "not yet committed",
> which **stops autocomplete, spell check, and live search, and breaks single-letter shortcuts**
> (keys like `j` or `t` in browsers and editors). It costs a lot and gains nothing, so it is not done.

**Turning it on**

<!-- @platform:linux -->
```bash
# Global word-unit
unim-cli config set commit-unit word

# Smart + specific apps only (e.g. LibreOffice)
unim-cli config set commit-unit smart
unim-cli config set word-mode-apps "winword.exe,soffice"
```

In the settings GUI, pick it from the **Korean commit unit** combo under General → Layout options. `word-mode-apps` is edited via CLI/`config.yaml` (exact match, case-insensitive). Example Linux app ID: LibreOffice is `soffice` — app IDs can be checked in the log when Mode share is set to Per-app.
<!-- @endplatform -->
<!-- @platform:windows -->
Pick it from the **Korean commit unit** combo under **General → Input mode** in the settings window (§5).

With **Smart** (the default), word-unit applies **only in MS Word (`winword.exe`)** and every other app stays syllable-unit. To add another app, put its **executable name** in the "Word-mode apps" list (exact match, case-insensitive). You can find the executable name in Task Manager → **Details** tab.

```yaml
# %APPDATA%\unim\config.yaml — safe to edit by hand; it is picked up shortly after saving
korean:
  commit_unit: Smart          # Syllable / Word / Smart — capitalized
  word_mode_apps:
    - winword.exe
```
<!-- @endplatform -->

**When it doesn't apply (safety net)**

<!-- @platform:linux -->
- **Terminals** (ghostty, kitty, wezterm, alacritty, foot, gnome-terminal, konsole, xterm, etc.): preedit is fragile there, so it's always syllable-unit.
- **XIM** (legacy xterm-family apps): structurally cannot support word-unit — always syllable-unit.
- **Pure Wayland / Flatpak/Snap (ibus)**: app identification isn't available yet, so these are currently excluded (syllable-unit).
- **With moachigi (chord input) enabled**: incompatible with word-unit, so it always falls back to syllable-unit.
<!-- @endplatform -->
<!-- @platform:windows -->
- **Apps not in the "Word-mode apps" list** (while on Smart): syllable-unit. In other words, out of the box nothing but MS Word is affected.
- **With moachigi (chord input) enabled**: incompatible with word-unit, so it always falls back to syllable-unit.
<!-- @endplatform -->

> In word-unit mode, AutoTypeFix's reverse correction (Korean→English) only replaces the composition — it never touches already-committed text. In all of the excluded cases above, UNIM automatically falls back to syllable-unit, so it's safe to leave word-unit turned on without worrying about data loss.

---

## 5. Settings GUI Tour

**`unim-settings`** (Slint, a single codebase shared by Linux and Windows) is the one entry point for settings. That means the four pages below are **identical on both operating systems** — only the way you open the window differs.

<!-- @platform:linux -->
**🐧 Opening it on Linux** — the "UNIM Settings" app-menu item, the first-run wizard, and the tray menu all launch this same executable. You can also start it from a terminal.

```bash
unim-settings &
```

> **About the legacy GTK dialog (`unim-settings-gtk`)**: the old GTK4+libadwaita settings window is still shipped in the package, but it is **no longer exposed in the app menu** (`NoDisplay=true` in its `.desktop` file). You can still launch it directly with `unim-settings-gtk &`, but new settings may not be reflected there — treat `unim-settings` above as the source of truth. The Qt dialog (`unim-gui-qt`) has already been retired; the tray icon is now owned by `unim-indicator`, and the hanja/special-char/emoji popups are owned by `unim-popup-service`, each running as its own process (see §2.5).
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Opening it on Windows** — three ways, whichever suits you.

| Route | Steps |
|-------|-------|
| **A. Language bar** (recommended) | **Right-click** the `한` / `A` indicator near the clock → **Settings** |
| **B. Start menu** | Start menu → **UNIM** folder → **UNIM Settings** |
| **C. Windows language settings** | Settings → Time & language → Language & region → Korean → keyboard options → UNIM |

> ⚠️ Left-clicking the language bar **toggles Korean/English**. Settings is on **right-click → Settings**.
>
> To launch it directly: `C:\Program Files\UNIM\unim-settings.exe`.

> **There is no Save button**: changes apply and are written to disk immediately. Just close the window when you're done.
>
> The **[Help]** button at the bottom of the sidebar opens this manual in your default browser.
<!-- @endplatform -->

Four pages (left-hand navigation):

### 5.1 Page 1 — General

<!-- screenshot: settings-general -->

| Group | Widget | Note |
|------|------|---------|
| **Layout options** | Korean layout / English layout | e.g. `ko_2bulstd` (Dubeolsik standard) — see §7.1. Layout-specific dynamic options (e.g. Sebeolsik 390's "sun-arae batchim") also appear here |
|  | Korean commit unit | `Syllable` / `Word` / `Smart` — see [4.6](#46-word-unit-commit-composition-commit-granularity) |
| **Input mode** | Initial mode | Korean or English when the daemon starts |
|  | Mode share | `Global` (default) / `Per-app` — see [4.1](#41-koreanenglish-mode-toggle) |
|  | Per-app rules | Add rules so specific apps (matched by window/client-name substring) always start in a given mode. Most useful when Mode share = Per-app |
| **Auto-English-Mode** | Enable switch + trigger keys | Off by default — see [4.5](#45-auto-english-mode) |
| **Accessibility** | One-click presets ("One-hand use" / "Relaxed timing") + individual switches | See below |

> **Accessibility presets**: "One-hand use" applies the Sebeolsik-noshift layout + a non-modifier toggle key + moachigi off + auto-repeat suppression, all at once. "Relaxed timing" applies auto-repeat suppression + a wider typo-correction detection window + (on layouts that support it) a wider moachigi chord window. Either preset can still be fine-tuned afterward with the individual switches.
>

<!-- @platform:linux -->
> **Suppress Composition Key Auto-repeat (accessibility)**: When you hold a key down, the OS re-fires it rapidly (auto-repeat); enabling this option makes the daemon ignore those repeats. It is meant for users with motor disabilities who tend to hold keys too long (e.g. tremor), and it is **now enforced by the daemon on both Windows and Linux** (Linux enforcement was previously missing; fixed in v0.4.0). Suppression applies to the **Korean/English toggle key and character keys in Korean mode**; repeats of editing keys (Backspace, arrows) and direct English typing are left alone. Wayland, Qt5/6, and the GNOME extension detect repeats precisely; the GTK3/4, XIM, and ibus-compatible paths approximate with an 80 ms time window, so the first repeat may slip through and, if your system key-repeat interval is set longer than 80 ms, repeats may not be filtered (in either case it errs toward suppressing less, fail-safe). The default is off; you can also enable it with `unim-cli config set ignore-key-repeat true`. GNOME extension users: applies after re-login.
>
> **Emoji input** has no separate switch — it shares the hanja-popup path and is always on; call it with `Super+.` (or whatever shortcut you registered) — see the [keyboard shortcuts guide](../keyboard-shortcuts/README.md#emoji-popup-shortcut-super).
>
> **GNOME-extension-only settings** (whether the panel indicator is shown, the manual conversion shortcuts, etc.) live in `gnome-extensions prefs unim-gnome@from104.github.io`, not in this app — see the [keyboard shortcuts guide's GNOME section](../keyboard-shortcuts/README.md).
<!-- @endplatform -->
<!-- @platform:windows -->
> **Suppress Composition Key Auto-repeat (accessibility)**: When you hold a key down, the OS re-fires it rapidly (auto-repeat); enabling this option makes the input method ignore those repeats. It is meant for users with motor disabilities who tend to hold keys too long (e.g. tremor), and it works **the same way as on Linux**. Suppression applies to the **Korean/English toggle key and character keys in Korean mode**; repeats of editing keys (Backspace, arrows) and direct English typing are left alone. The default is off.
>
> **Emoji input** has no separate switch — it shares the hanja-popup path and is always on. Press the **Hanja key (or F9)** while *not* composing and the emoji popup appears. The same key thus serves three purposes depending on context: **hanja, special characters, and emoji**.
<!-- @endplatform -->

### 5.2 Page 2 — Type Correction

<!-- screenshot: settings-typefix -->

| Group | Widget | Note |
|------|------|------|
| **Enable** | Enable AutoTypeFix | Master switch. OFF stops both forward and reverse |
| **Correction strength** | Conservative / Standard / Aggressive presets | Tunes thresholds, minimum word length, and detection window all at once. The "Advanced settings" section below still lets you fine-tune individual values |
| **Direction** | Enable forward / Enable reverse | Each can be toggled independently — see 4.4 |
| **Toggle hotkeys** | Master on/off, Forward on/off, Reverse on/off (three fields, one group) | Toggle instantly with the assigned key. Leave empty to disable. Modifier combos like `Shift+F9` are allowed — see 4.4 |
| **Advanced settings** (collapsible) | Korean-syllable threshold / English minimum word length / forward & reverse detection windows (ms) / Tentative expiry (hours) / Observation timeout (sec) — all sliders | The "Correction strength" presets above cover most cases |
| **Options** (inside Advanced) | Skip English words / Skip complete syllables / Rollback detection / User-dictionary only | — |
| — | Restore defaults | Resets AutoTypeFix settings to their initial values (undo within 5 seconds) |

### 5.3 Page 3 — Suppression Words

<!-- screenshot: settings-blacklist -->

A single list shows **Tentative**, **Confirmed**, and **Inactive** suppressions together (distinguished by a status badge, unlike the old GTK dialog's three separate sections). Select an entry and choose **Confirm** (promotes to Confirmed) or **Delete**; **Clear all** empties the whole list (undo within 5 seconds).

> Even if the daemon updates the file, the GUI refreshes immediately. No manual reload needed.

### 5.4 Page 4 — User Dictionary (reverse whitelist)

<!-- screenshot: settings-userdict -->

Enter a **word** and an optional **note**, then **Add**, to register an English ↔ Korean-jamo-sequence mapping. E.g. `wave` ↔ `ㅈㅐㅍㅁ`. Select an entry below and **Delete** to remove it. Reverse correction prefers user-dict entries.

---

<!-- @platform:windows -->
## 5.6 Keyboard-layout tools (Keymap Studio / Typing Practice)

**🪟 Not included on Windows yet.** The layout viewer/editor (Keymap Studio) and the typing-practice tool ship only with the Linux packages for now. The Windows installer (MSI) contains just the input method, the settings window, and the popup renderer.

**Choosing** a layout works the same on Windows — it's under General → Layout options in the settings window. What is missing is only the tooling for authoring new layouts and for typing practice.

<!-- @endplatform -->

<!-- @platform:linux -->
## 5.6 Keyboard-layout tools (Keymap Studio / Typing Practice)

Separate from the settings GUI, two GTK4 companion tools ship with UNIM for viewing, editing,
and practicing layouts. Each registers in the app list with its own icon
(`io.github.from104.unim.KeymapStudio`, `io.github.from104.unim.TypingPractice`).

```bash
unim-keymap-studio &      # view / edit layouts
unim-typing-practice &    # typing practice
```

### 5.6.1 unim-keymap-studio — view / edit layouts

<!-- screenshot: keymap-studio -->

See which jamo/characters each key produces for Korean/English layouts, and build your own.

- **Three-stage header dropdown**: narrow down by "language > source > layout". E.g. Korean >
  User > `my_3bul_variant`.
- **Four tabs**: Basic (metadata) · Keymap (per-key mapping grid) · Combos (jamo-combination
  rules) · Extended (rule_set toggles). The **Combos** and **Extended** tabs appear **only for
  Korean layouts** (English layouts have no jamo-combination concept).
- **Header-right buttons**: [Help] (F1) · [Settings] · [Menu].
- **Save policy**: built-in layouts are read-only, so only "Save As" is offered. User layouts
  support both "Save" and "Save As". A new user layout is written as JSON under
  `~/.config/unim/layouts/`, which the daemon auto-scans so it shows up in the settings GUI's
  layout list.

#### Shortcuts

| Key | Action |
| --- | ------ |
| F1 | Help |
| Ctrl + N | New layout |
| Ctrl + D | Duplicate current layout |
| Ctrl + S | Save (user layouts) |
| Ctrl + Shift + S | Save As |
| Ctrl + E | Export |
| Ctrl + I | Import |
| Ctrl + 1 / 2 / 3 / 4 | Switch tab (Basic / Keymap / Combos / Extended) |

### 5.6.2 unim-typing-practice — typing practice

<!-- screenshot: typing-practice -->

Practice typing with the currently **active layout** (the one the daemon uses). It auto-reloads
when you switch layouts.

- **Metrics**: WPM (words per minute), CPM (characters per minute), accuracy, and a **typo
  heatmap** that colors the on-screen keyboard to show which keys you mistype most.
- **Practice material**: built-in words/sentences, plus import from a file (Ctrl+O) or the
  clipboard (Ctrl+Shift+V).
- Shares the **same five-row keyboard widget** as Keymap Studio, so the layout looks consistent.

#### Shortcuts

| Key | Action |
| --- | ------ |
| F1 | Help |
| Ctrl + R | Restart |
| Ctrl + Shift + C | Copy results |
| Ctrl + 1 | Practice view |
| Ctrl + 2 | Results view |
| Ctrl + O | Import material from file |
| Ctrl + Shift + V | Import material from clipboard |
<!-- @endplatform -->

---

## 6. Key cheat sheet

<!-- @platform:linux -->
**🐧 Linux**

| Situation | Key | Result |
|------|----|------|
| Anywhere | Hangul (or Shift+Space) | Toggle mode |
| Korean mode, after typed jamos | Hanja (F9) | Hanja popup |
| Hanja popup | 1–9 | Direct select |
| Hanja popup | Arrows | Move focus |
| Hanja popup | `←`/`→` or PageUp/PageDown | Page navigation (wrap-around) |
| Hanja popup | Mouse ◀ / ▶ | Page navigation (wrap-around, hidden on single page) |
| Hanja popup | Mouse right-click | **Frontend-specific**: GNOME = toggle ★ bookmark / GTK·Qt IM·XIM = next page / others = no action (see §4.2) |
| Hanja popup | Enter | Commit focused |
| Hanja popup | ESC | Cancel |
| Hanja popup | `.` | 9 ↔ 81 grid toggle |
| Hanja popup | Space | Bookmark ☆/★ (un-bookmark flashes destination cell 140 ms) |
| Korean mode, lone consonant | Hanja (F9) | Special-char popup |
| Composing | BackSpace | Delete last jamo |
| After unwanted forward correction | BS + Hangul | Trigger Tentative learning |
| With Auto-English on | `Esc` or `/` | Force English + pass key |
<!-- @endplatform -->
<!-- @platform:windows -->
**🪟 Windows**

| Situation | Key | Result |
|------|----|------|
| Anywhere | Hangul (or right Alt) | Toggle mode |
| Korean mode, after typed jamos | Hanja (F9) | Hanja popup |
| Korean mode, lone consonant | Hanja (F9) | Special-char popup |
| Not composing | Hanja (F9) | Emoji popup |
| Hanja popup | 1–9 | Direct select |
| Hanja popup | Arrows | Move focus |
| Hanja popup | `←`/`→` or PageUp/PageDown | Page navigation (wrap-around) |
| Hanja popup | Mouse ◀ / ▶ | Page navigation (wrap-around, hidden on single page) |
| Hanja popup | Mouse left-click (a candidate cell) | Select that candidate |
| Hanja popup | Mouse left-click (outside the popup) | Dismiss — the click passes through to the window below |
| Hanja popup | **Mouse right-click** | **No action** (deliberate — it neither dismisses the popup nor commits a candidate) |
| Hanja popup | Enter | Commit focused |
| Hanja popup | ESC | Cancel |
| Hanja popup | `.` | 9 ↔ 81 grid toggle |
| Hanja popup | Space | Bookmark ☆/★ (un-bookmark flashes destination cell 140 ms) |
| Composing | BackSpace | Delete last jamo |
| After unwanted forward correction | BS + Hangul | Trigger Tentative learning |
| With Auto-English on | `Esc` or `/` | Force English + pass key |
<!-- @endplatform -->

---

<!-- @platform:windows -->
## 7. Changing settings by hand

The Windows build does not ship a command-line tool (`unim-cli`). The **settings window (§5)** is the supported way to change settings; edit the config file directly only for the few things the window does not expose.

### 7.1 Supported layouts

Pick one under **General → Layout options** in the settings window.

- Korean: Dubeolsik standard (`ko_2bulstd`), Sebeolsik 390, Sebeolsik 391, Sebeolsik no-shift
- English: QWERTY, Dvorak, Colemak, Colemak-DH, Workman

### 7.2 Editing the config file

Open `%APPDATA%\unim\config.yaml` in Notepad or any text editor. Pasting `%APPDATA%\unim` into the Explorer address bar takes you straight to the folder.

```powershell
# Open the settings folder in Explorer
explorer "$env:APPDATA\unim"

# Or open the file directly in Notepad
notepad "$env:APPDATA\unim\config.yaml"
```

Saving is enough — the change **is picked up shortly afterwards**, with no reboot and no restarting the input method.

> ⚠️ In YAML, indentation is syntax. Use **spaces, never tabs**, and keep a copy of the file before you edit it. If the file becomes unparseable, UNIM falls back to its default settings.
<!-- @endplatform -->

<!-- @platform:linux -->
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
# Show the full current config (or grep for a specific key)
unim-cli config show

# Set a value — key names take kebab-case (hyphens) only
unim-cli config set auto-typefix true
unim-cli config set auto-typefix-tentative-expiry-hours 6
unim-cli config set auto-english true

# Korean commit unit (syllable/word/smart) — see 4.6
unim-cli config set commit-unit word

# AutoTypeFix toggle hotkeys (comma-separated; modifier combos allowed) — see 4.4
unim-cli config set auto-typefix-toggle-keys "Shift+F8"
unim-cli config set auto-typefix-forward-toggle-keys F10
unim-cli config set auto-typefix-reverse-toggle-keys F11

# Layout profile management
unim-cli config layout list                    # built-in + user profiles
unim-cli config layout describe ko_3bul390     # profile details
unim-cli config layout validate my.json        # validate a custom layout
```

> Setting changes apply to the daemon immediately. config.yaml ↔ `unim-cli` ↔ the settings GUI (`unim-settings`) are kept in sync at all three points by design.
<!-- @endplatform -->

---

## 8. Config files / backup

<!-- @platform:linux -->
| File | Purpose | Back up? |
|------|------|----------|
| `~/.config/unim/config.yaml` | General settings | YES |
| `~/.config/unim/typefix-blacklist.yaml` | Learned suppressions | YES |
| `~/.config/unim/typefix-userdict.yaml` | Reverse user dict | YES |
| `~/.config/unim/layouts/*.json` | Custom v1 layouts | YES |
| `~/.unim-errors.log` | Debug log (`UNIM_DEVELOP=1`) | NO |

```bash
tar -czf unim-backup-$(date +%F).tar.gz -C ~/.config unim
tar -xzf unim-backup-2026-04-26.tar.gz -C ~/.config
systemctl --user restart unim-daemon
```
<!-- @endplatform -->

<!-- @platform:windows -->
Everything lives under **`%APPDATA%\unim\`** — usually `C:\Users\<you>\AppData\Roaming\unim\`. `AppData` is a hidden folder, so pasting `%APPDATA%\unim` into the Explorer address bar is the quickest way there.

| File | Purpose | Back up? |
|------|------|----------|
| `config.yaml` | General settings (layout, mode, type correction, …) | YES |
| `typefix-blacklist.yaml` | Learned suppressions | YES |
| `typefix-userdict.yaml` | Reverse user dictionary | YES |

```powershell
# Back up everything to a dated zip on your Desktop
Compress-Archive -Path "$env:APPDATA\unim" `
  -DestinationPath "$env:USERPROFILE\Desktop\unim-backup-$(Get-Date -Format yyyy-MM-dd).zip"

# Restore — overwrites the existing files
Expand-Archive -Path "$env:USERPROFILE\Desktop\unim-backup-2026-07-27.zip" `
  -DestinationPath $env:APPDATA -Force
```

> **Log out and back in** after a restore to be certain everything took effect. Individual file edits are picked up automatically, but a bulk restore is safer with a fresh login.

> **Uninstalling UNIM leaves this folder behind.** To wipe your settings completely, delete `%APPDATA%\unim` after uninstalling. Conversely, leaving it in place means a reinstall picks your settings right back up.
<!-- @endplatform -->

---

## 9. Next steps

- Something off → [troubleshooting](../troubleshooting/README.md)
- Compare with other IMEs / migration → [FAQ](../faq/README.md)
- Per-release changes → [changelog](../../../CHANGELOG.md)
- Want to contribute → [`CONTRIBUTING.md`](../../../CONTRIBUTING.md)
- Behavior spec → [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md), [`POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)

---

Doc version: 0.4.0 / 2026-08-01 / License: same as the project.
