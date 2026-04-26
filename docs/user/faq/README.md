# UNIM FAQ (English)

> The questions people actually ask about UNIM 0.2.0.
> Each answer carries at least one line of "why it works that way" so you can use it for your next decision, not just as a fact lookup.

---

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

## Q2. Can UNIM coexist with another IME on the same desktop?

**Technically yes, but not recommended.** With two IMEs alive, the OS and toolkit cannot tell where to deliver key events.

- **GNOME**: leaving IBus enabled alongside UNIM causes frequent key drops → uninstall IBus.
  ```bash
  sudo apt remove ibus
  ```
- **KDE**: fcitx5 runs its own daemon and conflicts. Pick exactly one through env vars.
- **Test bench**: separating into VMs/containers is fine for comparison.

> Bottom line: pick exactly one. Cleanly remove the other before installing UNIM.

---

## Q3. Which environments are most stable?

Stability tier as of UNIM 0.2.0:

| Environment | Tier | Note |
|------|------|------|
| Ubuntu 24.04 + GNOME(Wayland) + extension | 🟢 A | Recommended; main dev/test environment |
| Ubuntu 24.04 + GNOME(X11) | 🟢 A | Env vars + Standalone popup |
| KDE Plasma 6 (Wayland) | 🟢 B+ | input-method-v2 works |
| KDE Plasma 6 (X11) | 🟢 B+ | XIM/Qt IM both fine |
| Sway (Wayland) | 🟡 B | Popup positioning slightly off — see [popup spec §8.4](../../specs/POPUP_SPEC.md) |
| Hyprland (Wayland) | 🟡 B | Same |
| XFCE/MATE (X11) | 🟢 B+ | Traditional, solid |
| Pure Wayland (compositor-dependent) | 🟡 B/C | Depends on the compositor's IM protocol support |

**Tier A = "first-time user starting point"**. Once you are familiar, the rest is fine too.

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

---

## Q7. What layouts exist, and can I add my own?

### Built-in (10 Korean layouts as of 0.2.0)

`ko_2bulstd` (Dubeolsik standard), `ko_3bul390` (Sebeolsik 390), `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_qwerty` (Sebeolsik Qwerty-style), plus 5 variants.

### English

`qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`.

### User-defined

Drop a v1 schema JSON into `~/.config/unim/layouts/<name>.json` — daemon scans automatically. Use `inherits: "ko_3bul390"` to override only what you need.

```bash
unim-cli config layout validate ~/.config/unim/layouts/my.json
unim-cli config set korean.layout my
```

Schema details: [`docs/dev/plans/LAYOUT_PROFILE_V1.md`](../../dev/plans/LAYOUT_PROFILE_V1.md).

> Use `rule_sets` to bundle optional toggles with a layout. E.g. `ko_3bul390`'s `sun_arae_batchim`. The settings GUI dynamically renders a SwitchRow.

---

## Q8. How much memory does UNIM use?

In normal operation, `unim-daemon` RSS sits in 30–80 MB. UNIM 0.2.0 hardens this:

- `#[global_allocator] tikv_jemallocator::Jemalloc` blocks the glibc ptmalloc arena explosion.
- `Environment=MALLOC_ARENA_MAX=2` in systemd (belt-and-suspenders for the C path).
- 60-second `libc::malloc_trim(0)` task forces memory release back to the OS.

> A previous incident saw RSS balloon to 2 GB on 0.1.x. Regression on those items is forbidden. If you observe RSS > 500 MB, see [troubleshooting §14](../troubleshooting/README.md#14-daemon-eats-too-much-memory-rss-500-mb).

---

## Q9. Does UNIM intercept passwords?

**No.** Password fields are detected via `content_purpose` and forced to English. AutoTypeFix, hanja conversion, and special-char popup are all disabled. The input is not retained in daemon memory.

> Caveat: this works only when the app accurately reports `content_purpose=password`. Some web forms do not — for those, manually verify English mode via the Hangul key.

---

## Q10. Migration notes from 0.1.x to 0.2.0?

Mostly automatic. Two normalizations:

- `korean.layout` written as an enum (`Dubeolsik`) is auto-converted to a string (`ko_2bulstd`).
- `english.layout` written as an enum is auto-converted (`qwerty`, etc.).
- `typefix-blacklist.yaml`'s old keys go through a serde compat layer.

C API: `UnimEnglishLayout` / `UnimKoreanLayout` enums removed → setters/getters now take/return C strings. Affects only C/C++ clients.

Full migration: [release notes](../release-notes/0.2.0/RELEASE_NOTES.md).

---

## Q11. Does UNIM run on macOS / Windows?

**Linux only for now.** Roadmap stage 5 lists cross-platform but it has not started. Because the Rust core and C-API are separated, in principle adapters to macOS IMKit / Windows TSF are feasible. Volunteers welcome.

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
2. [`AGENTS.md`](../../../AGENTS.md) — architecture and component map.
3. [`IME_BEHAVIOR.md`](../../../IME_BEHAVIOR.md) — behavior spec.
4. Per-crate `SPEC.md`.
5. Verify: `make build` warning-free + `cargo test --workspace` all pass.
6. Convention: commit messages in English, docs in Korean.

`good-first-issue` labels are the best entry point.

---

## Q14. Why "Universal" in the name?

**Universal Next-generation Input Method.** Although it is a Korean IME, "universal" stands for (a) bidirectional Korean ↔ English handling and (b) one core plugged into every toolkit. In the long term, also macOS/Windows (roadmap stage 5).

---

## Read more

- [User manual](../user-guide/README.md)
- [Troubleshooting](../troubleshooting/README.md)
- [Release notes 0.2.0](../release-notes/0.2.0/RELEASE_NOTES.md)
- [`IME_BEHAVIOR.md`](../../../IME_BEHAVIOR.md)
- [`POPUP_SPEC.md`](../../specs/POPUP_SPEC.md)
