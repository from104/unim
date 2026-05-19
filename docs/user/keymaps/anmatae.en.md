# Ahnmatae Keyboard (2003) User Guide

> Requires **UNIM v0.3.0+**. Designed by Father Ahnmatae and Prof. Kim Jin-hyung of KAIST, published 2003.

---

## Overview

The Ahnmatae keyboard (안마태 자판) is a **three-beol (3벌식)** Korean layout with fixed regions for initial consonants (cho, 초성), vowels (jung, 중성), and final consonants (jong, 종성). Unlike standard two-beol or three-beol layouts, it supports **moachigi** (모아치기) — chord-based input where the jamo (자모, individual phonemes) of a syllable are typed simultaneously or in rapid succession and composed together.

- **Designers**: Father Ahnmatae + Prof. Kim Jin-hyung, KAIST (2003)
- **Layout type**: Three-beol (3bul), 4-row arrangement
- **Key features**: Fixed cho/jung/jong regions, **opt-in** bidirectional jamo combine, **opt-in** moachigi (chord) timing
- **Built-in profile name**: `ko_3bul_anmatae`
- **Where moachigi options live**: User config (`~/.config/unim/config.yaml` — keys `korean.bidirectional_combine` / `korean.chord_window_ms`). The keymap JSON only carries the `supports_moachigi: true` capability flag.

> **Archaic jamo (옛한글) not supported**: The original Ahnmatae layout placed archaic jamo at positions W/T/G/J/B/N (upper layer). In UNIM these positions are remapped to Korean typography symbols. Archaic jamo input is not supported in v0.3.0 and will be considered in a future release.

---

## Key Layout

### Region Overview

The Ahnmatae layout fixes each key's phoneme region across four rows.

| Row | Region | Contents |
|-----|--------|----------|
| Row 2 (QWERTY `Q`–`]` row) | cho (initial) + jung (vowel) | ㅁ ㅅ ㄴ ㄹ ㅎ + ㅕ ㅑ ㅡ ㅛ ㅠ |
| Row 3 (QWERTY `A`–`'` row) | cho (initial) + jung (vowel) | ㅂ ㅈ ㄷ ㄱ ㅇ + ㅓ ㅏ ㅣ ㅗ ㅜ |
| Row 4 (QWERTY `Z`–`/` row) | jong (final consonant) | ᆽ ᆮ ᆸ ᆨ ᆼ ᆺ ᆫ ᆷ ᆯ ᇂ |

> Cho and jung keys are interleaved across rows 2 and 3. Row 4 is exclusively jong.

### Lower Layer (no Shift)

```
┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
│ ` │ 1 │ 2 │ 3 │ 4 │ 5 │ 6 │ 7 │ 8 │ 9 │ 0 │ - │ = │ \ │  ← Row 1 (numbers/symbols)
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┴───┘   │
│ ㅁ│ ㅅ│ ㄴ│ ㄹ│ ㅎ│ ㅕ│ ㅑ│ ㅡ│ ㅛ│ ㅠ│ [ │ ]          │  ← Row 2 (cho + jung)
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┤            │
│ ㅂ│ ㅈ│ ㄷ│ ㄱ│ ㅇ│ ㅓ│ ㅏ│ ㅣ│ ㅗ│ ㅜ│ ' │            │  ← Row 3 (cho + jung)
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┴───┘            │
│ ᆽ│ ᆮ│ ᆸ│ ᆨ│ ᆼ│ ᆺ│ ᆫ│ ᆷ│ ᆯ│ ᇂ                       │  ← Row 4 (jong only)
└───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
```

> Row 4 jamo use **jongseong codepoints** (U+11A8 range), distinct from the compatibility jamo (U+3131 range) used in rows 2–3 for initial consonants. They represent the same sounds but differ internally — this is intentional and correct.

### Upper Layer (Shift held)

```
┌───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┬───┐
│ ~ │ ! │ @ │ # │ $ │ % │ ^ │ & │ * │ ( │ ) │ _ │ + │ | │  ← Row 1
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┴───┘   │
│ 「│ ㅆ│ 」│ ※ │ · │ ; │ '│ / │ [ │ ] │ { │ }          │  ← Row 2
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┼───┤            │
│ ㅃ│ ㅉ│ ㄸ│ ㄲ│ " │ ㅖ│ ㅒ│ ㅢ│ ㅘ│ ㅞ│ " │            │  ← Row 3
├───┼───┼───┼───┼───┼───┼───┼───┼───┼───┴───┘            │
│ ᆾ│ ᇀ│ ᇁ│ ᆿ│ " │ ᆻ │ ᆬ│ ᆱ│ ᆰ│ ?                     │  ← Row 4
└───┴───┴───┴───┴───┴───┴───┴───┴───┴───┘
```

> All 24 upper-layer slots produce something distinct from their lower counterparts. Even when moachigi (chord) is OFF, a single Shift keystroke can directly emit a double consonant, a combined vowel, an aspirated jongseong, a compound/double jongseong, or a Korean typography mark.

#### Korean typography marks (6 keys)

| Key | Shift output | Codepoint | Use |
|-----|--------------|-----------|-----|
| Q | `「` | U+300C | Korean opening bracket (낫표) |
| E | `」` | U+300D | Korean closing bracket (낫표) |
| R | `※` | U+203B | Reference mark |
| T | `·` | U+00B7 | Middle dot (Korean punctuation) |
| G | `"` | U+201D | Right double quotation mark |
| B | `"` | U+201C | Left double quotation mark |

#### Double consonants — 5 keys (initial)

| Key | Lower | Shift output | Codepoint |
|-----|-------|--------------|-----------|
| W | ㅅ | `ㅆ` | U+3146 |
| A | ㅂ | `ㅃ` | U+3143 |
| S | ㅈ | `ㅉ` | U+3149 |
| D | ㄷ | `ㄸ` | U+3138 |
| F | ㄱ | `ㄲ` | U+3132 |

#### Combined vowels — 5 keys (medial)

| Key | Lower | Shift output | Codepoint | Notes |
|-----|-------|--------------|-----------|-------|
| H | ㅓ | `ㅖ` | U+3156 | ㅕ+ㅣ |
| J | ㅑ | `ㅒ` | U+3152 | ㅑ+ㅣ |
| K | ㅣ | `ㅢ` | U+3162 | ㅡ+ㅣ |
| L | ㅗ | `ㅘ` | U+3158 | ㅗ+ㅏ |
| ; | ㅜ | `ㅞ` | U+315E | ㅜ+ㅔ |

#### Aspirated jongseong — 4 keys (final)

| Key | Lower | Shift output | Codepoint |
|-----|-------|--------------|-----------|
| Z | ᆽ | `ᆾ` | U+11BE |
| X | ᆮ | `ᇀ` | U+11C0 |
| C | ᆸ | `ᇁ` | U+11C1 |
| V | ᆨ | `ᆿ` | U+11BF |

#### Compound / double jongseong — 4 keys (final)

| Key | Lower | Shift output | Codepoint | Notes |
|-----|-------|--------------|-----------|-------|
| N | ᆺ | `ᆻ` | U+11BB | Double siot batchim |
| M | ᆫ | `ᆬ` | U+11AC | ㄴ+ㅈ → ㄵ |
| , | ᆷ | `ᆱ` | U+11B1 | ㄹ+ㅁ → ㄻ |
| . | ᆯ | `ᆰ` | U+11B0 | ㄱ+ㄹ → ㄺ |

> All of these can be produced through the moachigi chord window as well, but the direct Shift slots let users emit the same jamo in a single keystroke even when chord input is OFF.

---

## Jamo Combination Rules

Complex jamo are produced by combining two simpler jamo.

### Initial Consonant (Cho) Combinations

| First | Second | Result |
|-------|--------|--------|
| ㄱ | ㄷ | ㄸ |
| ㄱ | ㅇ | ㄲ |
| ㄱ | ㅎ | ㅋ |
| ㄴ | ㅅ | ㅆ |
| ㄷ | ㅈ | ㅉ |
| ㄷ | ㅎ | ㅌ |
| ㅂ | ㅈ | ㅃ |
| ㅂ | ㅎ | ㅍ |
| ㅈ | ㅎ | ㅊ |

Example: `ㄱ` + `ㅎ` → `ㅋ` (aspirated), `ㄱ` + `ㄷ` → `ㄸ` (tense/doubled)

### Vowel (Jung) Combinations

| First | Second | Result |
|-------|--------|--------|
| ㅏ | ㅣ | ㅐ |
| ㅓ | ㅣ | ㅔ |
| ㅗ | ㅏ | ㅘ |
| ㅗ | ㅣ | ㅚ |
| ㅜ | ㅓ | ㅝ |
| ㅜ | ㅣ | ㅟ |
| ㅡ | ㅣ | ㅢ |
| (8 more) | | |

### Final Consonant (Jong) Combinations (order-independent)

When `bidirectional_combine` is ON, both `(ᆨ, ᆯ)` and `(ᆯ, ᆨ)` produce `ᆰ` (ㄺ cluster).

| Combination | Result |
|-------------|--------|
| ᆨ + ᆼ | ᆩ |
| ᆨ + ᆯ | ᆰ |
| ᆨ + ᆺ | ᆪ |
| ᆫ + ᆽ | ᆬ |
| ᆫ + ᇂ | ᆭ |
| ᆯ + ᆷ | ᆱ |
| ᆯ + ᆸ | ᆲ |
| ᆯ + ᆺ | ᆳ |
| ᆸ + ᆺ | ᆹ |
| (11 more) | |

---

## Moachigi Options

Selecting "Ahnmatae Keyboard (2003)" in the GTK settings dialog automatically reveals the **Moachigi** option group. This group is only shown for layouts with `supports_moachigi=true`.

> **Both options default to OFF (opt-in)**. They must be enabled explicitly in the GTK settings dialog. The values are stored in the **user config** (`~/.config/unim/config.yaml`), not in the keymap JSON, so they persist across layout switches between moachigi-capable layouts. The keymap itself carries no option values.

### Option 1 — Bidirectional Jamo Combine

**Default: OFF (opt-in)** | User config key: `korean.bidirectional_combine`

Within each region (cho / jung / jong), jamo combination is attempted regardless of input order.

- **ON**: `(ᆯ, ᆨ)` is treated the same as `(ᆨ, ᆯ)` and produces `ᆰ` (ㄺ cluster). You can type the final consonants of "닭" (chicken) as ᆯ → ᆨ without worrying about order.
- **OFF (default)**: Only the forward direction defined in the combinations table is recognized. Option 2 (chord window) only has meaning when this option is ON.

### Option 2 — Chord Window (ms)

**Default: OFF (opt-in)** | Recommended: 60ms | Range: 10–200ms | 0 = OFF | User config key: `korean.chord_window_ms`

All jamo typed within N milliseconds of the first keystroke are collected into one chord and composed as a single syllable.

#### How It Works (Single Window)

```
First jamo arrives
    │
    ├─ Timer starts (N ms, single window — not reset on each keystroke)
    │
    ├─ Additional jamo within window → accumulated in chord buffer
    │
    └─ Window expires (or next-key arrival detects expiry)
           ├─ 1 jamo in buffer  → normal sequential processing
           └─ 2+ jamo in buffer → chord compose
                  (classify by region → bidirectional combine → commit syllable)
```

The window is flushed by any of the following:

- Idle timeout (automatic flush after N ms with no further input)
- Non-jamo key: Space, Enter, Tab, Backspace, arrow keys, symbols
- Korean/English mode switch
- Focus loss (FocusOut)
- Escape (chord discarded — uncommitted jamo are dropped)
- MAX 8 jamo reached (immediate flush)

**Setting to 0** disables chord entirely. Each jamo is processed immediately, identical to standard three-beol behavior. This is the default state.

> **Option 2 only has meaning when Option 1 (Bidirectional Combine) is ON.** When both are OFF (the default), the Ahnmatae layout behaves like a standard three-beol layout.

---

## GTK Settings Dialog

1. Click the tray icon → open **Settings**
2. **Keyboard** tab → Korean layout selector → choose **"Ahnmatae Keyboard (2003)"**
3. The **Moachigi** group appears immediately below the layout selector (both options default OFF)
4. **Bidirectional Jamo Combine** toggle: enable this first if you want moachigi behavior. Otherwise the layout recognizes only the forward jong order, just like a standard three-beol.
5. **Chord Window (ms)** slider: drag from 0 (OFF, default) up to 10–200ms to activate chord input. Fast typists: lower (20–30ms). Slower typists: higher (80–150ms). Recommended default is 60ms.

> Switching to a different layout (e.g., 390, 391, QWERTY-style three-beol) automatically hides the Moachigi group. The user-config option values are preserved and re-applied when you switch back to Ahnmatae.

---

## Input Scenarios

### Normal Sequential Input (chord OFF or in-order typing)

| Input sequence | Output | Notes |
|----------------|--------|-------|
| ㄱ → ㅏ → ㅁ | 감 | cho → jung → jong |
| ᆸ → ᆺ | ᆹ (ㅄ cluster) | jong combination |

### Moachigi Chord Input (chord ON, within 50ms)

| Simultaneous input (within 50ms) | Output | Notes |
|----------------------------------|--------|-------|
| ㄱ + ㅎ + ㅡ + ㅣ | 킈 | cho: ㄱ+ㅎ→ㅋ, jung: ㅡ+ㅣ→ㅢ |
| ᆨ + ᆯ | ᆰ | jong cluster, order-independent |

### Syllable Separation (window expired)

| Input | Output | Notes |
|-------|--------|-------|
| ㄱ + ㅡ → (50ms expires) → ㅎ + ㅣ | 그히 | Two syllables due to timeout |
| ㄱ + ㅜ → (50ms expires) → ㅎ + ㅏ + ㄷ + ㅏ | 구하다 | Correct syllable separation |

### Typography Symbol Input

| Key | Output | Use |
|-----|--------|-----|
| Shift+B | `"` | Left double quote |
| Shift+G | `"` | Right double quote |
| Shift+N | `'` | Left single quote |
| Shift+W | `'` | Right single quote |
| Shift+J | `·` | Middle dot |
| Shift+T | `…` | Ellipsis |

---

## Keyboard Compatibility (NKRO Recommended)

Moachigi (chord input) requires that every key pressed simultaneously is reported to the operating system as a separate key event. This property is called **N-Key Rollover (NKRO)**. Without it, some keys in a simultaneous press are silently dropped — a phenomenon known as **ghosting** — causing chords to be incomplete or composed as the wrong jamo.

### KRO Limits and Ghosting

Most membrane keyboards support only **2-KRO or 3-KRO**. On a 3-KRO keyboard, pressing four keys simultaneously means one of them is never reported. A typical Ahnmatae chord covers 2–4 keys (e.g., 2 cho + 2 jung), so a **6-KRO keyboard handles the majority of chords fine**. Complex chords that span all three regions (cho 2 + jung 2 + jong 2 = up to 6 keys) require NKRO.

| Keyboard type | KRO level | Moachigi suitability |
| ------------- | --------- | -------------------- |
| Standard membrane (budget) | 2–3 KRO | Simple 2-key chords OK; complex chords risk ghosting |
| Gaming membrane | 6–10 KRO (certain key combinations) | Most chords OK |
| Mechanical (USB) | 6–14 KRO, NKRO mode often available | Recommended |
| Mechanical (PS/2 interface) | Full NKRO | Ideal |

> Recommendation: use a **mechanical keyboard in NKRO mode** or a **PS/2-connected keyboard**. Many USB mechanical keyboards allow switching to NKRO via a firmware toggle (e.g., Fn+N or a BIOS-level setting — consult your keyboard's documentation).

### USB Polling Rate

USB keyboards report key events to the OS at a default rate of **125 Hz (one report every 8 ms)**. When `chord_window_ms` is set to 10–30 ms, that 8 ms polling interval consumes a significant portion of the window, and some chord keys may only be reported in the next polling cycle — causing them to be missed. To work around this:

- Raise `chord_window_ms` to **60 ms or higher** to give the polling interval adequate headroom.
- Switch to a **1000 Hz (1 ms) polling** gaming keyboard, or try a different USB port.

### Ghosting Self-Diagnosis

#### Method 1 — xev (Linux X11)

```sh
xev -event keyboard
```

Focus the white window that appears, then press all the keys of your target chord simultaneously. The terminal should print one `KeyPress event` line per key. If any key is missing from the output, ghosting is occurring.

Example: to type `킈` on the Ahnmatae layout, press Q (ㄱ), H (ㅎ), U (ㅡ), and I (ㅣ) at once. The `xev` log must show four `KeyPress` events — `q`, `h`, `u`, `i` — or a chord key is being dropped.

#### Method 2 — Online key tester

Open a key-tester site such as [keyboardchecker.com](https://keyboardchecker.com) or [keyboard-test.com](https://keyboard-test.com) in your browser, then press all the keys in your chord simultaneously. Every key should light up on the on-screen keyboard visualization. Any key that stays unlit is being ghosted.

#### Method 3 — Raise chord_window_ms above 100 ms as a test

Increasing the window relaxes the simultaneity requirement. If chords work correctly at 100 ms+ but fail at your normal setting, the issue is polling rate or KRO — not input speed.

---

## Troubleshooting

### "구하다" (guhada) comes out as "쿠ㅏ다"

The chord window is capturing `ㄱ` and `ㅎ` together and combining them into `ㅋ`. Two fixes:

- **Fix 1**: Lower the chord window or set it to 0. This forces each jamo to be processed immediately, preventing cross-syllable merging.
- **Fix 2**: Keep chord enabled but pause briefly between syllables — let 50ms pass after typing `ㄱ+ㅜ` before starting `ㅎ+ㅏ+ㄷ+ㅏ`.

### Screen shows nothing while I type (chord in progress)

The preedit (in-progress text) is not shown while the chord window is open. With a 50ms window this is imperceptible in practice. If you raise the slider above 100ms the delay becomes noticeable — this is expected behavior.

### I switched layouts but the Moachigi group is still visible

Close and reopen the settings dialog, or restart the UNIM daemon.

### Escape drops my in-progress input

Escape during a chord discards the entire buffer — all uncommitted jamo are dropped. This is intentional. To commit the in-progress syllable instead, press Space or Enter.

---

## Limitations and Non-Goals

| Item | Status | Notes |
|------|--------|-------|
| Archaic jamo (옛한글) input | **Not supported** | Archaic codepoints trigger `LoadError::ArchaicJamoNotSupported`. Planned post-v0.4.0. |
| Preedit display during chord | **Not supported** | Screen is silent while chord window is open. Imperceptible at 50ms default. |
| Variant layouts (e.g., Sinsebeolsik-M) | **Not built-in** | Only the 2003 standard Ahnmatae layout is included. Variants can be provided as user-defined layout JSON files. |
| Maximum jamo per chord | 8 | Immediate flush at 8 jamo. |

---

## References

- Ahnmatae, Kim Jin-hyung (2003), "Design of a Moachigi Hangul Input Keyboard"
- UNIM layout profile v3 schema: `docs/dev/architecture/LAYOUT_PROFILE_V3.md`
- Layout JSON: `src/keystroke/keymap/ko_3bul_anmatae.json`
