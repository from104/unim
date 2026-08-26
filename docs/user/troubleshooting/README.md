# UNIM Troubleshooting (English)

> UNIM 0.4.0 — organized as Symptom → first diagnosis → second-level command → fix.
> Covers 14 commonly seen symptoms, from "Korean never types" to "broken in one specific app".

<!-- @platform:linux -->
**🐧 Linux**

Every diagnosis starts with two questions: **is the daemon alive?** and **what does the log say?**

```bash
# (1) Is the daemon alive?
systemctl --user status unim-daemon
# Or PID check
unim-daemon --check && echo "RUNNING" || echo "STOPPED"

# (2) Turn on debug logging and reproduce
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
> ~/.unim-errors.log    # truncate
# … reproduce the bug …
tail -f ~/.unim-errors.log
```

> `UNIM_DEVELOP=1` aggregates Engine/DBus/Frontend/Extension logs into a single file (`~/.unim-errors.log`). The default is OFF so the log file does not grow unbounded.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

> Windows support was newly added in v0.4.0. Everything below reflects confirmed behavior; symptoms not listed here have not been verified yet — please report them at [GitHub Issues](https://github.com/from104/unim/issues).

Windows has **no daemon and no DBus.** UNIM is a **TSF** (Text Services Framework — the standard Windows mechanism for attaching an input method to an application) text service, `unim_tsf.dll`, and the OS loads it **directly inside the process of the app you are typing into**. So there is no "is the daemon dead?" check. Instead, look at two things:

1. **Is UNIM selected as the input method?** — Check the language bar (input indicator) at the right end of the taskbar for UNIM. Press `Win`+`Space` to cycle input methods and pick UNIM.
2. **What does the diagnostic log say?** — Logging is **OFF by default**. Turn it on with an environment variable:

```bat
:: No administrator rights needed — this writes a per-user environment variable
setx UNIM_DEBUG_LOG 1
```

- Environment variables are read **once, when a process starts**. Apps that are already running will not see the new value — fully close and reopen the app you are diagnosing. To be sure it applies to everything launched from the Start menu or Explorer, **sign out and back in**.
- Log file: `%TEMP%\unim-tsf.log`. Several apps append to the same file; each line is tagged `[unim-tsf <PID>]` so you can tell them apart. Paste `%TEMP%` into the Explorer address bar to open the folder.
- Clear the log before reproducing so it is easier to read: `del "%TEMP%\unim-tsf.log"`
- Other components write their own files in `%TEMP%` — `unim-popup-win.log` for the popup renderer, `unim-settings.log` for the settings app.
- The first line after each app relaunch is a one-time banner (`===== UNIM startup banner =====`) with the version, build timestamp, and the loaded DLL's path and modified time — check it first to confirm which build actually produced the log. Each log file caps at 5MB and rotates to `<file>.1` (e.g. `unim-tsf.log.1`) — if the banner is missing from `unim-tsf.log`, it scrolled into the `.1` file; grab both when collecting a report.

> ⚠️ **Do not turn on `UNIM_DEBUG_CONTENT` unless you were asked to.** With that variable also set, the actual keys you press and the text being composed and committed are written to the log verbatim. Passwords can end up in plain text. When you are done, turn both off with `setx UNIM_DEBUG_LOG ""` / `setx UNIM_DEBUG_CONTENT ""` and delete `%TEMP%\unim-tsf.log`.

> With logging off (the default), the logging code costs nothing — there is no reason to leave it on.
<!-- @endplatform -->

---

## 1. "Korean never types" — fresh install

<!-- @platform:linux -->
**🐧 Linux**

### First diagnosis

```bash
echo $GTK_IM_MODULE      # should be: unim
echo $QT_IM_MODULE       # should be: unim
echo $XMODIFIERS         # should be: @im=unim
unim-daemon --check && echo OK || echo MISSING
```

### Cause-by-cause fix

| Symptom | Cause | Fix |
|------|------|------|
| Env vars empty | im-config not configured | `im-config -n unim`, then log out/in |
| `unim-daemon --check` → MISSING | Systemd unit not enabled | `systemctl --user enable --now unim-daemon` |
| Visible in shell, not in GUI apps | Display manager did not load env | export in `~/.xprofile` or `/etc/environment` |
| GNOME+Wayland | Env-var path is dead | Use `gnome-extensions enable unim-gnome@from104.github.io` instead |
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Windows has no input-method environment variables like `GTK_IM_MODULE`. Check two things instead: whether UNIM is **registered** in the input-method list, and whether it is **selected**.

### First diagnosis

1. Open `Settings` → `Time & language` → `Language & region` and confirm **Korean** is installed. If it is not, add it — UNIM attaches to the Korean language profile and will not appear at all without it.
2. On the Korean entry, `⋯` → `Language options` → check that **`UNIM Korean IME`** is listed under **Keyboards**.
3. Open Notepad, press `Win`+`Space` to select UNIM, and type `dkssudgktpdy`. You should get `안녕하세요`.

### Cause-by-cause fix

| Symptom | Cause | Fix |
|------|------|------|
| Korean is not in the language list | Korean profile not installed | `Settings` → `Time & language` → `Language & region` → **Add a language** → Korean |
| `UNIM Korean IME` not under Keyboards | TSF registration missing or broken | Run `register-tsf.bat` from the install folder **as Administrator** (see the reinstall section below) |
| Missing right after installing | Registration not picked up yet | Sign out and back in (or reboot) |
| Missing only in 32-bit apps (KakaoTalk, Hancom, …) | 32-bit COM registration missing | See "UNIM missing only in 32-bit apps" below |
| Listed, but nothing types | A different input method is selected | Press `Win`+`Space` and pick UNIM |

> The default install folder is `C:\Program Files\UNIM\`. If you installed elsewhere, the real path is recorded in the registry at `HKLM\SOFTWARE\atit.org\UNIM`, value `InstallDir`.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 Linux** — §2–§4 cover the Linux frontends (GTK / Qt / GNOME extension) only.

## 2. "Broken only in GTK apps (GNOME Text Editor, etc.)"

### Diagnosis

```bash
ls /usr/lib/x86_64-linux-gnu/gtk-3.0/3.0.0/immodules/im-unim.so 2>/dev/null
ls /usr/lib/x86_64-linux-gnu/gtk-4.0/4.0.0/immodules/libim-unim.so 2>/dev/null

sudo gtk-query-immodules-3.0 --update-cache
sudo gtk-query-immodules-4.0 --update-cache
```

### Fix

- File missing → reinstall `unim-im-gtk` or rerun `sudo make install PREFIX=/usr`.
- File present but inert → refresh the module cache, restart GTK apps.
- Note: GTK4's filename is `libim-unim.so`, GTK3's is `im-unim.so` (different `lib` prefix).

> Tip: `GTK_IM_MODULE_FILE=/usr/lib/.../immodules.cache GTK_IM_MODULE=unim gnome-text-editor` shows module-load errors on stderr.

---

## 3. "Broken only in Qt apps (Kate, Krita)"

### Diagnosis

```bash
ls /usr/lib/x86_64-linux-gnu/qt5/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
ls /usr/lib/x86_64-linux-gnu/qt6/plugins/platforminputcontexts/libunimplatforminputcontextplugin.so
QT_DEBUG_PLUGINS=1 kate 2>&1 | grep -i unim
```

### Fix

- Plugin missing → reinstall `unim-im-qt`.
- `QT_DEBUG_PLUGINS=1` shows `Cannot load library` → check dependencies with `ldd <plugin>.so`.
- Plasma 6 prefers Qt6, so `QT_IM_MODULE=unim` is enough.

---

## 4. "GNOME extension not in the menu"

### Diagnosis

```bash
gnome-extensions list | grep unim
gnome-extensions info unim-gnome@from104.github.io
journalctl --user -u gnome-shell -b | grep -i unim
```

### Fix

- Make sure `~/.local/share/gnome-shell/extensions/unim-gnome@from104.github.io/` exists.
- If not, `make dev-extension` (source) or install the `unim-gnome` package.
- Enable: `gnome-extensions enable unim-gnome@from104.github.io` → Alt+F2 → `r` (X11) or log out/in (Wayland).
- Verify Shell version compatibility in `metadata.json`'s `shell-version` array.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 2-W. "UNIM missing only in 32-bit apps (KakaoTalk, Hancom, …)"

### Cause

On Windows, 64-bit and 32-bit applications look up input methods (COM/TSF) in **two separate registry views**. 64-bit apps resolve `unim_tsf.dll`; 32-bit apps resolve `unim_tsf32.dll`, each in its own view. If only the 32-bit side is missing, you get exactly this shape: **works fine in Notepad and Edge, but UNIM never shows up in KakaoTalk.**

The MSI performs both registrations. If UNIM is still missing, the registration is broken.

### Fix

Run `register-tsf.bat` from the install folder (`C:\Program Files\UNIM\`) **as Administrator**. That script re-registers **both** the 64-bit and the 32-bit DLL.

1. Search the Start menu for `cmd` → **Run as administrator**
2. Paste:

```bat
"C:\Program Files\UNIM\register-tsf.bat"
```

3. Sign out and back in.

> `register-tsf.bat` only redoes the **COM/TSF registration**. It does not reinstall files or restore any other installer-owned state — for that, use the MSI repair described in the reinstall section below.

> To undo it, run `unregister-tsf.bat` from the same folder, also as Administrator.

## 3-W. "Composition breaks up or the previous character disappears in a specific app"

### Symptom

In some apps (terminal emulators, some chat clients) the syllable you are composing gets committed mid-way, or the character before it vanishes. Standard text apps such as Notepad and Edge behave correctly.

### Cause

Windows has a compatibility layer (CUAS) that draws the composition string on behalf of apps that do not draw it themselves. That layer maintains composition differently from a proper text store, and it can **mistake the syllable still being composed for already-committed text**. UNIM detects such windows and switches to a fallback mode, but behavior varies per app, so **composition is not preserved perfectly everywhere.**

### Fix

- **There is no general fix yet — this is a known limitation.** Work on the underlying cause is in progress; it cannot be described as solved for all apps in this release.
- As a workaround, use a different input method in that specific app, or type in another window (Notepad) and paste.
- Reports genuinely help. File the **app name and version** together with the logs from "Diagnostic bundle" below at [GitHub Issues](https://github.com/from104/unim/issues) so the app can be added to the handled list.

> Observed so far in some terminal emulators and chat clients. This symptom has **not been confirmed** in KakaoTalk or Hancom Office — if you hit it there, that is worth reporting too.
<!-- @endplatform -->

---

## 5. "Hanja popup never appears"

<!-- @platform:linux -->
**🐧 Linux**

### Diagnosis

```bash
# Is the popup renderer alive? (X11/KDE/Xfce — if not, see §16)
pgrep -a unim-popup
# On GNOME Wayland, is the extension enabled?
gnome-extensions list --enabled | grep unim

# Are DBus signals being emitted? (in a separate terminal)
busctl --user monitor org.atit.unim.InputMethod
# Then type Korean and press Hanja → ShowHanjaPopup signal should appear
```

### Fix

| Environment | Popup renderer (0.3.0+) | Note |
| ----------- | ----------------------- | ---- |
| GNOME+Wayland | GNOME extension `popup_view.js` (St widget) | Extension receives `PopupRender` and paints directly |
| GNOME X11 / KDE / Xfce / X11 WM | `unim-popup-service` (GTK4) | Auto-launched via D-Bus activation |
| Wayland (KDE Plasma 6 / Sway, etc.) | `unim-popup-service` (GTK4, wayland-backend) | Needs `libgtk4-layer-shell`, experimental — see [popup spec §12](../../dev/specs/POPUP_SPEC.md) |

> Since 0.3.0, IM modules no longer draw their own popups. Rendering of the hanja /
> special-char / emoji popups is centralized in `unim-popup-service` (or the GNOME
> extension). Diagnosis therefore checks whether the renderer process is alive, not `popup_mode`.

> **DBus dead?**: `busctl --user list | grep atit` — if empty, the daemon failed to register. Check `journalctl --user -u unim-daemon -n 100`.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### Diagnosis

On Windows too, the input method does not draw popups itself. A **separate process, `unim-popup-win.exe`**, draws them, and `unim_tsf.dll` tells it what to draw over a named pipe. If that executable cannot be found, no popup appears.

1. **First check that the Hanja key gets through** — in Notepad, type `한자` and press the `Hanja` key (or right `Ctrl`). See whether the popup appears.
2. **Check the renderer process** — while the popup should be up, press `Ctrl`+`Shift`+`Esc` for Task Manager and look for `unim-popup-win.exe` on the **Details** tab.
3. **Check the executable is in place** — `unim-popup-win.exe` should exist in the install folder (`C:\Program Files\UNIM\`).

### Fix

| Symptom | Cause | Fix |
|------|------|------|
| `unim-popup-win.exe` missing from the install folder | Partially broken installation | MSI repair — see the reinstall section below |
| File is there but the process never starts | Renderer lookup failed | Check that `InstallDir` / `UnimPopupRenderer` under `HKLM\SOFTWARE\atit.org\UNIM` point at the real path |
| No popup at all (Hanja key does nothing) | Hanja key intercepted elsewhere | Try right `Ctrl`. If that also fails, check the Hanja key assignment in the settings app |

The renderer keeps its own log. Turn on `UNIM_DEBUG_LOG` as described at the top of this document, then read `%TEMP%\unim-popup-win.log`.

> You can inspect registry values with `Win`+`R` → `regedit`. **Read them, do not edit them** — if a path is wrong, an MSI repair is the correct fix.
<!-- @endplatform -->

---

## 6. "Special-character popup never appears"

Same code path as Hanja popup, so the cause is similar.

<!-- @platform:linux -->
**🐧 Linux** — There is no CLI key to query the current input mode (Korean/English) — check via the tray icon or the GNOME extension indicator instead.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — The current input mode (Korean/English) is shown by the **UNIM button in the language bar (input indicator)** at the right end of the taskbar. Clicking the button toggles Korean/English. If the indicator disagrees with what you are actually typing, press the `Hangul` key once to resynchronize.
<!-- @endplatform -->

Type one consonant (ㄱ–ㅎ) while in Korean mode, then press Hanja.

Works cleanly on Dubeolsik; on Sebeolsik the consonant entry is different.

---

## 7. "Hanja popup shows only 9 cells, period toggle does nothing"

<!-- @platform:linux -->
**🐧 Linux**

### Diagnosis

```bash
UNIM_DEVELOP=1 systemctl --user restart unim-daemon
# Open the Hanja popup, hit `.`, then:
grep -i 'ToggleExpanded\|9x9\|expanded' ~/.unim-errors.log
```

### Fix

- Old IM modules from 0.1.x may still be installed → `make build && sudo make install`.
- Check that your keymap actually emits `.` for the period key.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### Diagnosis

The period (`.`) key has to reach the popup while it is open for the 9 ↔ 81 toggle to fire.

1. Open the Hanja popup.
2. Press `.`. The grid should expand from 9 cells to 81.
3. If it does not, turn on `UNIM_DEBUG_LOG` as described at the top of this document, reproduce, and look for period-handling lines in `%TEMP%\unim-tsf.log`.

### Fix

- Check whether your keyboard layout maps `.` to something else (`Settings` → `Time & language` → `Language & region` → Korean → Keyboards).
- Try the `.` on the On-Screen Keyboard (`Win`+`Ctrl`+`O`). If it works there but not on the physical keyboard, the problem is the keyboard or the layout.
- Popup paging is `Page Down` / `Page Up`, bookmark (★) toggle is `Space`, cancel is `Esc`. If those work and only `.` does not, the period key alone is being intercepted.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 Linux** — §7-1 and §7-2 are Linux-only symptoms (Wayland compositors / XIM).

## 7-1. "Wayland popup shows ◀/▶ buttons but mouse clicks do nothing"

### Symptom

On `unim-frontends/wayland` popups (compositors: GNOME mutter, KWin, Sway, etc.) the ◀/▶ buttons render correctly but a mouse left-click on them produces no reaction.

### Cause

Wayland popups are drawn on `zwp_input_popup_surface_v2`. For pointer events to reach this surface, the compositor must route them into the IM popup. Some compositors (notably some GNOME mutter versions) treat the IM popup as pass-through, so the click falls through to the application below.

### Fix

- **Immediate workaround**: keyboard `←` / `→` (or `Page Up` / `Page Down`). 100 % equivalent to the mouse buttons.
- **GNOME users**: the GNOME Shell extension popup takes over and renders outside mutter, so ◀/▶ clicks work there. If they still fail on GNOME, the extension is probably disabled or missing — check with `gnome-extensions list --enabled | grep unim`.
- **Long-term**: depends on compositor support for IM popup pointer routing. File reports at [GitHub Issues](https://github.com/from104/unim/issues), include compositor name and version.

> Keyboard ←/→ is guaranteed across every compositor. Mouse ◀/▶ is best-effort, gated by compositor policy.

---

## 7-2. "XIM emoji popup shows ◀/▶ alongside category tabs"

### Symptom

In XIM (`unim-frontends/xim`) the emoji popup shows category tabs (smileys, animals, food, …) along the top *and* ◀/▶ paginate buttons at the bottom. Both react to left-click, which can be confusing.

### Cause / status

**Working as intended.** The XIM emoji popup has two distinct controls: (1) category tabs along the top (left-click switches category), (2) ◀/▶ in the footer (left-click moves one page within the active category). Both use left-click but they live in different regions.

### Fix

- Memorize the layout: category tabs on top, page nav on the bottom.
- Or use the keyboard:
  - **Switch category**: `Tab` (next) / `Shift+Tab` (previous).
  - **Page nav**: `←` / `→` (or `Page Up`/`Page Down`).

> The behavior is intended but visually under-separated. Tracked for a footer color tweak in a future release.
<!-- @endplatform -->

---

## 8. "AutoTypeFix not firing"

### Diagnosis

<!-- @platform:linux -->
**🐧 Linux**

```bash
unim-cli config show | grep -i typefix         # master/forward/reverse enabled state
cat ~/.config/unim/typefix-blacklist.yaml | head -50
```
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

There is no `unim-cli` on Windows. Use the settings app.

1. Start menu → **UNIM** → **UNIM Settings** (or the UNIM language-bar button → menu → `Settings`)
2. On the **Type Correction** page, check the master toggle and the forward/reverse enabled state.
3. On the **Suppression Words** page, review the registered words.

To look at the configuration files directly, paste this into the Explorer address bar:

```
%APPDATA%\unim
```

`config.yaml` (all settings) and `typefix-blacklist.yaml` (suppression words) live there.
<!-- @endplatform -->

### Fix

- Master toggle off → flip ON in the GUI.
- Particular word always blacklisted → "Suppression Words" page, mark as Inactive or remove.
- No word boundary yet → the correction only fires after a space/punctuation. Type one more char then space — corrects immediately.
- "Skip in English mode" is ON by intent (default).

---

## 8-1. "AutoTypeFix fires in a password field"

### Cause

Password protection ([FAQ](../faq/README.md) Q9) works only when the app reports "this field is a password" (`content_purpose`). The environments below cannot deliver that signal to UNIM, so the password field is treated as a normal field.

| Environment | Status | Reason |
|------|------|------|
| GTK3/4, Qt5/6, GNOME extension, Windows TSF (both 64-bit and 32-bit `unim_tsf32.dll`) | Detected | content_purpose / InputScope delivered correctly |
| Legacy XIM apps | Not detected | XIM protocol has no such signal |
| Some Wayland compositors / web forms | Not detected | content-purpose not sent (app/compositor's discretion) |
| GTK apps that change purpose after focus | Not detected | The GTK IM reads input-purpose only at focus time and does not subscribe to `notify::input-purpose` (existing limitation) — if the same field later becomes a password, it is not reflected until re-focus |

> **This table is still sparse — reports are wanted.** Which applications report password fields correctly and which do not can only be filled in from real-world use. On Linux and Windows alike there are not enough cases yet, so per-application handling is incomplete. If you find an app where correction fires inside a password field, please report the **app name and version** on [GitHub Issues](https://github.com/from104/unim/issues). That is the only way this table gets filled in.

### Fix

- In undetectable environments, verify **English mode** with the Hangul key before typing your password. In English mode, forward (English→Korean) correction is suppressed by default, so it is effectively safe.
- If a specific app hits this often, temporarily turn AutoTypeFix off with a toggle hotkey ([User Guide](../user-guide/README.md) 4.4).
<!-- @platform:linux -->
- When assigning toggle hotkeys, **do not reuse the Hangul/English or Hanja keys** — the roles conflict and language switching or Hanja conversion may be shadowed by the toggle (the CLI `unim-cli config set` warns on such duplicates).
<!-- @endplatform -->
<!-- @platform:windows -->
- When assigning toggle hotkeys, **do not reuse the Hangul/English or Hanja keys** — the roles conflict and language switching or Hanja conversion may be shadowed by the toggle.
<!-- @endplatform -->

> **preedit-exposure — tracked separately**: In a password field, Korean composition itself is blocked, so a character briefly showing as preedit (underline) during composition is essentially absent in the correctly-detected environments. In the undetectable Wayland environments above, exposure could occur in theory; this is **tracked as a separate issue**, and the current recommended workaround is "verify English mode manually" above.

---

## 9. "AutoTypeFix corrects too aggressively / wrongly"

### Fix

<!-- @platform:linux -->
**🐧 Linux**

| Case | Fix |
|--------|-----|
| One specific word | BS + Hangul to roll back → next time the same word triggers, it auto-registers as Tentative |
| Frequent regrets | Settings → "Type Correction" → bump tentative-expiry hours |
| Reverse fires in English mode | Turn ON `auto_typefix.reverse.skip_incomplete_syllable` |
| Hand-edit | Open `~/.config/unim/typefix-blacklist.yaml` — daemon hot-reloads on mtime change |
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

| Case | Fix |
|--------|-----|
| One specific word | BS + Hangul to roll back → next time the same word triggers, it auto-registers as Tentative |
| Frequent regrets | Settings app → "Type Correction" → bump tentative-expiry hours |
| Reverse fires in English mode | Check the reverse-correction options on the settings app's "Type Correction" page |
| Hand-edit | Open `%APPDATA%\unim\typefix-blacklist.yaml` in Notepad |

> After hand-editing a configuration file, **click away from the app you are typing in and back again**. Windows has no daemon pushing settings out, so UNIM checks whether the file changed **when input focus returns** to the app, and reloads then. Changes are therefore not always instant.
<!-- @endplatform -->

---

## 10. "Settings won't save / changes don't take effect"

<!-- @platform:linux -->
**🐧 Linux**

### Diagnosis

```bash
ls -la ~/.config/unim/
test -w ~/.config/unim/config.yaml && echo writable || echo BLOCKED
unim-cli config show 2>&1 | head -5
journalctl --user -u unim-daemon -n 50
```

### Fix

- Permission issue → `chmod 644 ~/.config/unim/*.yaml`, `chmod 755 ~/.config/unim`.
- Root-owned files → `sudo chown -R $USER:$USER ~/.config/unim`.
- Changed in GUI but daemon doesn't pick it up → `systemctl --user restart unim-daemon`.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### Diagnosis

All settings live in one file, `%APPDATA%\unim\config.yaml`. Paste `%APPDATA%\unim` into the Explorer address bar to open the folder.

1. Check that `config.yaml` exists and that its **Date modified** matches when you last saved.
2. If the timestamp did not change, the save itself failed — close the settings app, reopen it, and save again.
3. If the timestamp did change but typing does not reflect it, go to the fixes below.

### Fix

- **Most common cause — when the change takes effect.** Windows has no daemon pushing settings out. UNIM checks whether `config.yaml` changed **when input focus returns to the app**, and reloads then. After changing a setting, **click away from the app you are typing in and back again** and it will apply.
- If that does not work, fully close and reopen the app.
- If it stays wrong across several apps, sign out and back in.
- A **read-only** `%APPDATA%\unim` folder or `config.yaml` blocks saving. Right-click the file → `Properties` → clear **Read-only**.
- On a managed PC, `%APPDATA%` may be locked down by policy, which prevents saving — that one is a question for your administrator.
<!-- @endplatform -->

---

<!-- @platform:linux -->
**🐧 Linux** — §11–§14 are Linux-only. Windows has no GTK IM module, no Flatpak, no Snap, and no resident daemon.

## 11. "Keys are locked (ghostty/terminal)"

Symptom: after one keystroke, the terminal freezes IME-wise.

### Cause

Missing `preedit-end` signal in GTK3/4 IM (a 0.1.x leftover; resolved in 0.2.0 via the `unim_emit_preedit` helper).

### Fix

```bash
unim-cli --version       # 0.2.0+ contains the fix
```

If 0.1.x, rebuild and reinstall: `make build && sudo make install`.

---

## 12. "Korean broken in Flatpak apps (Telegram, VS Code)"

### Diagnosis

```bash
flatpak list --columns=application,environment | grep -E 'GTK_IM|QT_IM'
```

### Fix

On GNOME+Wayland, the host's IM env vars leak into the Flatpak sandbox and block input. Auto-handling should clear them.

```bash
# Verify auto-handling worked
journalctl --user -u unim-daemon | grep -i flatpak
# These two lines mean OK:
#   [Flatpak] GNOME+Wayland detected — applying Flatpak IM override
#   [Flatpak] IM environment override done

# Manual fallback
flatpak override --user --env=QT_IM_MODULE= --env=GTK_IM_MODULE=
flatpak kill org.telegram.desktop
```

On X11 or non-GNOME you actually need to keep the env vars — auto-handling fires only on GNOME+Wayland.

> ⚠️ **Persists after uninstalling UNIM**: this override is written permanently to your per-user `~/.local/share/flatpak/overrides/global` file and is not reverted automatically when the `unim` package is removed. If you switch to another input method and Flatpak apps start misbehaving, unset it yourself:
> ```bash
> flatpak override --user --unset-env=QT_IM_MODULE --unset-env=GTK_IM_MODULE
> ```

---

## 13. "Korean broken in Snap apps"

Snap inherits host env vars but offers no global-override mechanism.

### Fix

Conditional export in `~/.profile`:

```bash
if [ "$XDG_SESSION_TYPE" = "wayland" ] && echo "$XDG_CURRENT_DESKTOP" | grep -q "GNOME"; then
    export GTK_IM_MODULE=
    export QT_IM_MODULE=
else
    export GTK_IM_MODULE=unim
    export QT_IM_MODULE=unim
fi
export XMODIFIERS="@im=unim"
```

Or per-launch:

```bash
QT_IM_MODULE= GTK_IM_MODULE= snap run telegram-desktop
```

---

## 14. "Daemon eats too much memory (RSS 500 MB+)"

### Diagnosis

```bash
grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
cat /proc/$(pidof unim-daemon)/smaps_rollup | grep -E 'Rss|Anonymous'
```

### Fix

UNIM 0.2.0 ships with `tikv_jemallocator` + `MALLOC_ARENA_MAX=2` + a 60-second `malloc_trim(0)` task, which keeps RSS in the low MB. If you still cross 500 MB:

```bash
# Quick recovery
systemctl --user restart unim-daemon

# Diagnostics for an issue report
ps -o pid,rss,vsz,cmd $(pidof unim-daemon)
journalctl --user -u unim-daemon -n 500 > unim-mem.log
```

[`AGENTS.md` §Memory rules](../../dev/architecture/AGENTS.md) lists the regression-banned items and diagnostic commands.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 14-W. Reinstall · Repair · Uninstall

UNIM for Windows ships as **a single MSI installer**. When files are damaged or registration has drifted, do not delete things by hand — use the steps below.

### Repair (restore files and registration)

Run the MSI file you downloaded again; the installer offers a **Repair** option. If you no longer have the file, download the same version from [GitHub Releases](https://github.com/from104/unim/releases).

### Reinstall (including upgrades)

Just run the new MSI. It replaces the previous version — there is no need to uninstall first.

### Uninstall

`Settings` → `Apps` → `Installed apps` → **UNIM Korean IME** → `Uninstall`. The **Uninstall UNIM** shortcut in the **UNIM** Start-menu folder does the same thing.

### Re-register only

Use this when the files are fine but UNIM has disappeared from the input-method list. Run `register-tsf.bat` from the install folder **as Administrator** (see §2-W). To undo, run `unregister-tsf.bat`.

### What is in the install folder

The default path is `C:\Program Files\UNIM\`.

| File | Role |
|------|------|
| `unim_tsf.dll` | The input method itself, for 64-bit apps |
| `unim_tsf32.dll` | The input method itself, for 32-bit apps (KakaoTalk, Hancom, …) |
| `unim-settings.exe` | Settings app |
| `unim-popup-win.exe` | Renderer for the Hanja / special-character / emoji popups |
| `register-tsf.bat` / `unregister-tsf.bat` | Register / unregister the input method (Administrator) |
| `help\unim-help-ko.html`, `help\unim-help-en.html` | The offline manual you are reading now |
| `LICENSE.txt`, `NOTICE.txt`, `LICENSES\` | License notices |

> **Your settings are not in this folder.** They are stored per user in `%APPDATA%\unim\` and are **not** removed when you uninstall UNIM. To wipe them too, delete that folder yourself after uninstalling.
<!-- @endplatform -->

---

## 15. "Moachigi (chord) not recognized correctly"

Chord input failures have several possible causes. Work through the items below in order.

### 15-1. The active layout does not support moachigi

Chord input is only available for layouts that carry `supports_moachigi: true`. Among the built-ins, only the **Ahnmatae layout (ko_3bul_anmatae)** qualifies. **Qwerty Sebeolsik v2** is preserved as a research reference (`docs/references/keymaps/ko_3bul_qwerty_v2.json`); copy it into your user layout folder to enable it as a moachigi-capable user profile.

<!-- @platform:linux -->
The user layout folder is `~/.config/unim/layouts/` — copy the file to `~/.config/unim/layouts/ko_3bul_qwerty.json`.
<!-- @endplatform -->

<!-- @platform:windows -->
The user layout folder is `%APPDATA%\unim\layouts\` — copy the file to `%APPDATA%\unim\layouts\ko_3bul_qwerty.json`. Create the folder if it does not exist.
<!-- @endplatform -->

<!-- @platform:linux -->
```bash
# Check the active layout
unim-cli config show | grep -E 'layout|keymap'
```
<!-- @endplatform -->

<!-- @platform:windows -->
Check the active layout in the layout list on the **General** page of the settings app (`unim-settings.exe`).
<!-- @endplatform -->

<!-- @platform:linux -->
If the active layout is not moachigi-capable, switch to one in the GTK settings dialog. The **Moachigi** option group appears automatically once a compatible layout is selected.
<!-- @endplatform -->
<!-- @platform:windows -->
If the active layout is not moachigi-capable, switch to one in the settings app (`unim-settings.exe`). The **Moachigi** option group appears automatically once a compatible layout is selected.
<!-- @endplatform -->

### 15-2. chord_window_ms is too short

The recommended default for `chord_window_ms` is **60 ms**. If you are new to moachigi or type at a moderate pace, start at **80–100 ms** and lower the value as you become comfortable.

<!-- @platform:linux -->
```bash
# Check current setting
unim-cli config show | grep chord-window

# Set to 80 ms
unim-cli config set korean-chord-window-ms 80
```

Alternatively, use the settings app (`unim-settings`) → General page → layout options (shown only for chord-capable layouts) → slider.
<!-- @endplatform -->

<!-- @platform:windows -->
Use the settings app (`unim-settings.exe`) → General page → layout options (shown only for chord-capable layouts) → slider.
<!-- @endplatform -->

### 15-3. bidirectional_combine is off

If reverse-order jamo combinations do not work (e.g., ᆯ+ᆨ → ᆰ, or ㅎ+ㄱ → ㅋ), the **Bidirectional Jamo Combine** option is disabled.

<!-- @platform:linux -->
```bash
# Check current state
unim-cli config show | grep bidirectional-combine

# Enable
unim-cli config set korean-bidirectional-combine true
```

Or in the settings app (`unim-settings`) → General page → layout options → **Bidirectional Jamo Combine** toggle → ON.
<!-- @endplatform -->

<!-- @platform:windows -->
In the settings app (`unim-settings.exe`) → General page → layout options → **Bidirectional Jamo Combine** toggle → ON.
<!-- @endplatform -->

### 15-4. Keyboard does not support NKRO (ghosting)

Standard membrane keyboards are limited to 2–3 KRO (Key Rollover). When more keys are pressed simultaneously than the keyboard can report, the extras are silently dropped — this is called **ghosting**. Symptoms include chords that are consistently incomplete or produce the wrong jamo.

Self-diagnosis:

<!-- @platform:linux -->
```sh
# Check simultaneous key events on X11
xev -event keyboard
```

On Wayland, use `wev` instead of `xev` (`apt install wev` or equivalent).

Focus the window that appears, then press all keys in your chord at once. The terminal must print one `KeyPress event` per key. You can also use an online key tester such as [keyboardchecker.com](https://keyboardchecker.com).
<!-- @endplatform -->

<!-- @platform:windows -->
Open an online key tester such as [keyboardchecker.com](https://keyboardchecker.com) in your browser, then press all keys in your chord at once. **Every** key you pressed must register. If any are dropped, that is the keyboard's own simultaneous-key limit (ghosting) — no UNIM setting can work around it.
<!-- @endplatform -->

Fix: use a gaming keyboard or a mechanical keyboard in NKRO mode. See [Ahnmatae keyboard guide — Keyboard Compatibility](../keymaps/anmatae.en.md#keyboard-compatibility-nkro-recommended) for details.

### 15-5. Low USB polling rate (125 Hz = 8 ms resolution)

The default USB polling rate is 125 Hz (one report every 8 ms). If `chord_window_ms` is set to 10–30 ms, the polling interval itself occupies most of the window, causing some chord keys to be missed.

Fix:

- Raise `chord_window_ms` to **60 ms or higher** to accommodate polling latency.
- Switch to a 1000 Hz gaming keyboard, or try a different USB port.

---

## Build failure

<!-- @platform:windows -->
**🪟 Windows** — UNIM for Windows is distributed **only as an MSI installer**; there is no reason for an end user to build from source. If the installation itself failed, see "Reinstall · Repair · Uninstall" above. Developers building from source should refer to the `docs/dev/windows/` documents in the repository.
<!-- @endplatform -->

<!-- @platform:linux -->
**🐧 Linux**

```bash
make clean
make build 2>&1 | tee /tmp/unim-build.log
```

| Error | Cause | Fix |
|------|------|------|
| `lock file version 4 requires '-Znext-lockfile-bump'` | cargo 1.75 (old) | `rustup update stable` to get cargo 1.95+ |
| `gtk4/libadwaita not found` | Dev headers missing | `sudo apt install libgtk-4-dev libadwaita-1-dev` |
| `Qt6Core not found` | Qt6 dev missing | `sudo apt install qt6-base-dev` |
| `cxx-qt build error` | Qt header path mismatch | Inspect `pkg-config --cflags Qt6Core` |
| Any warnings | UNIM enforces zero-warning | File the warning verbatim as an issue |

> The canonical build command is `make build`. `cargo build --workspace` alone misses the C/C++ frontends.
<!-- @endplatform -->

---

## Diagnostic bundle (for issue reports)

<!-- @platform:linux -->
**🐧 Linux**

```bash
{
  echo "=== version ==="
  unim-cli --version
  echo "=== env ==="
  echo "session=$XDG_SESSION_TYPE"
  echo "desktop=$XDG_CURRENT_DESKTOP"
  env | grep -E 'GTK_IM|QT_IM|XMOD' | sort
  echo "=== daemon ==="
  systemctl --user status unim-daemon --no-pager
  echo "=== config ==="
  unim-cli config show
  echo "=== logs (last 200) ==="
  tail -n 200 ~/.unim-errors.log 2>/dev/null
} > unim-report.txt
```

Attach `unim-report.txt` to your issue. Skim it once first — passwords/tokens are unlikely to be in the log, but a quick check is wise.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Including these five things with your issue makes diagnosis far faster.

1. **Windows version** — `Win`+`R` → `winver`. Report the version and build number from the dialog.
2. **UNIM version** — from the settings app (`unim-settings.exe`), or the `About` item in the UNIM language-bar button menu.
3. **Name and version of the app where it happens** — this matters most, since behavior varies per app. Whether the app is 32-bit or 64-bit also helps (Task Manager → Details → right-click the process → `Properties`).
4. **Configuration file** — `%APPDATA%\unim\config.yaml`.
5. **Diagnostic log** — turn on `UNIM_DEBUG_LOG` as described at the top of this document, reproduce the symptom, then collect `%TEMP%\unim-tsf.log`. For popup problems, include `%TEMP%\unim-popup-win.log` as well.

Pasting this into a Command Prompt gathers the logs onto your Desktop:

```bat
copy "%TEMP%\unim-tsf.log" "%USERPROFILE%\Desktop\unim-report-tsf.log"
copy "%TEMP%\unim-popup-win.log" "%USERPROFILE%\Desktop\unim-report-popup.log"
copy "%APPDATA%\unim\config.yaml" "%USERPROFILE%\Desktop\unim-report-config.yaml"
```

> ⚠️ **Open and skim the files before attaching them.** If you also had `UNIM_DEBUG_CONTENT` on, the text you actually typed is in the log. Remove anything with passwords or personal information before uploading.
<!-- @endplatform -->

---

## Read more

- [FAQ](../faq/README.md) — comparison with other IMEs, coexistence, backup
- [User manual](../user-guide/README.md) — settings GUI page-by-page
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — behavior spec (developer-oriented)
- [`AGENTS.md`](../../dev/architecture/AGENTS.md) — architecture and memory rules

---

<!-- @platform:linux -->
**🐧 Linux** — §16 and the 0.2.0 release notes below are Linux-only.

## 16. popup-service debugging (0.3.0+)

### "Hanja / special-character / emoji popup never appears" (GNOME X11 or KDE/Xfce)

Since 0.3.0, all popups are rendered exclusively by `unim-popup-service`. Even if the daemon is running, popups will not appear if popup-service is not available.

#### Diagnostic commands

```bash
# Check whether popup-service process is running
pgrep -a unim-popup

# Verify the DBus interface is exposed
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup

# Check that the D-Bus service activation file is installed
ls ~/.local/share/dbus-1/services/org.atit.unim.PopupService.service \
   /usr/share/dbus-1/services/org.atit.unim.PopupService.service 2>/dev/null
```

#### Fixes

- If the service file is absent, the **`unim-desktop`** package (which bundles `unim-popup-service` together with the indicator and the legacy settings dialog) is not installed — `unim-popup-service` is not a standalone package.

  ```bash
  # deb (match the version to whatever you actually downloaded)
  sudo apt install ./unim-desktop_<version>_amd64.deb
  # or, from source
  sudo make install PREFIX=/usr
  ```

- If the service file exists but popups still do not appear, start the service manually to see log output:

  ```bash
  UNIM_DEVELOP=1 unim-popup-service &
  # trigger hanja popup, then check terminal output
  ```

- If `busctl` introspect fails entirely, the service is not responding. Check:

  ```bash
  systemctl --user status unim-popup-service
  journalctl --user -t unim-popup-service -b --no-pager
  ```

### "Popup closes immediately when I click it"

Clicking **outside** the popup is intentional dismiss behavior — the popup closes and the click event is passed through to the window underneath. Clicking inside a cell or button should not close the popup. If clicking inside the popup dismisses it, the popup position or size is being calculated incorrectly; check the DBus caret coordinates (`caret_rect`).

### "Two popups appear at once on GNOME Wayland"

`Meta.is_wayland_compositor()` detection has failed, causing both the extension `PopupView` and the popup-service GTK4 window to render simultaneously. Check your GNOME Shell version, then disable and re-enable the `unim-gnome@from104.github.io` extension.

### "The character after a commit does not show up (XIM)"

Right after a syllable is committed, the next jamo you type does not appear on screen. Open since 0.3.0, and **the status now differs by path.**

**Fixed (2026-08-07)** — for self-hosted XIM clients and OVER-THE-SPOT clients (XTerm, WezTerm). The cause was **not** what this page previously claimed (the xim crate's `commit()` not updating `preedit_started`) — removing that workaround entirely left the symptom unchanged. The real cause is that **ON-THE-SPOT clients stop processing messages once they hit `Commit` while handling a key**, so a new preedit sent after the commit was discarded. XIM now sends the preedit **before** the commit; see the exception in `docs/dev/architecture/IME_BEHAVIOR.md` §8.1.

**Still open (confirmed 2026-08-10)** — when GTK attaches through its XIM module (`im-xim`), the symptom remains and the cause is different. There the input method stalls right after the commit, so **the next character is swallowed for several seconds** (libX11 recovers on its own after a timeout). Sending `PreeditDraw` is what wedges that input context: it stops receiving further keys. Reproduces 3/3.

- **Who hits this**: mostly **Flatpak and Snap apps**. The host's `im-unim.so` is not visible inside the sandbox, so GTK falls back to XIM. Obsidian (Electron) is the common case.
- **Unaffected**: normally installed GTK/Qt apps (they use the native IM modules), and OVER-THE-SPOT clients such as XTerm and WezTerm.
- **Workaround today**: none within the sandbox. Installing the same app from a **non-sandboxed package** (deb, AppImage) makes it use the native IM module and the symptom disappears. The IBus compatibility path was substantially repaired in 0.4.0 but does not yet display preedit text, so it is not a usable alternative.

Progress is tracked in ROADMAP phase 3, "Sandboxed apps (Flatpak/Snap) input path".

---

## 0.2.0 release-specific notes

> Auxiliary diagnostics drafted by manual-test-planner just before the 0.2.0 release. The user-facing sections above (§1–§14) take precedence; this section keeps only supplementary diagnostic tools and regression-watch items.

### A. Diagnostic helpers

| Command | Purpose |
| --- | --- |
| `journalctl --user -u unim -b --no-pager` | Daemon systemd logs (this boot) |
| `: > ~/.unim-errors.log; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &` | Reset log + restart in dev mode |
| `pgrep -a unim-` | All unim-* processes |
| `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod` | DBus API surface |

### B. 0.2.0 regression-watch cases

- gedit double-commit (`늘늘`): focus-out CommitText broadcast — fixed in 0.2.0.
- English-mode space drop (gedit): fixed via `consumed=true commit=" "` path.
- AutoTypeFix residual BS (XIM): fixed via N+1 BS model. Chrome preedit edge case is a known SKIP.
- `tentative_expiry_hours` unit changed days → hours (1..=12) since 0.2.0; existing config auto-migrates.
- **XIM preedit drop after commit (PARTIALLY FIXED)**: Self-hosted XIM clients and OVER-THE-SPOT were fixed on 2026-08-07 — ON-THE-SPOT clients stop processing messages once they hit `Commit` while handling a key (the long-standing claim that xim's `commit()` fails to update `preedit_started` was a misdiagnosis), so XIM — and only XIM — now sends the preedit before the commit (`IME_BEHAVIOR.md` §8.1). **However, the path where GTK attaches via `im-xim` is still broken as of 2026-08-10, with a different cause**: sending `PreeditDraw` wedges that input context so it receives no further keys (reproduces 3/3). Flatpak and Snap apps are the main victims. Regression watch: `tests/unim-test-xim` and `tests/unim-test-gtk3` (ON-THE-SPOT), `xterm` (OVER-THE-SPOT), plus `tests/unim-test-gtk3` launched with `GTK_IM_MODULE=xim`.

### C. Multiple daemon instances

- `pkill -9 -x unim-daemon; sleep 1; systemctl --user start unim`
- Caused by DBus auto-activation overlapping with manual launch. When launching manually, use `--replace`.

### D. Hanja popup coordinates

- caret_rect missing → `cursor_y = 0` fallback (see POPUP_SPEC §6.3).
- 9-cell ↔ 81-cell toggle blocked → period (`.`) key intercepted elsewhere; check keymap.
- Bookmark (★) sync: `HanjaBookmarkChanged` signal not reaching listeners → `busctl --user monitor org.atit.unim.InputMethod`.

### E. CLI Korean text renders garbled

- Locale not installed: `sudo locale-gen ko_KR.UTF-8`
- gettext `.mo` file missing: `ls /usr/share/locale/ko/LC_MESSAGES/unim*.mo`

### F. `unim-cli config set` doesn't show up in the GUI

- The daemon failed to hot-reload on mtime change → `pkill -SIGHUP unim-daemon`
- Possible 5-point sync breakage → verify the CLI/engine/GUI/locale/DBus all got updated together

### G. Environment matrix (reconfirmed for 0.4.0 — originally written for 0.3.0)

| Environment | Support | Notes |
| --- | --- | --- |
| GNOME Wayland | ✅ Validated | GNOME extension `popup_view.js` (St widgets) renders popups directly |
| GNOME X11 | ✅ Validated | popup-service GTK4 + GNOME extension assist |
| X11 + KDE Plasma 5.x | ✅ Validated | popup-service GTK4 |
| X11 + XFCE / MATE / Cinnamon / LXDE | ✅ Validated | popup-service GTK4 |
| Wayland + KDE Plasma 5.x | ❌ Unsupported | `gtk4-layer-shell` missing in Ubuntu 24.04 (noble) standard repos → use X11 session or GNOME |
| Wayland + KDE Plasma 6 | ⚠️ Experimental | Requires `wayland-backend` feature + `libgtk4-layer-shell`. Not exercised in 0.4.0 QA either (unchanged since 0.3.0) |
| Sway / Hyprland / river (standalone Wayland) | ⚠️ Experimental | Same as above. Possible regressions in popup placement and IME focus handover |
| Weston etc. reference Wayland | ⚠️ Experimental | Same as above |

> **0.4.0 reconfirmation**: the table above still holds as of the v0.4.0 release — **pure (non-GNOME) Wayland still does not support the hanja/special-character popup** in this release (a deliberate design constraint, unchanged), and Wayland compositors that don't go through GNOME remain "experimental". Windows (TSF, both 64-bit and 32-bit via `unim_tsf32.dll`) is a newly added **experimental** platform in v0.4.0 and is not included in this table — see [FAQ Q11](../faq/README.md#q11-does-unim-run-on-macos--windows) instead.

⚠️ For issues on experimental environments, please file a bug at [GitHub Issues](https://github.com/from104/unim/issues).

### H. Log-analysis slash command

```bash
# If you're using Claude Code
/unim-log
```
→ automatically classifies, summarizes, and diagnoses `~/.unim-errors.log`.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

## 16-W. Known Windows limitations (v0.4.0)

Windows support was newly added in v0.4.0. Below are the limitations known at release time. **Anything not listed here has simply not been verified yet** — if you run into it, please report it.

| Item | Status | Notes |
|------|--------|-------|
| Composition breaks / previous character lost in some apps | ⚠️ Known limitation | Behavioral differences in the Windows compatibility layer (CUAS) used when an app does not draw the composition string itself. UNIM falls back, but not perfectly in every app. See §3-W |
| Console / terminal-style apps | ⚠️ Varies by app | Depends on which input mechanism the app uses. Standard text apps (Notepad, Edge) are the reference behavior |
| 32-bit apps such as KakaoTalk and Hancom Office | ✅ Supported (limited verification) | Covered by also installing the 32-bit input method (`unim_tsf32.dll`). If it does not appear, see §2-W |
| Individual symptoms inside 32-bit apps | ❓ Unverified | Hangul input itself is confirmed, but per-app details are outside what has been tested |
| Tray icon / resident indicator | ❌ Not present | On Windows the **UNIM button in the language bar (input indicator)** fills that role |
| `unim-cli` command-line tool | ❌ Not present | Configure via the settings app, or by editing `%APPDATA%\unim\config.yaml` directly |
| Settings applying instantly | ⚠️ Focus-driven | With no daemon to push settings out, you must leave and re-enter the app you are typing in. See §10 |
| Diagnostic commands from the Linux docs | ❌ Not applicable | `systemctl`, `journalctl`, `busctl`, `gsettings`, `im-config` and the `GTK_IM_MODULE` / `QT_IM_MODULE` / `XMODIFIERS` environment variables do not exist on Windows |

### Reporting

File the **app name and version** together with the logs from "Diagnostic bundle" above at [GitHub Issues](https://github.com/from104/unim/issues). Compatibility on Windows varies from app to app, so real-world reports are the single most useful contribution.
<!-- @endplatform -->
