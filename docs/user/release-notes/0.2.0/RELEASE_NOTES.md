# UNIM 0.2.0 Release Notes (English)

> Release date: 2026-04-26
> Codename: "Phase 8 cleanup + AutoTypeFix completion"
> About five days of focused cleanup and feature integration since 0.1.0 (2026-04-21).

---

## One-line summary

AutoTypeFix reached practical maturity with a learning-style suppression dictionary and a user dictionary; the Hanja popup gained 9-cell ↔ 81-cell toggle and bookmarks; built-in keyboard layouts migrated to a v1 JSON profile system that supports user-layout inheritance and optional rule-set toggles.

---

## Added

### 1. Layout Profile v1 (spec + engine + config + CLI + GUI)

Built-in layouts moved to self-contained v1 JSON files in `src/keystroke/keymap/*.json`, replacing the hybrid Rust-const + partial-JSON path.

- **User profiles**: drop a v1 JSON into `~/.config/unim/layouts/*.json`; daemon scans on startup with mtime-based hot reload.
- **`inherits` chain resolution**: child profiles declare `"inherits": "base_name"`. `ProfileRegistry` resolves the chain with cycle detection and layer-merged metadata/layout/rule_sets.
- **Rule sets**: each profile may declare named optional subrules (`rule_sets.<name>`) — e.g., `sun_arae_batchim` on `ko_3bul390` — toggled via GUI SwitchRow or CLI.
- **Config fields** (additive, zero impact when unset): `korean.custom_layout: Option<String>`, `korean.active_rule_sets: Vec<String>`. Wired through 5-point sync (config.rs ↔ unim-cli ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog).
- **CLI**: `unim-cli config layout list / describe <name> / validate <file.json>` (exit 0=pass, 1=warning, 2=error).
- **GUI**: Korean-layout ComboRow lists all profiles (10 built-in + user); selection redraws `rule_sets` as live SwitchRows.
- **New built-in profile**: `ko_3bul_qwerty` (Sebeolsik Qwerty-style) — 26-seat alphabet saturation (14 초성 / 15 중성 / 19 종성). Built-in count 9 → 10.

### 2. AutoTypeFix suppression + user dictionary

- **Suppression dictionary** (`~/.config/unim/typefix-blacklist.yaml`) with three states: Tentative / Confirmed / Inactive. Registration moved from rollback-moment to **retrigger-moment**, and the retrigger itself is suppressed in the same step. GUI "Confirm" promotes Tentative → Confirmed; tentatives flip to Inactive after `tentative_expiry_hours` (default 1, range 1..=12).
- **AutoTypeFix settings** — three new keys under `auto_typefix.*`: `rollback_detection` (bool, default true), `tentative_expiry_hours` (u16), `observation_timeout_secs` (u8, 5..=15). Wired through 3-point sync.
- **Settings GUI "Suppression Words" page**: three groups (Tentative / Confirmed / Inactive) with row actions [Confirm]/[Deactivate]/[Remove]/[Reactivate].
- **User dictionary** (`~/.config/unim/userdict.yaml`): selection-based `RegisterUserDictFromSelection` DBus method registers an English-side entry; GUI "User Dictionary" page lets you add/remove/update.

### 3. Hanja popup expansion

- **9×9 = 81-cell expanded grid**: period (`.`) toggles compact 9 ↔ expanded 81 across GTK Standalone, GTK IM, Qt IM, and XIM (the GNOME extension already had it). The ⊞/⊟ icon indicates the current mode.
- **Hanja bookmarks**: with a candidate focused, Space toggles ☆/★. The `HanjaBookmarkChanged` DBus signal refreshes every open popup across GTK/Qt/XIM/Wayland/GNOME instantly.

### 4. Auto-English-Mode

- **Opt-in** (`engine.auto_english.{enabled, trigger_keys}`, default off).
- In Korean mode, a trigger key (`Esc`, `/` etc.) commits the preedit, switches to English permanently, and forwards the key to the app.
- User-defined trigger keys: virtual names like `ShiftSemicolon` (`:`), `ShiftSlash` (`?`).
- Ideal for vim command mode and CLI slash commands.

### 5. Other

- **Korean/English layout enums removed**: layouts are now plain strings (Phases 8/9). C API setters/getters take/return C strings.
- **`unim-config` orphan crate removed**: folded into the `unim-cli config` subcommand.

---

## Changed

- **AutoTypeFix reverse rollback gate relaxed**: BS-AND-switch → **BS-OR-switch**. Reverse corrections use `clear_preedit=true`, so the IM module consumes the Backspace locally and never forwards it to `engine_worker`. Mode-switch alone is now sufficient. Forward keeps BS-AND-switch.
- **AutoTypeFix reverse suppression key fixed**: `RecentCorrection.ascii` now stores `fix.corrected` for reverse and `fix.original` for forward. Previously every reverse entry was blacklisted as `""` and never matched.
- **Blacklist registration moved to retrigger-moment**: prevents one-off mode mistakes from becoming permanent suppressions.
- **`KoreanLayout` enum removed (Phase 8)**: `korean.layout` is a String; built-ins or user profile names. Legacy `custom_layout: Option<String>` merged into `layout`. Existing YAML auto-normalizes via serde compat.
- **`EnglishLayout` enum removed (Phase 9)**: symmetric. `english.layout` String (`qwerty` / `dvorak` / `colemak` / `colemak_dh` / `workman`).
- **Tray/popup live sync**: `unim-gui` synchronizes immediately on `GlobalModeChanged`.

---

## Fixed

- **GTK3/4 IM `preedit-end` omission caused ghostty key lock**: introduced `unim_emit_preedit` helper to guarantee `preedit-end` on every commit/clear path.
- **XIM AutoTypeFix N+1 BS rewrite**: stable except for residual Chrome preedit cases (see `unim-frontends/xim/SPEC.md`).
- **`dbus_ime.js` `call_sync` non-standard argument fix**: `cancelHanja` / `cancelSpecialChar` now pass `GLib.VariantType` correctly.
- **Layout profile hot-reload re-initializes the active layout**: directory mtime watch + Composer rebuild.
- **Wayland `popup_surface` cleanup leak**: dangling popup_surface on FocusOut/Reset is now released.

---

## Breaking changes (user impact)

| Change | Impact | Auto migration |
|------|-----|------------------|
| `KoreanLayout` enum → String | Only C/C++ clients | YAML auto-normalized |
| `EnglishLayout` enum → String | Same | Same |
| `unim-config` crate removed | CLI invocation path | Same features under `unim-cli config` |
| `custom_layout` merged → `layout` | Only hand-edited YAML | serde compat handled |

**No impact for ordinary users.** GUI/CLI users have nothing to change.

---

## Migration guide

### 1. Upgrade packages

```bash
sudo apt install ./unim_0.2.0_amd64.deb \
                 ./unim-common_0.2.0_amd64.deb \
                 ./unim-im-gtk_0.2.0_amd64.deb \
                 ./unim-im-qt_0.2.0_amd64.deb \
                 ./unim-gui-gtk_0.2.0_amd64.deb
```

### 2. Restart the daemon

```bash
systemctl --user daemon-reload
systemctl --user restart unim-daemon
```

### 3. (Optional) Enable new features

```bash
unim-cli config set engine.auto_english.enabled true
unim-cli config set auto_typefix.tentative_expiry_hours 6
```

### 4. Try the new layout (`ko_3bul_qwerty`)

```bash
unim-cli config set korean.layout ko_3bul_qwerty
```

Or in the GTK GUI → "General" → "Korean layout".

---

## Known issues

| ID | Description | Impact | Workaround |
|----|------|-----|------|
| KI-001 | Pure Wayland (Sway/Hyprland) popup coordinates slightly off | Visual only | See [popup spec §8.4](../../../dev/specs/POPUP_SPEC.md) |
| KI-002 | XIM Chrome AutoTypeFix preedit residual | Rare visual artifact | Other browsers unaffected |
| KI-003 | Some Snap apps ignore conditional `~/.profile` env vars | Snap Korean input fails | `QT_IM_MODULE= GTK_IM_MODULE= snap run <app>` |

---

## Component-level summary

| Component | Highlights |
|----------|----------|
| Core (`src/`) | AutoTypeFix suppression + Layout profile v1 + auto_english hook |
| C-API (`unim-capi/`) | Layout enums removed → C strings |
| Daemon (`unim-daemon/`) | Profile hot-reload, blacklist mtime watch |
| DBus (`unim-dbus/`) | `RegisterUserDictFromSelection`, `HanjaBookmarkChanged` |
| CLI (`unim-cli/`) | `config layout` subcommand, `unim-config` folded in |
| GTK GUI (`unim-gui-gtk/`) | "Suppression Words" + "User Dictionary" pages, dynamic rule_sets |
| Qt GUI (`unim-gui-qt/`) | Live `GlobalModeChanged` sync |
| GTK3/4 IM | `unim_emit_preedit` helper, `preedit-end` fix |
| Qt5/6 IM | 81-cell grid, bookmark visualization |
| XIM | AutoTypeFix rewrite (N+1 BS) |
| Wayland | popup_surface cleanup fix |
| GNOME Extension | dbus_ime call_sync fix, bookmark signal handling |

---

## Contributors / acknowledgements

UNIM is led by a single maintainer (Seo Kihyun) with Claude Code-driven automation. The Phase 8/9 cleanup, AutoTypeFix stabilization, and v1 layout migration in 0.2.0 were executed via the harness configuration in [`AGENTS.md`](../../../../AGENTS.md), [`.claude/agents/`](../../../../.claude/agents/), and [`.claude/skills/`](../../../../.claude/skills/).

---

## Next (0.3.0 preview)

- Context-aware automatic Korean/English switching — roadmap stage 4.
- Engine v2 redesign (stroke replay, Old Hangul, dual-set auto-detection) — roadmap stage 6.
- macOS / Windows adapters — roadmap stage 5.

Full roadmap: [`ROADMAP.md`](../../../../ROADMAP.md).

---

## Reference docs

- [User manual](../../user-guide/README.md)
- [Troubleshooting](../../troubleshooting/README.md)
- [FAQ](../../faq/README.md)
- [`CHANGELOG.md`](../../../../CHANGELOG.md) — full changelog
