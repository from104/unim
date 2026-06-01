#!/usr/bin/env python3
"""assets/unim-logo.svg 의 'unim' 워드마크를 투명 배경 멀티해상도 .ico 로 렌더.

svglib/cairosvg 는 linearGradient(url(#g)) 를 못 그리므로, SVG path 를 직접
플래튼(cubic bezier → 폴리라인)하고 Pillow 로 가로 그라디언트(#1e6ab5→#6c40a6)
를 채운다. 배경은 완전 투명. 결과: unim-tsf/icons/unim-signature.ico
"""
import re
from PIL import Image, ImageDraw

SVG = r"C:\Users\USER\Desktop\work\unim\assets\unim-logo.svg"
OUT = r"C:\Users\USER\Desktop\work\unim\unim-tsf\icons\unim-signature.ico"

VB_W, VB_H = 874.0, 632.0
# linearGradient g: x1=69 → x2=805, #1e6ab5 → #6c40a6 (userSpaceOnUse)
GX1, GX2 = 69.0, 805.0
C0 = (0x1e, 0x6a, 0xb5)
C1 = (0x6c, 0x40, 0xa6)

ICO_SIZES = [256, 128, 64, 48, 40, 32, 24, 20, 16]
MASTER = 1024          # 논리 캔버스(정사각)
SS = 4                 # 안티앨리어싱 슈퍼샘플
MARGIN = 0.90          # 로고가 차지하는 최대 비율
BEZ_STEPS = 48         # cubic 분할 수


def extract_path_d(svg_text):
    m = re.search(r'<path\b[^>]*\bd="([^"]+)"', svg_text, re.S)
    if not m:
        raise SystemExit("path d 없음")
    return m.group(1)


def parse_subpaths(d):
    """절대 M/L/C/Z 만 처리 (이 로고가 사용하는 명령). 서브패스별 점 목록 반환."""
    tokens = re.findall(r'[MLCZ]|-?\d*\.?\d+(?:e-?\d+)?', d, re.I)
    subpaths, cur = [], None
    cx = cy = 0.0
    sx = sy = 0.0
    i = 0
    cmd = None
    nums = []

    def flush_point(x, y):
        cur.append((x, y))

    while i < len(tokens):
        t = tokens[i]
        if t in 'MLCZmlcz':
            cmd = t
            i += 1
            if cmd in 'Zz':
                if cur is not None and len(cur) > 2:
                    subpaths.append(cur)
                cur = None
            continue
        # numeric operand for current cmd
        if cmd in 'Mm':
            x = float(tokens[i]); y = float(tokens[i + 1]); i += 2
            if cur is not None and len(cur) > 2:
                subpaths.append(cur)
            cur = []
            cx, cy = x, y
            sx, sy = x, y
            flush_point(cx, cy)
            cmd = 'L'  # 이후 좌표쌍은 L 로 간주(SVG 규약)
        elif cmd in 'Ll':
            x = float(tokens[i]); y = float(tokens[i + 1]); i += 2
            cx, cy = x, y
            flush_point(cx, cy)
        elif cmd in 'Cc':
            x1 = float(tokens[i]); y1 = float(tokens[i + 1])
            x2 = float(tokens[i + 2]); y2 = float(tokens[i + 3])
            x = float(tokens[i + 4]); y = float(tokens[i + 5]); i += 6
            p0 = (cx, cy)
            for s in range(1, BEZ_STEPS + 1):
                tt = s / BEZ_STEPS
                u = 1 - tt
                bx = (u*u*u*p0[0] + 3*u*u*tt*x1 + 3*u*tt*tt*x2 + tt*tt*tt*x)
                by = (u*u*u*p0[1] + 3*u*u*tt*y1 + 3*u*tt*tt*y2 + tt*tt*tt*y)
                flush_point(bx, by)
            cx, cy = x, y
        else:
            i += 1
    if cur is not None and len(cur) > 2:
        subpaths.append(cur)
    return subpaths


def build_master(d):
    subpaths = parse_subpaths(d)
    # 정사각 캔버스에 가로/세로 비율 유지 배치
    scale = MARGIN * MASTER / max(VB_W, VB_H)
    draw_w, draw_h = VB_W * scale, VB_H * scale
    ox = (MASTER - draw_w) / 2.0
    oy = (MASTER - draw_h) / 2.0

    big = MASTER * SS

    def tf(p):
        return ((ox + p[0] * scale) * SS, (oy + p[1] * scale) * SS)

    # even-odd 마스크: 각 서브패스를 XOR 합성
    mask = Image.new("L", (big, big), 0)
    for sp in subpaths:
        layer = Image.new("L", (big, big), 0)
        ImageDraw.Draw(layer).polygon([tf(p) for p in sp], fill=255)
        # XOR (even-odd 누적)
        from PIL import ImageChops
        mask = ImageChops.difference(mask, layer)
    mask = mask.resize((MASTER, MASTER), Image.LANCZOS)

    # 가로 그라디언트 RGB (원본 x 좌표 기준)
    grad = Image.new("RGB", (MASTER, MASTER))
    px = grad.load()
    for mx in range(MASTER):
        orig_x = (mx - ox) / scale
        t = (orig_x - GX1) / (GX2 - GX1)
        t = 0.0 if t < 0 else (1.0 if t > 1 else t)
        col = tuple(round(C0[k] + (C1[k] - C0[k]) * t) for k in range(3))
        for my in range(MASTER):
            px[mx, my] = col

    out = Image.new("RGBA", (MASTER, MASTER), (0, 0, 0, 0))
    out.paste(grad, (0, 0), mask)
    return out


def main():
    with open(SVG, encoding="utf-8") as f:
        svg_text = f.read()
    d = extract_path_d(svg_text)
    master = build_master(d)
    frames = [master.resize((s, s), Image.LANCZOS) for s in ICO_SIZES]
    frames[0].save(OUT, format="ICO", sizes=[(s, s) for s in ICO_SIZES],
                   append_images=frames[1:])
    # 미리보기 PNG (검수용)
    master.resize((256, 256), Image.LANCZOS).save(
        r"C:\Users\USER\Desktop\work\unim\scripts\unim-signature-preview.png")
    print("wrote", OUT)
    print("preview scripts/unim-signature-preview.png")
    # 알파 통계
    a = master.getchannel("A")
    hist = a.histogram()
    print("alpha=0 px:", hist[0], "alpha=255 px:", hist[255])


if __name__ == "__main__":
    main()
