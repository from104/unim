#!/usr/bin/env python3
"""
UNIM popup CSS generator.

Inputs:
  - popup_tokens.toml — design SoT (colors, spacing, fonts, transitions).
  - templates/gtk_hanja_popup.css.tmpl — GTK4 CSS template (uses {{token.path}} placeholders).
  - templates/st_hanja_popup.css.tmpl  — GNOME Shell St CSS template.

Outputs:
  - ../../unim-gui-gtk/src/popup_styles.generated.css   — embedded via include_str! in popup_css() function.
  - ../../unim-gnome-extension/stylesheet.css           — regenerates the section between
      `/* GENERATED-HANJA-POPUP BEGIN */` and `/* GENERATED-HANJA-POPUP END */` markers.
      Other sections (indicator, preedit, special, emoji, shared widgets) preserved.

Usage:
  $ python3 tools/popup-styles/gen.py            # writes outputs in-place
  $ python3 tools/popup-styles/gen.py --check    # exit 1 if outputs would change (CI guard)

Token reference syntax in templates: {{section.key}}
  e.g. {{colors.bg_main}}, {{spacing.popup_padding}}, {{fonts.cell_size}}, {{transitions.duration_default}}.
"""

from __future__ import annotations

import argparse
import sys
import re
from pathlib import Path

# tomllib is stdlib in Python 3.11+. Fall back to tomli for older versions.
try:
    import tomllib  # type: ignore
except ModuleNotFoundError:  # pragma: no cover
    import tomli as tomllib  # type: ignore

ROOT = Path(__file__).resolve().parent
REPO = ROOT.parent.parent

TOKENS_PATH = ROOT / "popup_tokens.toml"
TMPL_GTK = ROOT / "templates" / "gtk_hanja_popup.css.tmpl"
TMPL_ST = ROOT / "templates" / "st_hanja_popup.css.tmpl"

OUT_GUI_GTK = REPO / "unim-gui-gtk" / "src" / "popup_styles.generated.css"
OUT_EXTENSION = REPO / "unim-gnome-extension" / "stylesheet.css"

MARKER_BEGIN = "/* GENERATED-POPUP-CSS BEGIN — DO NOT EDIT (run `make gen-popup-css`) */"
MARKER_END = "/* GENERATED-POPUP-CSS END */"

PLACEHOLDER_RE = re.compile(r"\{\{\s*([a-zA-Z_]+)\.([a-zA-Z_]+)\s*\}\}")


def load_tokens(path: Path) -> dict:
    with path.open("rb") as f:
        return tomllib.load(f)


def render_template(template: str, tokens: dict) -> str:
    """Substitute {{section.key}} placeholders. Errors out on missing tokens to avoid silent drift."""
    missing: list[str] = []

    def repl(m: re.Match) -> str:
        section, key = m.group(1), m.group(2)
        section_dict = tokens.get(section)
        if section_dict is None or key not in section_dict:
            missing.append(f"{section}.{key}")
            return m.group(0)
        return str(section_dict[key])

    out = PLACEHOLDER_RE.sub(repl, template)
    if missing:
        raise SystemExit(
            f"[gen.py] Missing tokens referenced by template: {', '.join(sorted(set(missing)))}"
        )
    return out


def update_extension_stylesheet(existing: str, generated_block: str) -> str:
    """Replace the section between markers in stylesheet.css with generated_block.

    If markers are absent, raise — installation step is required first (one-time manual marker
    insertion). This avoids accidentally replacing the wrong section.
    """
    if MARKER_BEGIN not in existing or MARKER_END not in existing:
        raise SystemExit(
            "[gen.py] stylesheet.css missing GENERATED-POPUP-CSS markers. "
            "Insert them around the .unim-hanja-popup section (one-time manual step) before running gen.py."
        )
    pattern = re.compile(
        re.escape(MARKER_BEGIN) + r".*?" + re.escape(MARKER_END),
        re.DOTALL,
    )
    new_block = f"{MARKER_BEGIN}\n{generated_block.strip()}\n{MARKER_END}"
    return pattern.sub(new_block, existing, count=1)


def write_if_changed(path: Path, content: str, *, check: bool) -> bool:
    """Return True if the file would change. In --check mode, do not write."""
    existing = path.read_text(encoding="utf-8") if path.exists() else None
    changed = existing != content
    if changed and not check:
        path.write_text(content, encoding="utf-8")
    return changed


def main() -> int:
    parser = argparse.ArgumentParser(description="Generate UNIM popup CSS from tokens.")
    parser.add_argument(
        "--check",
        action="store_true",
        help="Do not write files; exit 1 if generated content differs (CI guard).",
    )
    args = parser.parse_args()

    tokens = load_tokens(TOKENS_PATH)
    gtk_tmpl = TMPL_GTK.read_text(encoding="utf-8")
    st_tmpl = TMPL_ST.read_text(encoding="utf-8")

    gtk_out = render_template(gtk_tmpl, tokens)
    st_out = render_template(st_tmpl, tokens)

    changed_gtk = write_if_changed(OUT_GUI_GTK, gtk_out, check=args.check)

    existing_ext = OUT_EXTENSION.read_text(encoding="utf-8") if OUT_EXTENSION.exists() else ""
    new_ext = update_extension_stylesheet(existing_ext, st_out)
    changed_ext = write_if_changed(OUT_EXTENSION, new_ext, check=args.check)

    if args.check:
        if changed_gtk or changed_ext:
            print(
                "[gen.py] DRIFT detected — generated content does not match committed files.\n"
                "         Run `make gen-popup-css` and commit the result.",
                file=sys.stderr,
            )
            if changed_gtk:
                print(f"  - {OUT_GUI_GTK.relative_to(REPO)}", file=sys.stderr)
            if changed_ext:
                print(f"  - {OUT_EXTENSION.relative_to(REPO)}", file=sys.stderr)
            return 1
        print("[gen.py] OK — generated content matches committed files.")
        return 0

    print(
        f"[gen.py] wrote {OUT_GUI_GTK.relative_to(REPO)}"
        f" ({'changed' if changed_gtk else 'unchanged'})"
    )
    print(
        f"[gen.py] wrote {OUT_EXTENSION.relative_to(REPO)}"
        f" ({'changed' if changed_ext else 'unchanged'})"
    )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
