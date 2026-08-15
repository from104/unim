# UNIM FAQ (English)

> The questions people actually ask about UNIM 0.4.1.
> Each answer carries at least one line of "why it works that way" so you can use it for your next decision, not just as a fact lookup.

---

<!-- @platform:linux -->
## Q1. How is UNIM different from ibus-hangul, fcitx-hangul, kime, nimf?

| Item | UNIM | ibus-hangul | fcitx-hangul | kime | nimf |
|------|------|-------------|--------------|------|------|
| Core language | Rust | C | C | Rust | C |
| Transport | DBus daemon + IM module | IBus daemon | Fcitx daemon | Embedded | Daemon |
| GTK3/4 | ✅ Native IM module | ✅ | ✅ | ✅ | ✅ |
| Qt5/6 | ✅ Native plugin | ✅ | ✅ | ✅ | ✅ |
| XIM | ✅ | ✅ | ✅ | ✅ | ✅ |
| Wayland (input-method-v2) | ✅ | ✅ (IBus) | ✅ | △ | ✅ |
| GNOME Shell direct integration | ✅ Own extension | ✅ (IBus) | △ | ✗ | ✗ |
| Auto Korean↔English typo fix | ✅ AutoTypeFix (forward + reverse + learning) | ✗ | ✗ | ✗ | ✗ |
| Hanja 9-cell / 81-cell grid | ✅ unified | △ | △ | ✗ | △ |
| Hanja bookmarks | ✅ DBus signal sync | ✗ | ✗ | ✗ | ✗ |
| User layout v1 JSON | ✅ inherits + rule_sets | ✗ | △ | △ | ✗ |
| License | (see project license) | LGPL/GPL | GPL | GPLv3 | LGPL |

**One-liner**: UNIM's design is "one Rust core plugged into every environment", and it differentiates further on user-experience features (AutoTypeFix, learned suppression dictionary) that other IMEs do not have.

---
<!-- @endplatform -->

## Q2. Can UNIM coexist with another IME on the same desktop?

<!-- @platform:linux -->
**🐧 Linux**

**Technically yes, but not recommended.** With two IMEs alive, the OS and toolkit cannot tell where to deliver key events.

- **GNOME**: leaving IBus enabled alongside UNIM causes frequent key drops → uninstall IBus.
  ```bash
  sudo apt remove ibus
  ```
- **KDE**: fcitx5 runs its own daemon and conflicts. Pick exactly one through env vars.
- **Test bench**: separating into VMs/containers is fine for comparison.

> Bottom line: pick exactly one. Cleanly remove the other before installing UNIM.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

**Coexistence is fine here.** Windows is built to keep **several input methods registered side by side** — TSF (Text Services Framework) is the architecture UNIM plugs into — and let you pick one, so there is no "remove everything else first" step like on Linux. The UNIM installer only **adds** UNIM to that list; it never removes an existing input method.

- See the registered list under **Settings → Time & language → Language & region → Korean → Keyboards**.
- Which one comes up first is decided by the **default input method**. Right-click the input indicator in the taskbar and choose **`Set as default IME`** to make UNIM the default.
- That said, when you are **diagnosing a problem**, temporarily disabling other Korean IMEs (Saenaru, Nalgaeset, etc.) and reproducing with UNIM alone makes the cause much easier to isolate.
<!-- @endplatform -->

---

## Q3. Which environments are most stable?

<!-- @platform:linux -->
**🐧 Linux**

Stability tier as of UNIM 0.2.0:

| Environment | Tier | Note |
|------|------|------|
| Ubuntu 24.04 + GNOME(Wayland) + extension | 🟢 A | Recommended; main dev/test environment |
| Ubuntu 24.04 + GNOME(X11) | 🟢 A | Env vars + Standalone popup |
| KDE Plasma 6 (Wayland) | 🟢 B+ | input-method-v2 works |
| KDE Plasma 6 (X11) | 🟢 B+ | XIM/Qt IM both fine |
| Sway (Wayland) | 🟡 B | Popup positioning slightly off — see [popup spec §8.4](../../dev/specs/POPUP_SPEC.md) |
| Hyprland (Wayland) | 🟡 B | Same |
| XFCE/MATE (X11) | 🟢 B+ | Traditional, solid |
| Pure Wayland (compositor-dependent) | 🟡 B/C | Depends on the compositor's IM protocol support |

**Tier A = "first-time user starting point"**. Once you are familiar, the rest is fine too.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Windows support landed in v0.4.0. There is not yet enough measured data across machines to grade environments into tiers the way the Linux side does. Here is the support surface that is confirmed today.

| Item | Detail |
|------|--------|
| OS | Windows 10 / 11 (64-bit) |
| Input path | TSF (Text Services Framework) |
| 64-bit apps | `unim_tsf.dll` — Notepad, Edge, Chrome, Word, … |
| 32-bit apps | a separate 32-bit TIP, `unim_tsf32.dll` — KakaoTalk, Hancom, … |
| Console / IMM32-style apps | Hangul composition works in WezTerm, Telegram, and similar |

- **The range of applications tested is narrower than on Linux.** It is in daily production use on the maintainer's machine, but with uncommon apps the documented behavior may differ from what you see.
- On a fresh install, confirm Korean/English toggling and the Hanja popup **in Notepad first**, then widen to other apps. That ordering isolates causes fastest.
- If something misbehaves, report it on [GitHub Issues](https://github.com/from104/unim/issues) with the **app name, your Windows version (`winver`), and whether the app is 32- or 64-bit**.
<!-- @endplatform -->

---

## Q4. How does AutoTypeFix actually work?

Two stages.

### Stage 1 — observation

The engine simulates two virtual tracks (Korean track, English track) for every keystroke regardless of mode. So in English mode, while `gksrmf` arrives, the Korean track is also producing `한글` in parallel.

### Stage 2 — trigger

At a word boundary (space, punctuation, Enter), if the *other* track has produced a meaningful word, propose a correction.

- forward: in English mode, Korean track formed letters → `gksrmf` → `한글`.
- reverse: in Korean mode, English track formed a word → `ㅈㅐㅍㅁ` → `wave`.

### Learning (suppression dictionary)

If the user rejects a correction with BS+mode-switch → marked Pending. The *next* time the same word triggers, the engine suppresses that attempt **and** registers it as Tentative. Pressing Confirm in the GUI promotes it to Confirmed (permanent).

> Key insight: registration happens at retrigger time, not rollback time. This keeps a one-off mode mistake from becoming a permanent suppression.

---

## Q5-1. When I un-bookmark a hanja, the popup jumps to a different page — is that intended?

**Yes, that is the designed behavior.** Bookmarking (★) promotes a hanja to the top of page 1; un-bookmarking (☆) demotes it back to its lexicographic home. If that home is on a different page, the popup follows.

To make sure you do not miss that jump, the destination cell **flashes Catppuccin yellow (`#f9e2af`) for 140 ms**. The flash answers "I just unstarred it — where did it go?" with a single visual signal.

- Bookmarking (★ on) does not flash: the cursor predictably lands at page 1 row 1, which is hard to miss.
- Un-bookmarking (☆ off) flashes: the destination is unpredictable, so the flash spotlights it.

For mechanics see [user manual §4.2](../user-guide/README.md#42-hanja-conversion) and [popup spec §3.6/§3.7](../../dev/specs/POPUP_SPEC.md).

---

## Q5. What's the difference between the 9-cell and 81-cell Hanja popup?

| Item | 9-cell (compact) | 81-cell (expanded) |
|------|----------------|------------------|
| Screen footprint | small | large |
| Visible candidates | 9 | 81 (9×9) |
| Best for | top candidate is the right one | rare hanja, many homophones |
| Toggle key | — | `.` (period) |
| Indicator | ⊟ icon | ⊞ icon |
| Bindings | 1–9 direct, arrows | 1–9 + row jump, arrows |

> With more than 9 candidates, 9-cell paginates with arrow keys; 81-cell unfolds nine pages at once for visual comparison.

---

## Q6. Where are the config files? Backup and restore?

<!-- @platform:linux -->
**🐧 Linux**

### Location

```
~/.config/unim/
├── config.yaml              # General settings (source of truth)
├── typefix-blacklist.yaml   # Learned suppressions
├── userdict.yaml            # Reverse user dict (new in 0.2.0)
└── layouts/                 # Custom v1 layouts
    └── my_3bul_variant.json
```

### Backup

```bash
tar -czf ~/unim-backup-$(date +%F).tar.gz -C ~/.config unim
```

### Restore

```bash
systemctl --user stop unim-daemon
tar -xzf ~/unim-backup-2026-04-26.tar.gz -C ~/.config
systemctl --user start unim-daemon
```

> The daemon hot-reloads `typefix-blacklist.yaml` and `userdict.yaml` on mtime change, so you can restore them without stopping the daemon. `config.yaml` caches some keys at start, so a restart is safer for it.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### Location

```
%APPDATA%\unim\
├── config.yaml                # General settings (source of truth)
├── typefix-blacklist.yaml     # Learned suppressions (Suppressed Words tab)
├── typefix-userdict.yaml      # Reverse user dictionary (User Dictionary tab)
└── layouts\                   # Custom v1 layouts
    └── my_3bul_variant.json
```

Paste `%APPDATA%\unim` into the Explorer address bar to open it. (Usually `C:\Users\<you>\AppData\Roaming\unim`.)

### Backup

```powershell
Compress-Archive -Path "$env:APPDATA\unim" -DestinationPath "$env:USERPROFILE\Desktop\unim-backup.zip"
```

### Restore

```powershell
Expand-Archive -Path "$env:USERPROFILE\Desktop\unim-backup.zip" -DestinationPath "$env:APPDATA" -Force
```

> Unlike Linux, **there is no daemon to stop and start.** Input is handled by the TSF module loaded inside each application, and those all read the same files in this folder. So editing a file directly takes effect **after about 2 seconds**, and anything you change in the settings window applies immediately. If a change does not seem to land, click away to another window and come back.
>
> Uninstalling (Q27) does not delete this folder, so a reinstall picks your settings back up.
<!-- @endplatform -->

---

## Q7. What layouts exist, and can I add my own?

### Built-in Korean layouts

`ko_2bulstd` (Dubeolsik standard), `ko_3bul390` (Sebeolsik 390), `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_anmatae` (Ahnmatae, chord/moachigi).

> Note: Qwerty Sebeolsik (`ko_3bul_qwerty`) is preserved as a research reference, not as a built-in.
> Copy `docs/references/keymaps/ko_3bul_qwerty_v2.json` to `~/.config/unim/layouts/ko_3bul_qwerty.json` to activate it as a user profile.

### English

`qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`.

### User-defined

<!-- @platform:linux -->
**🐧 Linux**

Drop a v1 schema JSON into `~/.config/unim/layouts/<name>.json` — daemon scans automatically. Use `inherits: "ko_3bul390"` to override only what you need.

```bash
unim-cli config layout validate ~/.config/unim/layouts/my.json
# activate
unim-cli config set korean-layout my
```
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Drop a v1 schema JSON into `%APPDATA%\unim\layouts\<name>.json` and the engine scans it automatically. The schema, `inherits`, and `rule_sets` are the exact same file format as on Linux, so a layout JSON written on Linux can be copied over as-is.

```powershell
# Create the folder if it does not exist
New-Item -ItemType Directory -Force -Path "$env:APPDATA\unim\layouts"
# Open it in Explorer and drop your JSON in
explorer "$env:APPDATA\unim\layouts"
```

- The `unim-cli` command-line tool is **not included in the Windows installer**, so the `validate` / `set` commands above are not available on Windows.
- Pick a layout in **Settings window → General tab → Layout**.
- Custom layouts are one of the less-exercised areas on the Windows side. If your layout does not appear in the list, a JSON syntax error is the likeliest cause — validating the file on Linux first with `unim-cli config layout validate` is the surest route.
<!-- @endplatform -->

Schema details: [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](../../archive/plans/LAYOUT_PROFILE_V1.md).

> Use `rule_sets` to bundle optional toggles with a layout. E.g. `ko_3bul390`'s `sun_arae_batchim`. The settings GUI dynamically renders a SwitchRow.

---

<!-- @platform:linux -->
## Q8. How much memory does UNIM use?

In normal operation, `unim-daemon` RSS sits in 30–80 MB. UNIM 0.2.0 hardens this:

- `#[global_allocator] tikv_jemallocator::Jemalloc` blocks the glibc ptmalloc arena explosion.
- `Environment=MALLOC_ARENA_MAX=2` in systemd (belt-and-suspenders for the C path).
- 60-second `libc::malloc_trim(0)` task forces memory release back to the OS.

> A previous incident saw RSS balloon to 2 GB on 0.1.x. Regression on those items is forbidden. If you observe RSS > 500 MB, see [troubleshooting §14](../troubleshooting/README.md#14-daemon-eats-too-much-memory-rss-500-mb).

---
<!-- @endplatform -->

## Q9. Does UNIM intercept passwords?

**No.** Password fields are detected via `content_purpose` and forced to English. AutoTypeFix (both forward and reverse), hanja conversion, and the special-char popup are all disabled. Any keystroke-observation buffer and undo history already accumulated are cleared too, so a password typed like `dkssud` is never auto-corrected into Korean and corrupted. The input is not retained in daemon memory.

> Caveat: automatic detection only works when the app accurately reports `content_purpose=password` (Linux) or `InputScope` (Windows). Environments that do not report it — **legacy XIM apps, and some Wayland compositors/web forms that do not send content-purpose** — may fail to auto-detect; verify English mode manually via the toggle key there. (Environments that detect correctly: GTK3/4, Qt5/6, GNOME extension, Windows TSF — both 64-bit and 32-bit apps are detected the same way, via the `unim_tsf32.dll` TSF TIP's `InputScope`. If you've seen a claim that "Windows IMM32 apps are detected on a best-effort basis for standard ES_PASSWORD controls only," that describes the IMM32 fallback, which is not actually shipped in the release — see Q11.)

---

## Q9-1. Why doesn't AutoTypeFix work in password fields?

**This is intended.** AutoTypeFix is deliberately disabled in password and PIN fields (see Q9), because otherwise a password typed like `dkssud` would flip to Korean at a word boundary and break your login. It returns to normal the moment you leave the field, and any on/off toggle state you set manually is preserved.

> Conversely, if correction fails in a **non-password field**, the cause is different → [Troubleshooting](../troubleshooting/README.md) §8. In the undetectable environments above (XIM, some Wayland), a password field is treated as a normal field and correction may in fact fire — that limitation is documented in Troubleshooting §8-1.

---

<!-- @platform:linux -->
## Q9-2. Is the one-line `install.sh` safe?

`curl -fsSL .../install.sh | bash` is convenient, but it does mean "run an entire unseen script," which can be unnerving. The UNIM installer has four safety guards:

1. **SHA256 checksum verification** — the `SHA256SUMS` shipped in the release acts as the manifest; every downloaded `.deb` is verified against it. On any mismatch it **aborts before the install step** (no partial install).
2. **Temp-directory isolation** — all downloads land in a `mktemp` working directory that a `trap` removes automatically on success, failure, or interrupt. Nothing is left on your system.
3. **apt transaction** — installation uses `apt install`, not `dpkg -i`, so even external runtime dependencies are resolved inside apt's atomic transaction; a failure leaves no partial install.
4. **Fully public script** — the script is published verbatim on the [main branch](https://github.com/from104/unim/blob/main/install.sh). You can download it first with `curl ... -o install.sh` and read it before running.

> Limitation: because `SHA256SUMS` lives at the **same origin (GitHub Releases)** as the `.deb`s, transfer integrity is guaranteed but origin authenticity relies on trusting GitHub's TLS. GPG/minisign signing is future work. For minimal trust, use Method 2 (manual download) and verify each file yourself.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q10. Migration notes from 0.1.x to 0.2.0?

Mostly automatic. Two normalizations:

- `korean.layout` written as an enum (`Dubeolsik`) is auto-converted to a string (`ko_2bulstd`).
- `english.layout` written as an enum is auto-converted (`qwerty`, etc.).
- `typefix-blacklist.yaml`'s old keys go through a serde compat layer.

C API: `UnimEnglishLayout` / `UnimKoreanLayout` enums removed → setters/getters now take/return C strings. Affects only C/C++ clients.

Full migration: [release notes](../release-notes/0.2.0/RELEASE_NOTES.md).

---
<!-- @endplatform -->

## Q11. Does UNIM run on macOS / Windows?

**Windows: yes. macOS: not yet.**

As of v0.4.0, Windows 10/11 (64-bit) is supported via `unim-tsf`, built on the Text Services Framework (TSF).

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

downloads and installs the MSI (see [user manual §2.1 Install](../user-guide/README.md#21-install)). 32-bit apps are handled by a separate 32-bit TSF TIP (`unim_tsf32.dll`) rather than the 64-bit TSF path. The IMM32 fallback explored earlier (the `unim-imm32` crate) remains only as diagnostic/research source that is **not included in the shipped MSI** — if you've seen documentation advertising an "IMM32 fallback" as a shipped feature, it's out of date.

The Windows side is in daily production use on the maintainer's machine and is refined from that use, but it has not been through the same breadth of machines and applications as Linux. If you hit a problem, please report it on [GitHub Issues](https://github.com/from104/unim/issues) with the app name and your Windows version (`winver`).

macOS is still not started (roadmap stage 5). Because the Rust core and C-API are separated, in principle an adapter for macOS's IMKit is feasible, but nobody has started it yet. Volunteers welcome.

---

## Q12. Why does the build need cargo 1.95?

`Cargo.lock` is in v4 format. cargo 1.83+ handles it safely (1.95 is the version we verify). Some distros ship `/usr/bin/cargo` as 1.75; explicitly use rustup:

```bash
rustup update stable
which cargo                  # should be ~/.cargo/bin/cargo
cargo --version              # 1.95.0+
```

The error `lock file version 4 requires '-Znext-lockfile-bump'` is always this issue.

---

## Q13. I'd like to contribute — where to start?

1. [`CONTRIBUTING.md`](../../../CONTRIBUTING.md) — branch/PR workflow.
2. [`AGENTS.md`](../../dev/architecture/AGENTS.md) — architecture and component map.
3. [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — behavior spec.
4. Per-crate `SPEC.md`.
5. Verify: `make build` warning-free + `cargo test --workspace` all pass.
6. Convention: commit messages in English, docs in Korean.

`good-first-issue` labels are the best entry point.

---

## Q14. Why "Universal" in the name?

**Universal Next-generation Input Method.** Although it is a Korean IME, "universal" stands for (a) bidirectional Korean ↔ English handling and (b) one core plugged into every toolkit. In the long term, also macOS/Windows (roadmap stage 5).

---

---

## Q15. What is the difference between Ahnmatae and Moachigi?

**Ahnmatae** (안마태) is a specific keyboard layout. Finalized in 2003, it is a three-beol (sebeolsik) style layout with fixed cho/jung/jong regions mapped to distinct keyboard zones. In UNIM it ships as the `ko_3bul_anmatae` built-in profile.

**Moachigi** (모아치기, "gather-and-strike") is an input method. Multiple jamo pressed simultaneously or within a short window are combined into a single syllable. Unlike ordinary dubeolsik/sebeolsik which process one key at a time, moachigi collects all keys within the chord window (default 60 ms) and resolves them at expiry.

In UNIM 0.3.0, Ahnmatae is the first built-in layout that supports moachigi. The moachigi settings group in the GTK settings dialog appears only when the selected layout has `supports_moachigi=true`.

---

<!-- @platform:linux -->
## Q16. How do I diagnose a missing popup-service?

First check whether `unim-popup-service` is available on the bus:

```bash
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup
```

No response means the package is not installed or the D-Bus service file is missing. See [troubleshooting §16](../troubleshooting/README.md#16-popup-service-debugging-030) for the full diagnosis flow.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q17. Will my settings survive a deb to rpm (or rpm to deb) migration?

Yes. All user data lives under `~/.config/unim/` and is independent of the package format:

- `~/.config/unim/config.yaml` — main settings
- `~/.config/unim/layouts/*.json` — custom keyboard profiles
- `~/.config/unim/typefix-blacklist.yaml` — AutoTypeFix suppression dictionary
- `~/.config/unim/typefix-userdict.yaml` — user dictionary

Uninstalling one package format and installing the other leaves these files untouched. Note that the `unim-gui-qt` package was removed in 0.3.0 — the tray icon, settings window, and popup renderer are now split between `unim-desktop` (indicator + legacy settings dialog + `unim-popup-service`, bundled together) and `unim-settings` (the Slint settings app). For the current full list of 11 packages, check `debian/control` or `dpkg -l 'unim*'`.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q19. Is there a tool to view, edit, or practice keyboard layouts?

Two GTK4 companion tools ship alongside UNIM.

- **`unim-keymap-studio` (Keymap Studio)**: view and edit Korean/English layouts visually.
  A three-stage header dropdown (language > source > layout) selects the target, and four tabs
  (Basic / Keymap / Combos / Extended) show the content. The **Combos** and **Extended** tabs
  appear only for Korean layouts. The header's right side holds [Help] / [Settings] / [Menu].
  - **Built-in layouts** are read-only, so only "Save As" is available; **user layouts** support
    both "Save" and "Save As".
  - User layouts are written as JSON under `~/.config/unim/layouts/` (same location as the
    user-defined layouts in Q7).
- **`unim-typing-practice` (Typing Practice)**: practice typing with the currently active layout.
  It measures WPM/CPM, accuracy, and a typo heatmap so you can see which keys you mistype most.

Both tools share the same five-row keyboard widget, so the layout looks consistent across them.
For shortcuts see [user manual §5.6](../user-guide/README.md#56-keyboard-layout-tools-keymap-studio--typing-practice).

---
<!-- @endplatform -->

## Q18. What is the right chord_window_ms value?

Valid range: **10–200 ms**. Default: **60 ms** (tuned for experienced typists).

| Profile | Recommended range | Notes |
|---------|------------------|-------|
| Beginner | 100–150 ms | Chord timing still inconsistent; extra window prevents missed chords |
| General | 60–100 ms | Comfortable for most users |
| Expert | 10–60 ms | Minimize false positives, maximize responsiveness |

Set to `0` to disable moachigi entirely.

<!-- @platform:linux -->
**🐧 Linux** — adjust via the settings dialog slider or:

```bash
unim-cli config set korean-chord-window-ms 80
```
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — adjust with the slider under **Settings window → General tab → Moachigi**. The `unim-cli` command-line tool is not included in the Windows installer, so there is no CLI route; editing `%APPDATA%\unim\config.yaml` directly works too (takes effect after about 2 seconds).
<!-- @endplatform -->

## Q20. When I hold a key too long, the same letter is typed several times.

If you hold a key down (e.g. because of a tremor), the OS re-fires the same key rapidly (auto-repeat). UNIM has a **Suppress Composition Key Auto-repeat (accessibility)** option that makes the daemon ignore those repeats. Suppression applies to the **Korean/English toggle key and character keys in Korean mode**; repeats of editing keys (Backspace, arrows) and direct English typing are left alone. The default is off, so nothing changes until you enable it.

<!-- @platform:linux -->
**🐧 Linux**

To enable it — the **Accessibility → Suppress Composition Key Auto-repeat** switch in the settings app, or the CLI:

```bash
unim-cli config set ignore-key-repeat true
```

**Fallback limits**: Wayland, Qt5/6, and the GNOME extension detect repeats precisely. The GTK3/4, XIM, and ibus-compatible paths approximate with an 80 ms time window, so (1) the first repeat may slip through, and (2) if your system key-repeat interval is set longer than 80 ms, repeats may not be filtered. In either case, when in doubt it errs toward suppressing less (fail-safe). GNOME extension users: applies after re-login.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

To enable it — turn on Suppress Composition Key Auto-repeat in the **settings window**. If opening the settings window is awkward, editing `%APPDATA%\unim\config.yaml` works just as well (takes effect after about 2 seconds).

```yaml
engine:
  ignore_key_repeat: true
```

It behaves **the same way as on Linux** — only the Korean/English toggle key and character keys in Korean mode are covered; repeats of editing keys (Backspace, arrows) and direct English typing are left alone. The default is off.

> If repeats still get through with this on, also slow down Windows' own key repeat settings (**Control Panel → Keyboard → Repeat delay / Repeat rate**). The two settings are independent, so using both together is fine.
<!-- @endplatform -->

---

<!-- @platform:linux -->
## Q21. Password fields in Chrome/Chromium are not being auto-protected. Why?

**Chrome does not report input field types to the input method, so UNIM cannot auto-detect them.**

### Diagnosis by cause

#### 1. Wayland Chrome (native, `--enable-wayland-ime` not enabled)

By default, Chrome does not use the Wayland input method protocol (`input-method-v2`). You must enable the flag explicitly.

**Solutions:**

- **Option 1 — Command-line flag**
  ```bash
  google-chrome --enable-wayland-ime
  chromium --enable-wayland-ime
  ```

- **Option 2 — Flag file** (`~/.config/chrome-flags.conf`)
  ```
  --enable-wayland-ime
  ```

- **Option 3 — .desktop entry** (all users, persists across package upgrades)
  ```bash
  # Find /usr/share/applications/google-chrome.desktop or ~/.local/share/applications/google-chrome.desktop
  # Locate the Exec= line and append --enable-wayland-ime
  Exec=/opt/google/chrome/google-chrome --enable-wayland-ime %U
  ```

After enabling the flag and restarting Chrome, UNIM will detect password fields.

#### 2. X11 Chrome / Chromium

The Chromium engine **does not report input field types to the input method even on X11.** This is a design choice in Chromium that UNIM cannot override. Manually verify English mode by pressing the Hangul/Korean toggle key before entering the password.

> Alternative: Firefox reports field info to the input method, so detection works correctly.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q22. XIM environments: why doesn't password-field auto-detection work?

**The XIM protocol itself has no way to convey input field semantics.**

XIM (X Input Method) is a legacy protocol from 1994 with no facility to signal field types like "password". Your options:

1. **Manual English mode check** — before entering a password field, press Hangul/Korean to verify English mode.
2. **Switch to GTK/Qt apps** — migrate from XIM-only legacy apps to modern GTK/Qt equivalents (e.g., gvim → vim-gtk / nvim-qt).
3. **Try an alternative input path** — for command-line use, also enable the ibus-compat path and test.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q23. I removed UNIM and now Korean/English toggling doesn't work at all — how do I go back to another IME?

**Removing UNIM does not automatically restore the system IME setting.** If you removed IBus with `sudo apt remove ibus` while installing UNIM (as Q2 recommends), the "current IME = unim" setting made by `im-config -n unim` (or by enabling the GNOME+Wayland extension) survives even after you remove the UNIM package. Log back in after that and you're left with `run_im unim` pointing at a binary that no longer exists — **no IME starts at all, and Korean/English toggling stops working entirely.**

### Before removing UNIM (recommended)

Switch back to another IME **before** removing UNIM.

```bash
# Example: install ibus-hangul first if you want to go back to it
sudo apt install ibus ibus-hangul

# Point im-config back
im-config -n ibus
# Or let it auto-pick from what's installed
im-config -n auto

# Now remove UNIM (all packages at once — shell glob)
sudo apt remove 'unim*'
```

### If you already removed it and Korean input is completely broken

1. Run `im-config -n auto` to auto-reassign to whatever IME is installed. If nothing is installed, `sudo apt install ibus ibus-hangul` first, then rerun it.
2. Check `~/.xinputrc` directly — if `run_im unim` is still there, delete it or replace it with another IME's name (on a GNOME+Wayland session this file may not be used at all — see [user manual §2.2](../user-guide/README.md#22-environment-variables-any-desktop-without-gnome-extension)).
3. Log out and back in.

> This removal/rollback path is managed by Debian/Ubuntu's `im-config` framework, not by UNIM's package scripts, so UNIM cannot automatically revert it. The manual steps above are currently the only recovery path.
<!-- @endplatform -->

<!-- @platform:windows -->
## Q24. I installed UNIM but it does not appear in the input method list.

**🪟 Windows** — **Not appearing right after install is normal.** A TSF (Text Services Framework — Windows' input method architecture) input method is loaded by the OS when a session starts, so you have to **reboot, or sign out and sign back in**, before UNIM shows up in the list.

If it is still missing after signing back in, work down this list.

1. **Check that Korean is installed** — UNIM attaches underneath **Korean** in Settings → Time & language → Language & region. If Korean is not there, add it first.
2. **Add the keyboard manually** — Settings → Time & language → Language & region → **Korean → Keyboards → Add a keyboard** → `UNIM Korean IME`.
3. **Re-register the input method** — in the install folder (`C:\Program Files\UNIM\`), right-click `register-tsf.bat` and choose **Run as administrator**. To undo, use `unregister-tsf.bat` in the same folder. Both scripts touch input method registration only; they never delete files or settings.

> Once it is attached, the taskbar shows your current mode next to the clock — `한` for Korean, `A` for English. **Left-click toggles Korean/English**; **right-click opens the menu** (toggle Korean/English · Set as default IME · Open settings).

---

## Q25. Windows shows a "Windows protected your PC" warning during install.

**🪟 Windows** — The MSI we ship right now is **not code-signed**, so SmartScreen treats it as coming from an unknown publisher and warns you. To continue, click **More info → Run anyway**.

- Code signing is planned, but no certificate has been obtained yet. And note that signing does **not** make the warning disappear immediately — SmartScreen reputation accrues as downloads accumulate, so a new publisher keeps seeing the warning for an initial period.
- Until then, verify integrity by **comparing the SHA256 hash** against `SHA256SUMS-msi`, which is published alongside the MSI in the release.

  ```powershell
  Get-FileHash .\unim-0.4.1-x64.msi -Algorithm SHA256
  ```

- If you install with `install.ps1`, the script does this comparison for you — twice (Q26).

---

## Q26. Is the one-line `install.ps1` safe?

**🪟 Windows** — `irm ... | iex` raises exactly the same concern as `curl | bash` on Linux. The Windows installer script has these guards:

1. **SHA256 verified twice** — once right after download, against the release's `SHA256SUMS-msi`, and again **immediately before running the installer** after elevating to administrator. A mismatch at either point **aborts without installing anything**.
2. **Least privilege** — the script **starts non-elevated**, does the download and verification there, and elevates via UAC (the administrator consent prompt) only for the MSI execution step. You get exactly one UAC prompt.
3. **Unblocking happens after verification** — the mark-of-the-web (the "downloaded from the internet" block Windows attaches to files) is removed only from a file that already passed the first check.
4. **The script is fully public** — it lives verbatim on the [main branch](https://github.com/from104/unim/blob/main/install.ps1). You can download and read it before running, and you can pin the exact version you want.

   ```powershell
   # Inspect first, then run
   powershell -ExecutionPolicy Bypass -File .\install.ps1 -Check
   # Pin a specific version
   $env:UNIM_VERSION='v0.4.1'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
   ```

`-Check` (report installed vs latest version, change nothing), `-Update`, and `-Force` are available.

> Limitations: the MSI itself is unsigned, so the SmartScreen warning still appears (Q25). And because `SHA256SUMS-msi` lives at the **same origin (GitHub Releases)** as the MSI, transfer integrity is guaranteed but origin authenticity relies on trusting GitHub's TLS — the same limitation as on Linux.

---

## Q27. How do I completely uninstall UNIM on Windows?

**🪟 Windows** —

1. **Close every app you have been typing in first.** The input module is loaded inside any app where you typed Korean, and if those are open, file removal can get deferred to the next reboot.
2. **Settings → Apps → Installed apps** → remove `UNIM`. The Start menu shortcut **UNIM → Uninstall UNIM** does the same thing.
3. When it finishes, **sign out and back in, or reboot**.

Uninstalling removes the install folder (`C:\Program Files\UNIM\`), the input method registration, and the Start menu shortcuts. However, **your settings (`%APPDATA%\unim\`) stay** — reinstall and your config, suppression dictionary, and user dictionary carry over. Delete that folder by hand if you want a clean slate.

> There is no separate "restore the system IME assignment" step like on Linux (Q23), because on Windows UNIM was only **added** to the input method list and never displaced anything. If Korean input behaves oddly after removal, check the remaining entries under **Settings → Time & language → Korean → Keyboards**.

---

## Q28. The Hanja / special character / emoji popup does not appear.

**🪟 Windows** — On Windows the popup is drawn by a **separate program**, `unim-popup-win.exe`. If that is not running you get exactly this symptom: typing Korean works fine, only the popup is missing.

1. **Check the key first** — the `Hanja` key (or `F9`) branches three ways depending on state:
   - while composing a syllable → **Hanja** candidates
   - after composing a single consonant (e.g. `ㅁ`) → **special characters**
   - while not composing anything → **emoji**
2. **Check the program is running** — look for `unim-popup-win.exe` in Task Manager. If it is missing, run `unim-popup-win.exe` from the install folder (`C:\Program Files\UNIM\`) directly. Launching it twice by accident is harmless — the second copy exits on its own.
3. **Sign out and back in once** — the program is registered to start automatically at login, so a re-login is the surest fix right after installing.

> The popup key bindings (9×9 grid toggle `.`, bookmark `Space`, column jump `Q`–`O`, category `A`–`L`) are matched to the Linux behavior. If something behaves differently, please tell us on [GitHub Issues](https://github.com/from104/unim/issues).

---

## Q29. In MS Word, text commits by word instead of by character.

**🪟 Windows** — **This is intended.** Apps differ in how they handle in-progress composition, so in some apps UNIM commits **per word** rather than per character (smart commit unit). The default targets are `winword.exe` (MS Word) and `wmux.exe`.

- To use the same behavior in another app, add its executable name under **"Word mode apps"** in the settings window. This setting exists only on Windows.
- Conversely, remove `winword.exe` from that list if you prefer per-character composition in Word.

---

## Q30. Does it work in 32-bit apps like KakaoTalk and Hancom, or in WezTerm and Telegram?

**🪟 Windows** — Yes.

- **32-bit apps**: 64-bit apps are served by `unim_tsf.dll`, and 32-bit apps by a separate 32-bit input method, `unim_tsf32.dll`. The installer registers both, so Korean input works in 32-bit-only apps such as KakaoTalk and Hancom.
- **Console / IMM32-style apps**: apps where Hangul composition used to break — WezTerm, Telegram — were fixed in v0.4.0 to follow the CUAS (Windows' legacy input compatibility layer) contract, and now compose correctly.
- The **IMM32 `.ime` registration route explored earlier has been dropped.** If you have seen documentation advertising an "IMM32 fallback" as a shipped feature, it is out of date (see Q11).

If it fails in one specific app, include the **app name and whether it is 32- or 64-bit** in a [GitHub Issues](https://github.com/from104/unim/issues) report — that makes diagnosis far faster.

---

## Q31. Does the Windows build have the same features as the Linux build?

**🪟 Windows** — **The input core is identical; the surrounding tooling differs.** Hangul composition, layout selection, the Hanja / special character / emoji popups, AutoTypeFix (forward, reverse, and learning), suppressed words, and the user dictionary all run on the **same Rust core**. This table is the complete list of differences.

| Item | Linux | Windows |
|------|-------|---------|
| Settings window | settings app (`unim-settings`) | settings app (`unim-settings.exe`) — General / Typo Correction / Suppressed Words / User Dictionary, 4 tabs |
| Applying settings | daemon propagates immediately | immediate from the settings window; ~2 seconds when you edit the file directly |
| `unim-cli` command-line tool | yes | **no** (not included in the installer) |
| Keymap Studio · Typing Practice | yes | **no** |
| GNOME extension | yes | not applicable |
| Current mode indicator | tray indicator | taskbar input indicator (`한` / `A`) |

- **The Windows edition has a shorter track record than the Linux one.** Fewer applications have been exercised, so with uncommon apps the documented behavior may differ from what you observe.
- The offline manual (this document) ships with the installer — open it any time from the **Help button in the settings window**.
- To carry settings over from Linux, copy the files in `~/.config/unim/` to `%APPDATA%\unim\`. The file formats are identical on both platforms (Q6 · Q7).

<!-- @endplatform -->

---

## Read more

- [User manual](../user-guide/README.md)
- [Troubleshooting](../troubleshooting/README.md)
- [Release notes 0.4.1](../release-notes/0.4.1/README.en.md)
- [Release notes 0.4.0](../release-notes/0.4.0/README.en.md)
- [Release notes 0.3.0](../release-notes/0.3.0/README.en.md)
- [Release notes 0.2.0](../release-notes/0.2.0/RELEASE_NOTES.md)
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md)
- [`POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)
