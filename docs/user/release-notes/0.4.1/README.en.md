# UNIM 0.4.1 Release Notes

**Release date**: 2026-08-16
**Branch**: develop → main

> In one line: the **rpm packages that were missing** for Fedora-family distributions are back, and Sticky Keys combinations no longer drop their modifier.

A small follow-up to 0.4.0. No new features, no configuration format changes — upgrade in place.

---

## Fixed

### 1. rpm packages never reached the release

0.4.0 shipped 11 .deb packages and the Windows MSI, but **no rpm at all**. One-line installation was simply unavailable on Fedora and RHEL-family systems.

The build was broken in two places.

- **`MAKEFLAGS` leak**: `rpmbuild`'s `%make_build` injects a `--` separator into `MAKEFLAGS`, which the jemalloc (`tikv-jemalloc-sys`) build script swallowed and then failed on.
- **`Qt6::GuiPrivate` not found**: On Qt 6.8 and later (Fedora 43) private modules must be requested explicitly. The .deb side uses Qt 6.4, where the target comes along for free, so this never surfaced there.

Verified with a real build on Fedora 43. The one-liner now works on Fedora-family systems:

```bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
```

### 2. Sticky Keys combinations intermittently did nothing (GNOME Wayland)

Inside text fields, combinations such as `Ctrl` → `A` or `Alt` → `F` were **sometimes delivered without the modifier**, so a plain character was typed instead.

When the input method intercepted a key and handed it back, GNOME skipped the point where it re-applies the Sticky Keys latch. The returned key arrived a beat later, and if the latch was released in the meantime the modifier was gone. Now **keys the input method will not consume are passed straight through**, so the latch survives.

Selection commands like `Shift`+`Home` and `Shift`+arrow had the **same defect** and are fixed as well.

> For people who can only press one key at a time, Sticky Keys is not a convenience — it is the difference between being able to use combination keys and not.

---

## Known limitation

- **Modifier combinations used mid-composition** do not guarantee the latch. This applies when you press something like `Shift`+`Home` while a Hangul syllable is still being composed. In that case we prioritise delivering the committed text to the application first, because text arriving out of order is the worse failure. Finish the syllable first and the combination behaves normally.

---

## Upgrading

If UNIM is already installed, upgrade through your distribution's package manager:

```bash
# Debian / Ubuntu family
sudo apt update && sudo apt upgrade

# Fedora / RHEL family
sudo dnf upgrade unim
```

Re-running the install script works too. Your configuration (`~/.config/unim/`) is preserved.

> This release changes the GNOME Shell extension, so the Sticky Keys fix only takes effect after you **log out and log back in**. GNOME Shell does not reload extension code during a session.

---

## Still outstanding

Not included in this release:

- **Hangul input in sandboxed apps (Flatpak, Snap)**: commit and continuous input were restored on the IBus path in 0.4.0, but the in-progress syllable (preedit) still does not appear on screen.
- **Composition stalls on the XIM path**: environments using GTK's `im-xim` still stall mid-composition.
- **Windows**: combination keys (`Ctrl`+`B` and similar) do not work while auto-English switching is active, and the tray right-click menu does not appear after a fresh install or update.

---

See [CHANGELOG.md](../../../../CHANGELOG.md) for the full change history.
