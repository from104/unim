# UNIM 0.3.0 Release Notes (English)

**Release date**: 2026-05-19
**Branch**: arch/popup-unify

> One-line summary: popup architecture unified under a single popup-service SoT, GNOME extension gains an integrated popup_view, and Ahnmatae + Moachigi v4 Atomic Window ships as the first chord-based Korean layout.

<!-- TODO: screenshot popup-service GTK4 view -->
<!-- TODO: screenshot GNOME extension popup_view -->
<!-- TODO: screenshot settings dialog moachigi group -->

---

## Migration guide

### unim-gui-qt removed — switch to unim-gui-gtk + unim-popup-service

The `unim-gui-qt` package is gone in 0.3.0. KDE Plasma users should migrate:

```bash
# deb
sudo apt remove unim-gui-qt
sudo apt install unim-gui-gtk unim-popup-service

# rpm
sudo dnf remove unim-gui-qt
sudo dnf install unim-gui-gtk unim-popup-service
```

**Your settings are preserved.** `~/.config/unim/config.yaml` and any user profiles under `~/.config/unim/layouts/` are unaffected by the package change.

### v0 layout profile users

Any `~/.config/unim/layouts/*.json` file without `schema_version`, `metadata`, or `combinations` keys is treated as a legacy v0 profile and rejected by the loader. Add `"schema_version": 1` and supply a `combinations` block. All built-in profiles were already migrated to v1 in 0.2.0.

---

## Added

### 1. Popup single SoT architecture — unim-popup-service

Rendering responsibility for hanja, special-character, and emoji popups has moved from the daemon to a new sidecar process: `unim-popup-service`.

- Daemon's `org.atit.unim.InputContext` signals (8 total) are forwarded to the `org.atit.unim.Popup` interface
- D-Bus auto-activation via `org.atit.unim.PopupService.service` — autostart `.desktop` removed
- Single view-model (`PopupRender` payload): cells, header, footer, tabs, and highlight are identical across all environments

Environment routing:

| Environment | Renderer |
|-------------|----------|
| GNOME Wayland | Extension `popup_view.js` (St widget) |
| GNOME X11 / KDE / Xfce | `unim-popup-service` GTK4 window |
| Wayland WM (Sway/Hyprland) | `unim-popup-service` GTK4 (wayland-backend, requires `libgtk4-layer-shell`) |

Debug:

```bash
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup
```

### 2. GNOME Shell extension popup_view integration

Since Mutter does not support `wlr-layer-shell` or `zwp_input_popup_v2`, the extension now renders hanja, special-character, and emoji popups itself using `popup_view.js` (`PopupView` class, St widget).

- Shares the same CSS tokens and class names as popup-service (`.unim-hanja-popup`, `.grid-cell`, etc.)
- Activated only when `Meta.is_wayland_compositor()` returns true — on X11 the popup-service GTK4 window is used instead (prevents double-render)
- **Outside-click dismiss**: clicking outside the popup closes it; the click event passes through to the window below

### 3. Ahnmatae 2003 + Moachigi v4 Atomic Window

UNIM's first chord-based Korean layout.

- **`ko_3bul_anmatae`** built-in: 9 cho, 15 jung, 20 jong combination rules
- **Moachigi v4 — Atomic Window Principle**: all branching decisions made at window expiry, not on each keystroke. 1 jamo in buffer → normal sequential processing; 2+ jamo → region-classified permutation search. Mid-window commit artifacts eliminated.
- **`chord_window_ms`**: range 10–200 ms, default 60 ms (expert-tuned). Beginners: 100–150 ms recommended
- **`bidirectional_combine`**: sequential jamo also combine bidirectionally (e.g. ㅎ then ㄱ → ㅋ)
- Exposed as a slider in the GTK settings dialog; group visible only when the selected layout has `supports_moachigi=true`

### 4. AutoTypeFix learning blacklist refinement

Tentative suppression entries are now registered and applied at retrigger time (not at rollback time), eliminating false positives. GUI "Suppression Words" page manages Tentative/Confirmed/Inactive states.

### 5. Hanja mouse pagination + 9×9 grid

- ◀/▶ buttons unified across all frontends (GNOME, GTK, Qt, XIM, Wayland)
- Buttons auto-hidden when `total_pages == 1`
- Period (`.`) key toggles compact 9-cell ↔ expanded 81-cell grid
- Un-bookmark (☆) triggers Catppuccin yellow `#f9e2af` cursor flash for 140 ms

---

## Changed

- **Settings dialog live help enrichment**: 26 tooltips and 15 subtitles rewritten using a what/when/why/recommended-value template
- **`chord_window_ms` slider range**: 10–100 ms → **10–200 ms**, default 50 ms → 60 ms
- **`emoji_popup.enabled` setting removed**: the Hanja key idle trigger is now always-on (during composition → hanja conversion; idle → emoji popup)

---

## Fixed (best-effort)

- **XIM ON-THE-SPOT lingering regression mitigated**: `commit_then_preedit` now forces `clear_preedit()` before `commit()`. OVER-THE-SPOT clients (XTerm, WezTerm) are now fully restored. Some ON-THE-SPOT (PREEDIT_CALLBACKS) clients still exhibit the regression — pending an upstream fix in xim-0.5.0.

---

## Breaking changes

- **`HanjaCandidatesReordered` signal payload changed to 10-tuple** (was 9). A `was_bookmarked: bool` field was appended. External subscribers must update their unpacking code.
- **Layout profile v0 schema rejected**: JSON files without v1 markers now return `LoadError::UnsupportedSchema`. Add `"schema_version": 1`.
- **`unim-gui-qt` removed**: migrate to `unim-gui-gtk` + `unim-popup-service`.

---

## Removed

- `unim-gui-qt` package
- `emoji_popup.enabled` config field (all 5 sync points)
- Rust const jamo combination tables (`JUNG_COMBINATIONS`, etc.)
- `SchemaKind` enum + `detect()`
- `HangulComposer3BulMoachigi` separate composer
- `ko_3bul_qwerty` built-in (JSON preserved at `docs/references/keymaps/ko_3bul_qwerty_v2.json`)

---

## Known issues

- **KDE Plasma 5.x Wayland**: `gtk4-layer-shell` unavailable in Ubuntu 24.04 standard repositories; popups do not appear. Workaround: use an X11 session or switch to GNOME.
- **XIM ON-THE-SPOT (PREEDIT_CALLBACKS) preedit drop**: persists on some clients after the best-effort fix. Waiting on an upstream xim-0.5.0 fix.

---

## Read more

- [User manual](../../user-guide/README.md)
- [Troubleshooting](../../troubleshooting/README.md)
- [FAQ](../../faq/README.md)
- [CHANGELOG](../../../../CHANGELOG.md)
- [POPUP_SPEC.md](../../../dev/specs/POPUP_SPEC.md)
