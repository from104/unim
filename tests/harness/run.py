#!/usr/bin/env python3
"""UNIM 테스트 앱 자동시험 실행기

  tests/harness/run.py --app gtk3
  tests/harness/run.py --all
  tests/harness/run.py --app xim --scenario commit-then-preedit
  tests/harness/run.py --all --allow-layout-change

판정은 앱이 남긴 `field.render` 사건 — 화면에 실제로 나타나는 문자열 — 으로
한다. 데몬이 옳게 보냈는지가 아니라 **화면에 보이는지**를 본다.

설계 근거: docs/dev/testing/TEST_APPS.md §5
"""

from __future__ import annotations

import argparse
import sys

import harness as H

G, R, Y, D, B, Z = ("\x1b[32m", "\x1b[31m", "\x1b[33m",
                    "\x1b[2m", "\x1b[1m", "\x1b[0m")


def print_result(res: H.ScenarioResult) -> None:
    if res.skipped:
        print(f"  {Y}⊘ SKIP{Z} {res.name} {D}— {res.skipped}{Z}")
        return

    if res.ok:
        mark = f"{G}✓ PASS{Z}"
    elif res.known_issue:
        mark = f"{Y}✗ KNOWN{Z}"
    else:
        mark = f"{R}✗ FAIL{Z}"
    print(f"  {mark} {res.name}")
    if res.known_issue and not res.ok:
        print(f"      {Y}알려진 문제:{Z} {res.known_issue}")

    for s in res.steps:
        if s.ok:
            print(f"      {D}{s.index}. {s.action}{Z}")
            continue
        print(f"      {R}{s.index}. {s.action}{Z}")
        for k, want in s.expected.items():
            got = s.actual.get(k)
            flag = " " if got == want else f"{R}←{Z}"
            print(f"         {k:10} 기대 {want!r}  실제 {got!r} {flag}")

    if res.screenshot:
        print(f"      {D}스크린샷 {res.screenshot}{Z}")
    if not res.ok and res.log_path:
        print(f"      {D}로그      {res.log_path}{Z}")


def main() -> int:
    ap = argparse.ArgumentParser(description="UNIM 테스트 앱 자동시험")
    ap.add_argument("--app", action="append",
                    help="대상 앱 (반복 가능). 미지정+--all 이면 XTEST 가능한 전부")
    ap.add_argument("--all", action="store_true", help="XTEST 가능한 모든 앱")
    ap.add_argument("--scenario", action="append", help="시나리오 이름 (반복 가능)")
    ap.add_argument("--allow-layout-change", action="store_true",
                    help="시나리오가 요구하면 korean_layout 을 바꾸고 끝나면 되돌린다")
    ap.add_argument("--keep-open", action="store_true",
                    help="실패해도 앱을 닫지 않는다 (눈으로 확인할 때)")
    ap.add_argument("--list", action="store_true", help="시나리오 목록만 출력")
    args = ap.parse_args()

    scenarios = H.load_scenarios(args.scenario)
    if args.list:
        for sc in scenarios:
            print(f"{sc['name']:24} [{sc.get('layout','any')}] {sc.get('desc','')}")
        return 0
    if not scenarios:
        print(f"{R}시나리오가 없다{Z}", file=sys.stderr)
        return 2

    if args.all:
        apps = [a for a, s in H.APPS.items() if s["xtest"]]
    elif args.app:
        apps = args.app
    else:
        ap.error("--app 또는 --all 중 하나가 필요하다")

    if not H.daemon_alive():
        print(f"{R}unim-daemon 이 떠 있지 않다 — 시험을 돌릴 수 없다{Z}",
              file=sys.stderr)
        return 3

    print(f"{B}UNIM 테스트 앱 자동시험{Z}")
    print(f"  앱       {' '.join(apps)}")
    print(f"  시나리오 {len(scenarios)}개")
    print(f"  레이아웃 {H.get_config('korean_layout')}")
    print(f"  결과     {H.OUT_DIR}")
    print()

    total = passed = failed = skipped = known = 0
    for app in apps:
        if app not in H.APPS:
            print(f"{R}알 수 없는 앱: {app}{Z}", file=sys.stderr)
            return 2
        print(f"{B}▶ {app}{Z}")
        for sc in scenarios:
            res = H.run_scenario(app, sc,
                                 allow_layout_change=args.allow_layout_change,
                                 keep_open=args.keep_open)
            print_result(res)
            total += 1
            if res.skipped:
                skipped += 1
            elif res.ok:
                passed += 1
            elif res.known_issue:
                known += 1
            else:
                failed += 1
        print()

    print(f"{B}═══ 결과 ═══{Z}")
    print(f"  {G}통과 {passed}{Z}   {R}실패 {failed}{Z}   "
          f"{Y}알려진 문제 {known}{Z}   {Y}건너뜀 {skipped}{Z}"
          f"   (전체 {total})")
    # 알려진 문제는 종료 코드를 더럽히지 않는다 — 신규 회귀만 CI 를 빨갛게 한다.
    return 1 if failed else 0


if __name__ == "__main__":
    sys.exit(main())
