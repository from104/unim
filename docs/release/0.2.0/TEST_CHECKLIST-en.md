# UNIM 0.2.0 — Manual Test Checklist (English)

> Reproducible scenarios the user can execute by hand. Each scenario ends with a `[ ] PASS / FAIL` box.
> Time estimates are conservative (a 3-minute task is listed as 5 min). All commands are copy-paste ready.
>
> Regression scenarios in §14 map 1:1 to the `[0.2.0] Fixed` entries.
> Automation coverage: see [`TEST_AUTOMATION.md`](TEST_AUTOMATION.md). Troubleshooting: [`TROUBLESHOOTING.md`](TROUBLESHOOTING.md).

---

## 0. Prerequisites (10 min)

```bash
export PATH=$HOME/.cargo/bin:$PATH        # ensure cargo 1.95.0
cd /home/from104/work/unim
```

- [ ] `cargo --version` → `cargo 1.95.0 (...)`
- [ ] `make build` succeeds with **zero warnings**
- [ ] `cargo test --workspace` all PASS
- [ ] `sudo make install PREFIX=/usr` (or `make deb` → `sudo dpkg -i`)
- [ ] `systemctl --user daemon-reload && systemctl --user start unim` → `is-active` reports `active`
- [ ] `pgrep -a unim-daemon` shows exactly one PID
- [ ] Log reset: `: > ~/.unim-errors.log`
- [ ] Start in dev mode: `pkill -9 unim-daemon; UNIM_DEVELOP=1 /usr/libexec/unim-daemon -n --replace &`

**On failure**: `journalctl --user -u unim -b --no-pager | tail -50`, inspect `~/.unim-errors.log` last 100 lines.

---

## 1. unim-daemon

### [unim-daemon] Start / restart / clean shutdown — 5 min
**Precondition**: §0 prereqs done
**Steps**:
1. `systemctl --user restart unim`
2. `pgrep -a unim-daemon` → exactly one PID
3. `kill -TERM $(pgrep unim-daemon)` then `journalctl --user -u unim --since '1 min ago'`
4. `systemctl --user start unim`

**Expected**: No panic on shutdown, no `thread 'main' panicked`. After restart `busctl --user list | grep org.atit.unim` shows the service.
**Diagnostics**: `journalctl --user -u unim -p err`, `~/.unim-errors.log`.
- [ ] PASS / FAIL

### [unim-daemon] RSS / arena leak regression — 10 min
**Precondition**: jemalloc + `MALLOC_ARENA_MAX=2` active (`cat /proc/$(pidof unim-daemon)/environ | tr '\0' '\n' | grep MALLOC`)
**Steps**:
1. Record baseline: `grep VmRSS /proc/$(pidof unim-daemon)/status`
2. For 5 min, type Korean in a GTK4 textview and switch focus 50 times
3. Record RSS again

**Expected**: Δ RSS < 30 MB. Anonymous arenas ≥ 64 MB ≤ 2 (see AGENTS.md memory diagnostics).
- [ ] PASS / FAIL

---

## 2. unim-cli

### [unim-cli] --help / locale — 1 min
**Steps**:
1. `LANG=ko_KR.UTF-8 unim-cli --help` → Korean help
2. `LANG=en_US.UTF-8 unim-cli --help` → English help
3. `unim-cli --version` → `0.2.0`

**Expected**: Locale auto-switch, no mojibake.
- [ ] PASS / FAIL

### [unim-cli] convert — 1 min
**Steps**:
1. `unim-cli convert --to-hangul "dkssudgktpdy"` → `안녕하세요`
2. `unim-cli convert --to-english "안녕"` → ASCII roundtrip

- [ ] PASS / FAIL

### [unim-cli] config show / set / path / reset — 5 min
**Steps**:
1. `unim-cli config path` → emits `~/.config/unim/config.yaml`
2. `unim-cli config show`
3. `cp ~/.config/unim/config.yaml /tmp/unim-config.bak`
4. `unim-cli config set engine.auto_typefix.enabled false`
5. `unim-cli config show | grep -A1 auto_typefix` → `enabled: false`
6. Restore: `unim-cli config set engine.auto_typefix.enabled true`
7. (Optional) `unim-cli config reset` then `cp /tmp/unim-config.bak ~/.config/unim/config.yaml`

**Expected**: Change reflected in GUI within 1 s; daemon hot-reloads via mtime.
- [ ] PASS / FAIL

### [unim-cli] config layout list / describe / validate — 3 min
**Steps**:
1. `unim-cli config layout list` → 10 built-in + user profiles
2. `unim-cli config layout describe ko_3bul_qwerty`
3. `unim-cli config layout validate src/keystroke/keymap/ko_3bul390.json` → exit 0
4. (Negative) Validate a malformed JSON → exit 2

- [ ] PASS / FAIL

---

## 3. unim-dbus

### [unim-dbus] busctl introspect — 2 min
**Steps**:
1. `busctl --user list | grep org.atit.unim`
2. `busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod | head -30`

**Expected**: `ProcessKeyEvent`, `FocusIn/Out`, `GetHanjaCandidates`, `SelectHanja`, `HanjaBookmarkChanged` signal exposed.
- [ ] PASS / FAIL

### [unim-dbus] make test-dbus — 2 min
**Expected**: introspection prints, no `unim service missing` warning, daemon cleanly killed at end.
- [ ] PASS / FAIL

### [unim-dbus] re-entrancy regression — 5 min
**Precondition**: GNOME Shell + Wayland (re-entrancy reproducer)
**Steps**:
1. In gedit/Discord, `매`+F9 to open hanja popup
2. While popup is up, press `1`
3. Repeat with `매`+F9 → `2`

**Expected**: No key lock or drop. (Key-queue re-entrancy guard in `key_handler.js` working.)
- [ ] PASS / FAIL

---

## 4. unim-gui-gtk

### [unim-gui-gtk] Start + theme follow — 2 min
**Steps**: Launch `unim-gui-gtk`, toggle GNOME dark theme; dialog flips light/dark.
- [ ] PASS / FAIL

### [unim-gui-gtk] Walk every page — 10 min
Click every page, toggle every SwitchRow / SpinRow / ComboRow / Scale:
1. **General** — toggle keys, startup mode
2. **Korean Layout** — `ko_2bulstd → ko_3bul390 → ko_3bul_qwerty`; rule_sets switch rows refresh
3. **English Layout** — `qwerty / dvorak / colemak / colemak_dh / workman`
4. **AutoTypeFix** — switches + `tentative_expiry_hours` Scale (1..=12)
5. **Suppression Words** — Tentative/Confirmed/Inactive groups
6. **Hanja** — 9 vs 81 grid default + bookmarks
7. **Reverse Dict** — add/remove user entry
8. **Per-app Rules**
9. **About**

After each change run `unim-cli config show | grep <key>` to confirm persistence.
- [ ] PASS / FAIL

### [unim-gui-gtk] Suppression Words row actions — 5 min
**Steps**:
1. In English mode type `the` → triggers a forward correction
2. Backspace + toggle Korean/English → flags as pending rollback
3. Type `the` again → instantly suppressed; appears in Tentative group
4. Click `Confirm` → row moves to Confirmed; daemon hot-reloads

**Expected**: Subsequent `the` no longer corrects.
- [ ] PASS / FAIL

---

## 5. unim-gui-qt

### [unim-gui-qt] QML walkthrough — 10 min
**Precondition**: `qt6-base-dev qt6-declarative-dev` installed; `unim-gui-qt` deployed
**Steps**: Launch and traverse every page (General/Layout/AutoTypeFix/Hanja/About). Compare values to `unim-cli config show`.
- [ ] PASS / FAIL

---

## 6. unim-frontends/xim

### [xim] xterm Korean input — 5 min
**Precondition**: X11 session, env: `XMODIFIERS=@im=unim GTK_IM_MODULE=xim QT_IM_MODULE=xim`; `unim-xim` running
**Steps**:
1. `pgrep -a unim-xim` (start if missing: `/usr/libexec/unim-xim &`)
2. `XMODIFIERS=@im=unim xterm`
3. Toggle Korean, type `dkssudgktpdy` → `안녕하세요`
4. `매` + F9 → hanja popup → `1`

**Expected**: Inline preedit, popup at caret.
- [ ] PASS / FAIL

### [xim] AutoTypeFix N+1 BS regression — 5 min
**Steps**:
1. xterm in English mode, type `dks`
2. Toggle Korean → forward AutoTypeFix
3. Result `안` rendered cleanly with no stray BS

**Expected**: Multi-char correction works under the N+1 BS protocol. (Chrome preedit edge case is a known SKIP.)
- [ ] PASS / FAIL

### [xim] Emacs / terminal — 3 min
- [ ] PASS / FAIL

---

## 7. unim-frontends/wayland

### [wayland] weston-text-input-demo — 5 min
**Precondition**: Pure Wayland compositor (Weston / sway), `unim-wayland` running
**Steps**:
1. `weston-text-input-demo &`
2. Type Korean → inline preedit
3. Switch focus → preedit auto-commits, new context starts cleanly

**Expected**: Honors §2.2 / §8.3 sequences. Pure-Wayland hanja popup is a known unsolved item — SKIP.
- [ ] PASS / FAIL

---

## 8. GTK3 IM module

### [im-gtk3] gedit / gtk3-demo golden path — 5 min
**Precondition**: `GTK_IM_MODULE=unim`; `im-unim.so` installed under `gtk-3.0/3.0.0/immodules`
**Steps**:
1. `GTK_IM_MODULE=unim gtk3-demo` → "Text View"
2. `dkssudgktpdy` → `안녕하세요`
3. Single Space → exactly one space committed (regression: 552b5bd English-mode space drop)
4. `매` + F9 → popup at caret
5. Period toggles 9 ↔ 81 grid (⊞/⊟ icon updates)
6. Space on a candidate → bookmark ☆/★ toggles

**Expected**: §3.4 bindings work. Bookmark toggle live-refreshes other GTK4/Qt popups via `HanjaBookmarkChanged`.
- [ ] PASS / FAIL

### [im-gtk3] preedit-end keylock regression — 3 min
**Steps**: In ghostty (or another GTK3 terminal) type Korean, press Enter, then immediately type `a` — no key lock.
**Expected**: `preedit-end` emitted via `unim_emit_preedit` helper.
- [ ] PASS / FAIL

---

## 9. GTK4 IM module

### [im-gtk4] gedit / gnome-text-editor — 5 min
**Steps**:
1. `GTK_IM_MODULE=unim gnome-text-editor`
2. `dkssudgktpdy` → `안녕하세요`
3. Click another window (focus-out) → `늘` is **not** committed twice (regression: dup-CommitText fix)
4. Repeat hanja popup + bookmark + 9×9 toggle from §8

**Expected**: Single commit on focus-out. Live bookmark sync.
- [ ] PASS / FAIL

### [im-gtk4] surrounding-text reverse correction — 3 min
**Steps**: Type Korean word → toggle Korean/English → reverse correction succeeds (uses `request_surrounding`).
- [ ] PASS / FAIL

---

## 10. Qt5 IM module

### [im-qt5] qt5 test app — 5 min
**Steps**: `make sandbox-qt5`; type Korean; verify hanja popup + 81-grid toggle.
- [ ] PASS / FAIL

---

## 11. Qt6 IM module

### [im-qt6] qt6 test app — 5 min
**Steps**: `make sandbox-qt6`; same scenarios as Qt5.
- [ ] PASS / FAIL

---

## 12. unim-gnome-extension

### [gnome-ext] Activation + indicator — 3 min
**Precondition**: GNOME 45+ Wayland session, `make dev-extension`, log out/in
**Steps**:
1. `gnome-extensions list --enabled | grep unim`
2. Top panel shows Korean/English indicator
3. Click → toggles, icon updates immediately

**Expected**: `GlobalModeChanged` signal handled.
- [ ] PASS / FAIL

### [gnome-ext] prefs.js options — 3 min
**Steps**: `gnome-extensions prefs unim@from104.github.io`
- [ ] All 5 GNOME-only options shown and persist
- [ ] No dead-feature options (Phase 8 cleanup)
- [ ] PASS / FAIL

### [gnome-ext] Hanja popup Push mode — 5 min
**Precondition**: Wayland + GNOME, `popup_mode = Standalone`
**Steps**:
1. In Firefox/Discord (native Wayland): `매`+F9
2. GNOME extension renders push-style popup (St widget)
3. Verify `1`-`9`, Period, Space all functional

**Expected**: §3.4 bindings work in Push mode; popup auto-closes on focus change.
- [ ] PASS / FAIL

### [gnome-ext] Emoji popup (Super+.) — 3 min
**Steps**: `Super+.` → emoji popup; switch tabs; search `smile`; commit with Enter.
**Expected**: MRU favorites tab updates after commit.
- [ ] PASS / FAIL

---

## 13. unim-windows / unim-tsf (optional)

### [windows-tsf] Cross-compile check — 3 min
**Steps**: `WIN_TARGET=x86_64-pc-windows-gnu make check-windows`
**Expected**: 0 warning / 0 error.
- [ ] PASS / FAIL

### [windows-tsf] Notepad input (optional, Windows VM) — 10 min+
**Steps**: `make build-windows`, copy artifacts, install on Windows, type Korean in Notepad with hanja conversion.
- [ ] PASS / FAIL

---

## 14. Regression matrix (mapped to 0.2.0 Fixed)

| ID | Issue | Verified by | Result |
|----|-------|-------------|--------|
| R1 | English-mode Space dropped (gedit) | §9 GTK4 golden | [ ] |
| R2 | Focus-out duplicate commit `늘늘` | §9 GTK4 focus-out | [ ] |
| R3 | tentative_expiry days → hours | §4 Suppression Words | [ ] |
| R4 | gedit surrounding-text reverse fix | §9 GTK4 surrounding | [ ] |
| R5 | GTK preedit-end keylock | §8 GTK3 ghostty | [ ] |
| R6 | XIM AutoTypeFix N+1 BS | §6 XIM AutoTypeFix | [ ] |
| R7 | Reverse blacklist empty-string register | §4 Suppression rows | [ ] |
| R8 | DBus call_sync re-entrancy | §3 DBus re-entrancy | [ ] |
| R9 | RSS leak (jemalloc + ARENA_MAX) | §1 RSS regression | [ ] |

---

## 15. Environment matrix (manual only)

For each combination, run §8 + §9 + §10/§11 + §12 golden paths once.

| Combo | gedit Korean | Hanja popup | AutoTypeFix | Result |
|-------|--------------|-------------|-------------|--------|
| X11 + GNOME (Xorg) | [ ] | [ ] | [ ] | [ ] |
| X11 + KDE Plasma | [ ] | [ ] | [ ] | [ ] |
| Wayland + GNOME | [ ] | [ ] | [ ] | [ ] |
| Wayland + KDE Plasma | [ ] | [ ] | [ ] | [ ] |

---

## 16. Final cleanup

- [ ] `~/.unim-errors.log` has zero ERROR/PANIC entries
- [ ] `journalctl --user -u unim -p err -b` is empty
- [ ] `pgrep -a unim-` shows only intended daemons (no zombies/dupes)
- [ ] `git status` is clean (no incidental edits during testing)
- [ ] Summary recorded in `_workspace/release/01_test_plan_report.md`
