#!/usr/bin/env bash
# GitHub 릴리스 본문 생성기
#
# 릴리스 노트를 따로 쓰지 않는다 — CHANGELOG 가 유일한 원본이고, 이 스크립트가
# 해당 버전 절을 그대로 떠서 릴리스 본문을 만든다. 그래서 릴리스 페이지와
# 저장소의 변경 이력이 어긋날 수가 없다.
#
# 본문 구성 (qcalc 와 같은 형식):
#   ## [X.Y.Z] YYYY-MM-DD
#   ### 알려진 문제        ← 읽는 사람이 먼저 봐야 하므로 맨 위로 끌어올린다
#   ### 수정됨 / 추가됨 / 변경됨
#   <details>English</details>
#   ---
#   설치 안내
#   전체 변경 이력 compare 링크
#
# 사용법: scripts/release-body.sh <태그> [이전태그]
#         이전 태그를 안 주면 git 태그 목록에서 바로 앞 것을 찾는다.

set -euo pipefail

TAG="${1:?태그를 달라 (예: v0.4.1)}"
PREV="${2:-}"
VERSION="${TAG#v}"
ROOT="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"

if [ -z "$PREV" ]; then
    # 현재 태그 바로 앞의 v* 태그. 첫 릴리스면 빈 값으로 남고 compare 줄을 뺀다.
    PREV=$(git -C "$ROOT" tag -l 'v*' --sort=-v:refname |
           awk -v cur="$TAG" 'found { print; exit } $0 == cur { found = 1 }')
fi

python3 - "$ROOT" "$VERSION" <<'PY'
import os, re, sys
root, version = sys.argv[1], sys.argv[2]

def section(path):
    """CHANGELOG 에서 해당 버전 절을 떼어 (제목줄, 본문) 로 돌려준다."""
    text = open(os.path.join(root, path), encoding="utf-8").read()
    m = re.search(r"^## \[%s\][^\n]*\n" % re.escape(version), text, re.M)
    if not m:
        sys.exit(f"{path} 에 [{version}] 절이 없다")
    rest = text[m.end():]
    nxt = re.search(r"^## \[", rest, re.M)
    return m.group(0).rstrip(), (rest[:nxt.start()] if nxt else rest).strip()

def known_first(body, *titles):
    """'알려진 문제' 소절을 맨 앞으로 옮긴다 — 읽는 사람이 먼저 봐야 한다."""
    blocks, cur = [], []
    for line in body.split("\n"):
        if line.startswith("### "):
            if cur: blocks.append(cur)
            cur = [line]
        else:
            cur.append(line)
    if cur: blocks.append(cur)
    known = [b for b in blocks if b and b[0][4:].strip() in titles]
    other = [b for b in blocks if b not in known]
    return "\n\n".join("\n".join(b).strip() for b in known + other if any(x.strip() for x in b))

heading, ko = section("CHANGELOG-ko.md")
_,       en = section("CHANGELOG.md")
ko = known_first(ko, "알려진 문제", "알려진 이슈")
en = known_first(en, "Known issues")

print(heading)
print()
print(ko)
print()
print("<details>")
print("<summary>English</summary>")
print()
print(en)
print()
print("</details>")
PY

cat <<EOF

---

### 설치 / Install (Ubuntu 24.04+ / Debian, amd64)

\`\`\`bash
curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
\`\`\`

버전 고정 / pin a version:

\`\`\`bash
UNIM_VERSION=${TAG} curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
\`\`\`

수동 설치 / manual: [SHA256SUMS](https://github.com/from104/unim/releases/download/${TAG}/SHA256SUMS) 로 검증 후 \`sudo apt install ./unim*.deb\`.

### 설치 / Install (Windows 10/11, x64)

PowerShell 에서 실행하시고, 설치할 때 UAC 승인이 한 번 필요합니다.

\`\`\`powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
\`\`\`

버전 고정 / pin a version:

\`\`\`powershell
\$env:UNIM_VERSION='${TAG}'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
\`\`\`

> ⏳ **MSI 는 별도 워크플로에서 빌드되어 이 릴리스에 첨부됩니다.**
> 태그를 막 올린 직후라면 몇 분 뒤에 나타납니다 — 설치 스크립트가 릴리스에서 MSI 를 내려받습니다.
> 아직 코드 서명이 없어 SmartScreen 경고가 뜹니다. "추가 정보 → 실행" 으로 넘어가 주세요.

변경 이력 / Changelog: [CHANGELOG-ko.md](https://github.com/from104/unim/blob/main/CHANGELOG-ko.md) · [CHANGELOG.md](https://github.com/from104/unim/blob/main/CHANGELOG.md)
EOF

if [ -n "$PREV" ]; then
    printf '\n**전체 변경 이력 / Full Changelog**: https://github.com/from104/unim/compare/%s...%s\n' "$PREV" "$TAG"
fi
