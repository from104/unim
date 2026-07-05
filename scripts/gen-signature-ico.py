#!/usr/bin/env python3
"""langbar/IME-선택 아이콘(unim-tsf/icons/unim-signature.ico) 재생성.

`assets/unim-logo v2.svg` 의 "un" 모노그램 글리프만 **투명 배경**으로 멀티해상도
.ico 로 굽는다. 배경 둥근타일(bgGradient rect)과 드롭섀도(filter)는 제거 — langbar/
입력기 선택기에서 어두운 타일이 배경에 묻혀 시인성이 나빴던 문제를 해소한다.

렌더링: 글리프는 그라디언트 stroke 라 cairosvg/순수 Pillow 로는 충실히 못 그린다.
Chrome(또는 Edge) 헤드리스로 SVG→투명 PNG 래스터한 뒤 Pillow 로 autocrop(여백 최소화
→ 작은 크기 시인성↑) + 멀티해상도 ICO 로 조립한다.

ARP 제품 아이콘(installer/assets/unim.ico, 채운 타일)은 별개이며 이 스크립트가 건드리지
않는다.

실행:  python scripts/gen-signature-ico.py
"""
import re
import subprocess
import sys
import tempfile
from pathlib import Path

from PIL import Image

ROOT = Path(__file__).resolve().parent.parent
SVG = ROOT / "assets" / "unim-logo v2.svg"
OUT = ROOT / "unim-tsf" / "icons" / "unim-signature.ico"

RENDER = 1024                                  # Chrome 래스터 해상도
ICO_SIZES = [256, 128, 64, 48, 40, 32, 24, 20, 16]
MARGIN = 0.08                                  # autocrop 후 사방 여백 비율

CHROME_CANDIDATES = [
    r"C:\Program Files\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files (x86)\Google\Chrome\Application\chrome.exe",
    r"C:\Program Files\Microsoft\Edge\Application\msedge.exe",
    r"C:\Program Files (x86)\Microsoft\Edge\Application\msedge.exe",
]


def find_chrome():
    for c in CHROME_CANDIDATES:
        if Path(c).exists():
            return c
    raise SystemExit("Chrome/Edge 를 찾지 못함 — CHROME_CANDIDATES 수정 필요")


def glyph_only_svg():
    """v2 SVG 에서 배경 rect + 드롭섀도를 제거하고 RENDER 크기로 스케일한 SVG 텍스트."""
    svg = SVG.read_text(encoding="utf-8")
    svg = re.sub(r"<rect\b[^>]*/>", "", svg)            # bgGradient 타일 제거
    svg = re.sub(r'\s*filter="url\(#shadow\)"', "", svg)  # 글리프 드롭섀도 제거
    svg = svg.replace('width="512" height="512"',
                      f'width="{RENDER}" height="{RENDER}"')
    return svg


def rasterize(svg_text):
    """Chrome 헤드리스로 글리프 SVG 를 투명 배경 PNG 로 래스터해 RGBA Image 반환."""
    with tempfile.TemporaryDirectory() as td:
        td = Path(td)
        html = td / "glyph.html"
        png = td / "glyph.png"
        html.write_text(
            "<!doctype html><html><head><meta charset='utf-8'>"
            "<style>html,body{margin:0;padding:0;background:transparent}"
            "svg{display:block}</style></head><body>" + svg_text + "</body></html>",
            encoding="utf-8",
        )
        subprocess.run(
            [find_chrome(), "--headless=new", "--disable-gpu", "--hide-scrollbars",
             "--force-device-scale-factor=1", "--default-background-color=00000000",
             f"--screenshot={png}", f"--window-size={RENDER},{RENDER}",
             html.as_uri()],
            check=True, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
        )
        return Image.open(png).convert("RGBA")


def main():
    src = rasterize(glyph_only_svg())
    bbox = src.getchannel("A").getbbox()
    if bbox is None:
        raise SystemExit("래스터 결과가 비어 있음(투명) — 렌더 실패")
    glyph = src.crop(bbox)
    gw, gh = glyph.size
    side = round(max(gw, gh) / (1 - 2 * MARGIN))
    canvas = Image.new("RGBA", (side, side), (0, 0, 0, 0))
    canvas.paste(glyph, ((side - gw) // 2, (side - gh) // 2), glyph)

    master = canvas.resize((512, 512), Image.LANCZOS)
    frames = [master.resize((s, s), Image.LANCZOS) for s in ICO_SIZES]
    OUT.parent.mkdir(parents=True, exist_ok=True)
    frames[0].save(OUT, format="ICO", sizes=[(s, s) for s in ICO_SIZES],
                   append_images=frames[1:])

    a = master.getchannel("A").histogram()
    print("wrote", OUT)
    print("glyph bbox", bbox, "fill", f"{100*max(gw,gh)/side:.0f}%")
    print("alpha=0", a[0], "alpha=255", a[255])


if __name__ == "__main__":
    main()
