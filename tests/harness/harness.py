"""UNIM 테스트 앱 자동시험 하네스 — 핵심 엔진

앱을 실제 IM 환경변수로 띄우고, XTEST(`xdotool`)로 진짜 키를 넣고, 앱이
남긴 JSONL 의 `field.render` 로 판정한다. 데몬을 직접 호출하지 않는다 —
툴킷 → IM 모듈 → 데몬 → 되돌아오는 전 구간이 시험 대상이기 때문이다.

표준 라이브러리만 쓴다.

설계 근거: docs/dev/testing/TEST_APPS.md §5
"""

from __future__ import annotations

import json
import os
import re
import shutil
import subprocess
import sys
import time
from dataclasses import dataclass, field as dc_field
from pathlib import Path

REPO = Path(__file__).resolve().parents[2]
OUT_DIR = Path(os.environ.get("UNIM_HARNESS_OUT", "/tmp/unim-harness"))

DBUS_ARGS = [
    "--session",
    "-d", "org.atit.unim.InputMethod",
    "-o", "/org/atit/unim/InputMethod",
]

# ─── 앱 정의 ────────────────────────────────────────────────────────────
#
# `xtest` 가 False 인 앱에는 XTEST 키 주입이 닿지 않는다(Wayland 네이티브).
# 하네스는 그런 앱을 자동으로 건너뛴다 — TEST_APPS.md §9.

APPS: dict[str, dict] = {
    "gtk3": {
        "bin": "tests/unim-test-gtk3/build/unim-test-gtk3",
        "env": {"GDK_BACKEND": "x11", "GTK_IM_MODULE": "unim",
                "XMODIFIERS": "@im=unim"},
        "xtest": True,
    },
    # gtk3 바이너리를 GTK 의 xim 모듈로 띄운다 — GTK3 im-xim + libX11 경로
    # (이슈 C: CALLBACKS 미광고 + PeWindow 커버리지). 바이너리·창 제목은
    # gtk3 와 동일하므로 title 로 명시한다.
    "gtk3-xim": {
        "bin": "tests/unim-test-gtk3/build/unim-test-gtk3",
        "env": {"GDK_BACKEND": "x11", "GTK_IM_MODULE": "xim",
                "XMODIFIERS": "@im=unim"},
        "xtest": True,
        "title": "gtk3",
    },
    "gtk4": {
        "bin": "tests/unim-test-gtk4/build/unim-test-gtk4",
        "env": {"GDK_BACKEND": "x11", "GTK_IM_MODULE": "unim",
                "XMODIFIERS": "@im=unim"},
        "xtest": True,
    },
    "qt5": {
        "bin": "tests/unim-test-qt/build/unim-test-qt5",
        "env": {"QT_QPA_PLATFORM": "xcb", "QT_IM_MODULE": "unim",
                "XMODIFIERS": "@im=unim"},
        "xtest": True,
    },
    "qt6": {
        "bin": "tests/unim-test-qt/build/unim-test-qt6",
        "env": {"QT_QPA_PLATFORM": "xcb", "QT_IM_MODULE": "unim",
                "XMODIFIERS": "@im=unim"},
        "xtest": True,
    },
    "xim": {
        "bin": "tests/unim-test-xim/build/unim-test-xim",
        "env": {"XMODIFIERS": "@im=unim", "GTK_IM_MODULE": "xim"},
        "xtest": True,
    },
    "gnome": {
        "bin": "tests/unim-test-gnome/build/unim-test-gnome",
        "env": {},                      # Wayland 네이티브 — 확장 경로
        "xtest": False,
    },
    "wayland": {
        "bin": "target/release/unim-test-wayland",
        "env": {},
        "xtest": False,
    },
}


def window_title(app: str) -> str:
    """`UNIM_SPEC_WIN_TITLE_FMT` 와 같아야 한다 (unim_test_spec.h).

    창 제목은 바이너리에 컴파일타임으로 박혀 있다. 같은 바이너리를 다른
    env 조합으로 띄우는 앱 엔트리(예: "gtk3-xim")는 APPS 의 선택 키
    "title" 로 실제 바이너리 이름을 지정한다 — 없으면 앱 키를 그대로 쓴다.
    """
    name = APPS.get(app, {}).get("title", app)
    return f"UNIM {name} 테스트"


# ─── 데몬 ───────────────────────────────────────────────────────────────

def _gdbus(method: str, *args: str) -> str:
    cmd = ["gdbus", "call", *DBUS_ARGS,
           "-m", f"org.atit.unim.InputMethod.{method}", *args]
    r = subprocess.run(cmd, capture_output=True, text=True)
    if r.returncode != 0:
        raise RuntimeError(f"gdbus {method} 실패: {r.stderr.strip()}")
    return r.stdout.strip()


def daemon_alive() -> bool:
    try:
        _gdbus("GetGlobalMode")
        return True
    except Exception:
        return False


def get_mode() -> bool:
    return "true" in _gdbus("GetGlobalMode")


def set_mode(korean: bool) -> None:
    _gdbus("SetGlobalMode", "true" if korean else "false")


def get_config(key: str) -> str:
    out = _gdbus("GetConfig", key)
    return out.strip("(),'\" ")


def set_config(key: str, value: str) -> None:
    _gdbus("SetConfig", key, value)


# ─── 키 주입 ────────────────────────────────────────────────────────────

class Injector:
    """xdotool(XTEST) 래퍼. 실패를 삼키지 않고 예외로 올린다."""

    def __init__(self, wid: str, delay_ms: int = 40):
        self.wid = wid
        self.delay = str(delay_ms)

    def _run(self, *args: str) -> None:
        r = subprocess.run(["xdotool", *args], capture_output=True, text=True)
        if r.returncode != 0:
            raise RuntimeError(f"xdotool {' '.join(args)}: {r.stderr.strip()}")

    def activate(self) -> None:
        # `windowactivate` 는 창 관리자에게 "이 창을 최상단으로 올리고
        # 포커스를 줘라" 라고 요청한다 — WM 이 없는 환경(Xvfb 단독 CI
        # 컨테이너)에서는 요청을 받아줄 WM 자체가 없어 조용히 아무 일도
        # 안 하고, mutter 등 일부 WM 은 포커스 탈취(focus stealing) 방지
        # 정책으로 거부한다. `windowfocus` 는 X 서버에 직접
        # XSetInputFocus 를 거는 XTEST 수준 동작이라 WM 유무와 무관하게
        # 먹힌다 — 실세션(WM 있음)에서도 결과는 동일(포커스 이동)하므로
        # 안전한 대체다.
        self._run("windowfocus", "--sync", self.wid)
        time.sleep(0.2)

    def origin(self) -> tuple[int, int]:
        """
        창 콘텐츠 영역의 화면 좌표. 앱이 상대 좌표만 알려줄 때 더한다.

        `xdotool getwindowgeometry` 는 쓰지 않는다 — CSD 창에서 실측해 보면
        콘텐츠 원점과 어긋난다(GTK3 에서 (114,115) vs 실제 (100,66)).
        `xwininfo` 의 Absolute upper-left 가 정확하다.
        """
        r = subprocess.run(["xwininfo", "-id", self.wid],
                           capture_output=True, text=True)
        x = y = 0
        for line in r.stdout.splitlines():
            s = line.strip()
            if s.startswith("Absolute upper-left X:"):
                x = int(s.rsplit(":", 1)[1])
            elif s.startswith("Absolute upper-left Y:"):
                y = int(s.rsplit(":", 1)[1])
        return x, y

    def key(self, keyname: str) -> None:
        self._run("key", "--clearmodifiers", "--delay", self.delay, keyname)

    def click(self, x: int, y: int) -> None:
        self._run("mousemove", "--sync", str(x), str(y))
        self._run("click", "1")

    def screenshot(self, path: Path) -> bool:
        if not shutil.which("import"):
            return False
        r = subprocess.run(["import", "-window", self.wid, str(path)],
                           capture_output=True)
        return r.returncode == 0


# ─── 앱 프로세스 ────────────────────────────────────────────────────────

@dataclass
class RunningApp:
    name: str
    proc: subprocess.Popen
    log_path: Path
    stdout_path: Path
    wid: str = ""
    geometry: dict = dc_field(default_factory=dict)
    _pos: int = 0

    # ── JSONL 읽기 ──

    def events(self) -> list[dict]:
        """마지막으로 읽은 지점 이후의 새 사건들."""
        if not self.log_path.exists():
            return []
        out = []
        with self.log_path.open("r", encoding="utf-8", errors="replace") as f:
            f.seek(self._pos)
            for line in f:
                line = line.strip()
                if not line:
                    continue
                try:
                    ev = json.loads(line)
                except json.JSONDecodeError:
                    continue      # 아직 다 안 쓰인 마지막 줄
                # 좌표는 여기서 항상 최신으로 유지한다. 창 관리자가 창을 옮기면
                # 앱이 geometry 를 다시 내는데(XIM 은 ConfigureNotify 마다),
                # 예전에는 wait_ready() 안에서만 반영해 app.ready 이후의 갱신을
                # 통째로 버렸다 — 그래서 옛 좌표로 창 밖을 클릭했다(2026-08-09).
                if ev.get("ev") == "geometry" and "field" in ev:
                    self.geometry[ev["field"]] = ev
                out.append(ev)
            self._pos = f.tell()
        return out

    def all_events(self) -> list[dict]:
        if not self.log_path.exists():
            return []
        out = []
        for line in self.log_path.read_text(encoding="utf-8",
                                            errors="replace").splitlines():
            line = line.strip()
            if line:
                try:
                    out.append(json.loads(line))
                except json.JSONDecodeError:
                    pass
        return out

    def wait_ready(self, timeout: float = float(os.environ.get(
            "UNIM_HARNESS_READY_TIMEOUT", "15.0"))) -> bool:
        deadline = time.time() + timeout
        while time.time() < deadline:
            if self.proc.poll() is not None:
                return False
            for ev in self.events():
                if ev.get("ev") == "geometry":
                    self.geometry[ev["field"]] = ev
                elif ev.get("ev") == "app.ready":
                    return True
            time.sleep(0.05)
        return False

    def last_render(self, field: str) -> dict | None:
        best = None
        for ev in self.all_events():
            if ev.get("ev") == "field.render" and ev.get("field") == field:
                best = ev
        return best

    def stop(self) -> None:
        if self.proc.poll() is None:
            self.proc.terminate()
            try:
                self.proc.wait(timeout=3)
            except subprocess.TimeoutExpired:
                self.proc.kill()


def launch(app: str, tag: str) -> RunningApp:
    spec = APPS[app]
    binary = REPO / spec["bin"]
    if not binary.exists():
        raise FileNotFoundError(f"{app}: 빌드된 바이너리가 없다 — {binary}\n"
                                f"  `make build-tests` 를 먼저 돌릴 것")

    OUT_DIR.mkdir(parents=True, exist_ok=True)
    log_path = OUT_DIR / f"{app}-{tag}.jsonl"
    out_path = OUT_DIR / f"{app}-{tag}.stdout"
    log_path.unlink(missing_ok=True)

    env = dict(os.environ)
    env.update(spec["env"])
    env["UNIM_TEST_LOG"] = str(log_path)
    env["UNIM_TEST_LOG_FORMAT"] = "human"      # stdout 은 사람용, 판정은 파일

    proc = subprocess.Popen([str(binary)], env=env,
                            stdout=out_path.open("w"),
                            stderr=subprocess.STDOUT)
    return RunningApp(name=app, proc=proc, log_path=log_path,
                      stdout_path=out_path)


def find_window(app: str,
                timeout: float = float(os.environ.get(
                    "UNIM_HARNESS_WINDOW_TIMEOUT", "8.0"))) -> str:
    title = window_title(app)
    # ⚠️ CI 컨테이너(Xvfb, 로케일 미설치)에서 실측: `xdotool getwindowname` 은
    # "UNIM xim 테스트" 를 정확히 돌려주는데 `xdotool search --name` 으로
    # 그 *전체* 문자열(한글 포함)을 찾으면 LANG=C.utf8 을 줘도 항상 rc=1 —
    # xdotool 의 정규식 매칭기가 다중바이트 패턴을 못 삼키는 한계로 보인다
    # (2026-09 실측, gtk3/gtk4/qt5/qt6/xim 전 앱에서 재현 — 이 함수 자체의
    # 문제이지 특정 앱 회귀가 아니다). 제목 앞부분 "UNIM {app} " 는 항상
    # ASCII 이고 앱마다 유일하므로, 검색 패턴에서 첫 비 ASCII 문자 이후를
    # 잘라내 그 접두어만으로 찾는다 — 표시되는 실제 창 제목은 그대로 둔다.
    search_pattern = re.split(r"[^\x00-\x7f]", title, maxsplit=1)[0].rstrip()
    deadline = time.time() + timeout
    while time.time() < deadline:
        r = subprocess.run(["xdotool", "search", "--name", search_pattern],
                           capture_output=True, text=True)
        ids = [w for w in r.stdout.split() if w]
        if ids:
            return ids[-1]
        time.sleep(0.15)
    raise RuntimeError(f"{app}: 창을 못 찾았다 (제목 \"{title}\", 검색패턴 \"{search_pattern}\")")


# ─── 시나리오 ───────────────────────────────────────────────────────────

@dataclass
class StepResult:
    index: int
    action: str
    ok: bool
    expected: dict
    actual: dict
    waited_ms: int


@dataclass
class ScenarioResult:
    app: str
    name: str
    ok: bool
    skipped: str = ""
    steps: list[StepResult] = dc_field(default_factory=list)
    screenshot: Path | None = None
    log_path: Path | None = None
    known_issue: str = ""      # 이 앱에서 이미 알려진 실패면 그 설명


def _match(expect: dict, render: dict | None) -> bool:
    if render is None:
        return False
    return all(render.get(k) == v for k, v in expect.items())


def _wait_for(app: RunningApp, field: str, expect: dict,
              timeout_ms: int | None = None) -> tuple[bool, dict, int]:
    """기대값에 도달할 때까지 폴링한다 — 도달 즉시 통과라 빠르다.

    `timeout_ms` 미지정 시 `UNIM_HARNESS_STEP_TIMEOUT_MS`(기본 2500) 를 쓴다 —
    CI 전용 상향 값은 scripts/ci/functional-test.sh 가 CI=true 일 때만 심는다.
    실세션(그 변수가 없는 일반 실행)은 항상 2500ms 그대로다.
    """
    if timeout_ms is None:
        timeout_ms = int(os.environ.get("UNIM_HARNESS_STEP_TIMEOUT_MS", "2500"))
    deadline = time.time() + timeout_ms / 1000.0
    last: dict = {}
    while time.time() < deadline:
        r = app.last_render(field)
        if r:
            last = r
            if _match(expect, r):
                return True, r, int(timeout_ms - (deadline - time.time()) * 1000)
        time.sleep(0.03)
    return False, last, timeout_ms


def run_scenario(app_name: str, sc: dict, *,
                 allow_layout_change: bool = False,
                 keep_open: bool = False) -> ScenarioResult:
    res = ScenarioResult(app=app_name, name=sc["name"], ok=False)
    # 이미 원인이 규명된 앱별 실패는 신규 회귀와 구분해 표시한다.
    # 고쳐지면 시나리오에서 이 항목을 지운다 — 그때부터 다시 FAIL 로 잡힌다.
    res.known_issue = (sc.get("known_fail") or {}).get(app_name, "")

    spec = APPS[app_name]
    if not spec["xtest"]:
        res.skipped = "XTEST 가 닿지 않는 앱 (Wayland 네이티브)"
        return res
    if not daemon_alive():
        res.skipped = "unim-daemon 이 떠 있지 않다"
        return res

    # 레이아웃 — 사용자 설정을 함부로 바꾸지 않는다.
    want_layout = sc.get("layout")
    saved_layout = None
    if want_layout:
        cur = get_config("korean_layout")
        if cur != want_layout:
            if not allow_layout_change:
                res.skipped = (f"레이아웃이 {cur} 인데 시나리오는 {want_layout} 요구 "
                               f"(--allow-layout-change 로 허용)")
                return res
            saved_layout = cur
            set_config("korean_layout", want_layout)
            time.sleep(0.4)

    saved_mode = get_mode()
    running = None
    try:
        running = launch(app_name, sc["name"])
        res.log_path = running.log_path
        if not running.wait_ready():
            res.skipped = "app.ready 도달 실패 (앱이 뜨지 않음)"
            return res

        running.wid = find_window(app_name)
        inj = Injector(running.wid, delay_ms=sc.get("key_delay_ms", 40))
        inj.activate()
        ox, oy = inj.origin()

        def click_field(fid: str) -> None:
            g = running.geometry.get(fid)
            if not g:
                raise RuntimeError(f"필드 {fid} 의 geometry 가 없다")
            # 앱이 화면 절대 좌표를 알면 그걸 쓴다 — 창 장식 두께에 안 흔들린다.
            if g.get("screen_cx", -1) >= 0:
                inj.click(g["screen_cx"], g["screen_cy"])
                return
            # 폴백: 창 원점 + 창 내부 상대. 창 장식·스케일·좌표계 차이에
            # 흔들리므로 신뢰도가 낮다. 2026-08-09 에 gtk4 가 절대 좌표를
            # -1 로 내는 바람에 이 경로를 타서 빈 곳을 클릭했고, 캔버스가
            # 포커스를 잃어 키가 한 개도 안 들어갔다. 조용히 빗나가면
            # 15 초 타임아웃까지 원인을 알 수 없으므로 쓰인 사실을 남긴다.
            x, y = ox + g["cx"], oy + g["cy"]
            print(f"    ⚠ {app_name}: {fid} 절대 좌표 없음 — 폴백 클릭 "
                  f"({x},{y}) = 창원점({ox},{oy}) + 상대({g['cx']},{g['cy']})",
                  file=sys.stderr)
            inj.click(x, y)

        set_mode(bool(sc.get("korean", True)))
        time.sleep(0.4)

        field = sc.get("field", "core.plain")
        # 대상 필드를 클릭해 포커스를 확실히 잡는다.
        if field in running.geometry:
            running.events()          # 이전 사건을 흘려보내고 클릭 결과만 본다
            click_field(field)
            time.sleep(0.25)
            #
            # 클릭이 빗나가면 캔버스가 포커스를 잃어 키가 한 개도 안 들어가고,
            # 15 초 뒤 "preedit 기대 'ㅎ' 실제 ''" 라는 애매한 실패로 끝난다.
            # 그래서 여기서 바로 끊되, **판정 기준은 대상 필드가 포커스를
            # 받았는가**다.
            #
            # ⚠️ `focus.out` 을 실패로 보면 안 된다 — 다른 필드에서 옮겨오면
            # 이전 필드의 focus.out 은 당연히 난다(2026-08-09, 이 판정을 잘못
            # 걸어 password/multiline 시나리오가 5개 앱에서 전부 걸렸다).
            #
            # 판정: **대상 필드가 포커스를 잃었는데 되찾지도 못했는가**.
            #   · 다른 필드에서 옮겨옴 → 대상의 focus.in 이 온다 (정상)
            #   · 같은 필드 재클릭    → focus 사건이 아예 없다 (정상)
            #   · 클릭이 빗나감       → 대상이 focus.out / canvas-focus-out (실패)
            evs = running.events()
            got = any(e.get("ev") == "focus.in" and e.get("field") == field
                      for e in evs)
            lost = any(
                (e.get("ev") == "focus.out" and e.get("field") == field)
                or (e.get("ev") == "reset" and e.get("field") == field
                    and "focus-out" in str(e.get("reason", "")))
                for e in evs)
            if lost and not got:
                raise RuntimeError(
                    f"{field} 클릭 직후 포커스를 잃었다 — 클릭 좌표가 필드를 "
                    f"벗어났을 가능성이 크다. 앱이 screen_cx/cy 를 내는지 확인할 것 "
                    f"(geometry={running.geometry.get(field)})")

        # 포커스 획득과 IM 컨텍스트 등록(XIM 연결·Qt 플랫폼 컨텍스트·데몬 D-Bus
        # 왕복)은 별개 비동기 경로다 — 공유 CI 러너에서 CPU 경합이 있으면 포커스는
        # 잡혔는데 IM 컨텍스트가 아직 등록 전이라 첫 키가 IM 을 거치지 않고
        # 그대로 리터럴 문자로 커밋되는 사례가 있다(2026-09 실측, 로컬 docker
        # 에서는 재현 안 됨 — CPU 여유 차이로 추정). 기본값 0 이라 실세션은
        # 영향 없고, CI 는 functional-test.sh 가 UNIM_HARNESS_SETTLE_MS 를 심는다.
        settle_ms = int(os.environ.get("UNIM_HARNESS_SETTLE_MS", "0"))
        if settle_ms > 0:
            time.sleep(settle_ms / 1000.0)

        all_ok = True
        for i, step in enumerate(sc["steps"]):
            if "click" in step:
                target = step["click"]
                click_field(target)
                action = f"click {target}"
                field = target
            elif "key" in step:
                inj.key(step["key"])
                action = f"key {step['key']}"
            elif "keys" in step:
                for k in step["keys"]:
                    inj.key(k)
                action = "keys " + " ".join(step["keys"])
            elif "mode" in step:
                set_mode(bool(step["mode"]))
                action = "mode " + ("한글" if step["mode"] else "영문")
                time.sleep(0.3)
            elif "wait" in step:
                time.sleep(step["wait"] / 1000.0)
                action = f"wait {step['wait']}ms"
            else:
                raise RuntimeError(
                    f"스텝 {i}: key/keys/click/mode/wait 중 하나가 필요하다")

            expect = step.get("expect")
            check_field = step.get("field", field)
            if expect:
                ok, actual, waited = _wait_for(running, check_field, expect)
            else:
                time.sleep(0.2)
                ok, actual, waited = True, running.last_render(check_field) or {}, 0

            res.steps.append(StepResult(i, action, ok, expect or {},
                                        {k: actual.get(k) for k in
                                         (expect or {"rendered": None})},
                                        waited))
            if not ok:
                all_ok = False
                shot = OUT_DIR / f"{app_name}-{sc['name']}-step{i}.png"
                if inj.screenshot(shot):
                    res.screenshot = shot
                break        # 첫 실패에서 멈춘다 — 뒤 스텝은 의미가 없다

        res.ok = all_ok
        return res

    except Exception as e:
        # 한 시나리오의 사고가 전체 매트릭스를 멈추게 하지 않는다.
        res.ok = False
        res.steps.append(StepResult(len(res.steps), f"오류: {e}", False,
                                    {}, {}, 0))
        return res

    finally:
        if running and not keep_open:
            running.stop()
        try:
            set_mode(saved_mode)
            if saved_layout is not None:
                set_config("korean_layout", saved_layout)
        except Exception:
            pass


def load_scenarios(names: list[str] | None = None) -> list[dict]:
    """`scenarios/*.json` 을 읽는다. 한 파일에 시나리오 하나 또는 배열."""
    d = Path(__file__).resolve().parent / "scenarios"
    out = []
    for p in sorted(d.glob("*.json")):
        data = json.loads(p.read_text(encoding="utf-8"))
        for sc in (data if isinstance(data, list) else [data]):
            if names is None or sc["name"] in names:
                out.append(sc)
    return out
