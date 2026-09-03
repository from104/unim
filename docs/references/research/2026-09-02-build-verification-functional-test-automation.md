# UNIM 빌드 검증·기능 테스트 자동화 조사 보고서

- 작성일: 2026-09-02
- 브랜치: develop / HEAD `c013775`("배포판 매트릭스 빌드 — 여섯 배포판을 각자의 컨테이너에서")
- 대상: UNIM 6배포판 빌드·패키징 파이프라인(`scripts/ci/*`, `scripts/build-linux-matrix.sh`, `.github/workflows/linux-deb.yml`·`linux-rpm.yml`·`linux-ci.yml`) + 런타임 기능 테스트 자산(`tests/harness/*`, `tests/unim-test-*`) + 기존 헤드리스 검사(`make check-compat`)
- 범위: (a) 지금 CI가 실제로 검증하는 층이 어디까지인가, (b) "Qt GuiPrivate ABI·glibc 하위호환 때문에 배포판별 재빌드가 필요하다"는 매트릭스의 존재 이유가 실제로 검증되고 있는가, (c) 외부 IME/데스크톱 프로젝트의 테스트 관례에서 이식 가능한 패턴, (d) 로컬 실험으로 실증한 헤드리스/격리 실행 가능성을 반영한 자동 기능 테스트 설계·로드맵
- 방법: 조사를 세 갈래(① 저장소 인벤토리 grep·코드 대조, ② 외부 프로젝트(fcitx5·ibus·Debian autopkgtest·Fedora CI·weston/gnome-shell) 소스·문서 조사, ③ 로컬 실세션·컨테이너에서의 격리 실행 실증 실험)로 병행한 뒤 종합했다. 본 문서 작성 시 핵심 수치(`--smoke` 실제 호출 2건, 매트릭스 6레그 정의, `%preun` 실행 검증 1건, `windowactivate` 사용처 1건)는 실코드로 재확인했다.

---

## 1. 현행 검수 지도

배포판×검증층 매트릭스. `○`=CI에서 자동 검증됨, `△`=부분/간접(경고만·다른 목적의 부산물), `✗`=현재 무검증(로컬 수동 또는 전무).

| 검증층 | ubuntu24.04 | ubuntu26.04 | debian13 | fedora43 | fedora44 | almalinux10(el10) |
|---|---|---|---|---|---|---|
| **빌드**(컴파일+링크 성공) | ○ | ○ | ○ | ○ | ○ | ○ |
| **패키징**(개수 11·0바이트 금지·버전 파일명 일치) | ○ | ○ | ○ | ○ | ○ | ○ |
| **설치**(apt/dnf install 실성공) | ○(`--smoke`) | ○(`--smoke`) | ○(`--smoke`) | ○(`--smoke`) | ○(`--smoke`) | ○(`--smoke`) |
| **제거 스크립틀릿 실행**(postrm/%preun) | ✗ | ✗ | ✗ | ○(erase로 %preun 실행+잔존확인) | ○ | ○ |
| **GTK3/4 IM 모듈 실로드**(GTK3: immodules 캐시 등록 / GTK4: gio 모듈 dlopen) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **Qt5/6 IM 모듈 실로드**(플랫폼 통합 플러그인 인식) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗(EPEL Qt5도 미검증) |
| **unim-daemon 기동·D-Bus 응답** | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **XIM 서버 동작** | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **기능 타이핑**(preedit→commit 종단, GTK/Qt/XIM) | ✗ | ✗ | ✗ | ✗ | ✗ | ✗ |
| **GNOME 확장 로드·activate()** | ✗(별개 경로, ②참고) | ✗ | ✗ | ✗ | ✗ | ✗ |
| **install.sh 배포판 감지·라우팅** | ✗(전 배포판 공통, 완전 수동) | | | | | |

빠진 층("실설치 개수 게이트" 이후 전부)의 위치를 파이프라인 순서로 보면:

```
빌드 ──▶ 패키징 게이트 ──▶ 실설치(--smoke) ──▶ [ 여기서 파이프라인이 끝난다 ]
                                                    ↓ 아래는 전부 무검증
                                     IM 모듈 실로드 · 데몬 기동 · XIM · 기능 타이핑
```

부연:
- `--smoke`는 `linux-deb.yml:133`·`linux-rpm.yml:115`에서 실제로 호출된다(호출 여부 자체가 이번 조사로 재확인된 사실 — 스크립트에 옵션이 있다고 워크플로가 쓴다는 보장은 아니었다). 두 호출 모두 매트릭스 전략(`matrix.tag`) 아래 단일 스텝이라 6레그 전부에 동일하게 적용된다.
- 빌드 단계(○) 자체도 무의미한 통과가 아니다 — `scripts/ci/bootstrap-rpm.sh`는 spec의 `BuildRequires`를 `dnf builddep`으로 해석시켜 "그 패키지명이 해당 배포판에 실재하는지"를 부트스트랩 단계에서 이미 검증하며(해석 실패 시 즉시 실패), el10은 CRB+EPEL 저장소를 추가로 켜야 Qt5/Qt6 `private-devel` 헤더가 잡힌다 — 이 사실은 §2 P0에서 el10을 최고위험 레그로 꼽는 근거를 보강한다.
- `check-gnome-api`·`smoke-gnome-extension`(`Makefile:571-582`, `make check-compat`)은 GNOME 확장 activate()까지 실행 검증하는 유일한 "실로드" 수준 검사이지만, `linux-ci.yml`·`linux-deb.yml`·`linux-rpm.yml` 어디에도 호출되지 않는다 — 존재는 하되 배선이 안 된 것으로, 표의 GNOME 확장 행을 "완전 무검증"이 아니라 "CI 미연결"로 특기한다.
- `linux-ci.yml`(develop/main 상시 게이트)은 `cargo test --workspace`를 돌리지만, 매트릭스 재빌드의 존재 이유인 `unim-im-gtk`(C, GTK3/4)·`unim-im-qt`(C++, Qt5/6)는 Cargo workspace 멤버가 아니라 이 커버리지 밖이다.
- `install.sh`의 배포판 감지·버전 라우팅을 실행하거나 검증하는 CI 잡·테스트는 저장소 전체에 0건이다(grep 전수 확인).

---

## 2. 구멍 목록 — 리스크 순위

우선순위는 "이 층이 뚫려 있을 때 사용자에게 도달하는 실패의 크기 × 매트릭스가 그 층을 검증한다고 암묵적으로 주장하는 정도"로 매겼다.

### P0 — Qt/GTK IM 모듈 실로드 미검증 (매트릭스의 존재 이유가 검증되지 않음)

`os-compatibility.md`의 배포판 매트릭스 도입 근거는 "glibc 하위호환 + Qt GuiPrivate ABI 때문에 배포판별 재빌드가 필요하다"는 것이다. 그런데 현재 파이프라인이 검증하는 것은:

1. 빌드 성공 — `find_package`가 헤더·라이브러리를 찾아 컴파일·링크가 끝남
2. 패키징 게이트 — 산출물 개수·0바이트·버전 파일명
3. 설치 성공 — `apt-get install`/`dnf install`이 의존성을 풀어 파일을 배치함

이 세 층 어디에도 "그 `.so`가 진짜 GTK IM 모듈로 등록되어 GTK 프로세스가 로드하는지", "그 `.so`가 진짜 Qt `QPlatformInputContext` 플러그인으로 인식되어 `QInputMethod`가 살아 있는지"를 실행해 확인하는 단계가 없다. `gtk-query-immodules-3.0` 실행(⚠️ GTK4 에는 이 도구가 없다 — 4.x 는 immodules 캐시 대신 gio extension point 로 등록하므로 dlopen 심볼 확인이 필요), `QT_IM_MODULE=unim` 상태의 최소 Qt 앱 기동, `qtdiag`류 플러그인 목록 확인 — 이런 커맨드가 `scripts/ci/*.sh` 전체에 0건이다(`gtk-query-immodules|immodules.cache|qt.conf|QT_PLUGIN_PATH|ldd` grep 결과 `scripts/sandbox.sh` 3건뿐, 이마저 CI 미연결 로컬 도구).

**결론**: "Qt GuiPrivate ABI 때문에 배포판별로 다시 빌드한다"는 매트릭스 전체의 존재 이유가, 정작 어느 배포판에서도 실제 로드로 증명되지 않는다. el10은 특히 Qt5가 EPEL 별도 패키지(RHEL 본체에서 제거됨)이고 `private-devel` 헤더조차 CRB+EPEL을 추가로 켜야 잡히는 구조라 ABI 불일치 리스크가 가장 높은 레그인데도 검증 강도는 다른 레그와 동일(무검증)하다. 같은 논리가 GTK IM 모듈에도 그대로 적용된다.

근거: `docs/dev/linux/os-compatibility.md`(배포판 매트릭스 섹션, 2026-08-24 도입) "glibc 하위호환 + Qt GuiPrivate ABI" 서술; `scripts/ci/build-deb.sh`·`scripts/ci/build-rpm.sh`(설치까지만 게이트); grep 전수(gtk-query-immodules 등) 결과.

### P0 — 데몬 기동·기능 타이핑 종단 검증 전무

`unim-daemon`이 실제로 뜨는지, D-Bus 인터페이스(`GetGlobalMode` 등)가 응답하는지를 확인하는 스텝이 CI 어디에도 없다(`unim-daemon|systemctl --user|daemon_alive|GetGlobalMode` grep — `scripts/ci/*.sh` 전체 0건). 그 위에 있는 "실제 키 입력 → preedit → commit"은 `tests/harness`가 로컬에서는 가능하지만 어느 워크플로에도 연결돼 있지 않다. 패키지가 "설치된다"는 것과 "입력기로 동작한다"는 것 사이의 간극이 완전히 비어 있다.

### P1 — 제거 스크립틀릿(postrm) 검증이 deb 레그 3곳에서만 빠짐

rpm 레그(fedora43/44·el10)는 erase 트랜잭션으로 `%preun`을 실제 실행해 잔존 파일 없음까지 확인하지만(`scripts/ci/build-rpm.sh:107-113`), deb 레그(ubuntu24.04/26.04·debian13)는 대응하는 `postrm` 실행 검증이 없다 — 개수 게이트만 본다. rpm 쪽에서 이미 확립한 패턴을 deb 쪽에 이식하지 않은 비대칭.

### P1 — GNOME 확장 로드 검증이 만들어져 있는데 배선만 안 됨

`make check-compat`(`check-gnome-api` + `smoke-gnome-extension`)은 이미 `dbus-run-session` + `gnome-shell --headless --virtual-monitor`로 로그아웃 없이 확장 activate()까지 실행 검증하도록 완성돼 있다. 그런데 어느 GitHub Actions 워크플로에도 걸려 있지 않다 — `os-compatibility.md`는 이를 지원 범위 보증 수단("규칙 3")으로 서술하지만, 실제로는 "릴리스 업그레이드 뒤 점검 순서"에 등장하는 개발자 수동 절차일 뿐이다. 문서와 실제 CI 배선 사이의 괴리 자체가 리스크(구현이 있는데 자동으로 안 돈다는 사실이 문서에서 드러나지 않음)다.

### P2 — install.sh 라우팅 완전 수동

배포판 감지(os-release 코드명→버전 매핑, "감지 버전 이하 최대 기준선" 선택, apt/dnf/rpm-ostree 3분기, 체크섬 검증) 로직 자체를 실행하는 테스트가 0건. 다만 이 로직은 순수 셸 함수라 컨테이너 없이도 단위테스트가 가능해 비용 대비 리스크는 P0~P1보다 낮게 잡았다(§4 참고).

### P3 — XIM·Wayland 네이티브 경로 검증 부재

XIM 서버 동작·Wayland 네이티브(GNOME 확장 경유가 아닌 순수 text-input-v3) 경로의 자동 검증도 없다. 다만 이 두 경로는 harness의 `xtest:False` 분류가 이미 시사하듯 자동화 난도 자체가 가장 높아(§3, §6 미해결 갭), 우선순위를 다른 항목들보다 낮췄다.

---

## 3. 외부 사례에서 이식할 패턴

랭킹은 (구축비용 낮음 → 높음) 순. 각 항목에 근거 URL을 병기한다.

### 3.1 fcitx5의 "가짜 프론트엔드" 인프로세스 테스트 — 최우선 권고

fcitx5 테스트 스위트의 주력은 실제 X11/Wayland 클라이언트 없이, `Fcitx5::Module::TestFrontend` + `Fcitx5::Module::TestIM`이라는 코어 내장 모듈에 합성 키 이벤트를 직접 주입하고 커밋/preedit 결과를 단정하는 순수 C++ ctest다(`testinputcontext`/`testinstance`/`testtempmode`/`testspell` 등 다수가 이 패턴). 디스플레이가 전혀 필요 없고 비결정성이 없다. 반대로 실제 XIM 프로토콜을 검증하는 `testxim`은 Xvfb로 구동하도록 짜여 있었음에도 "Test may fail on some system"이라는 FIXME 주석과 함께 `add_test` 자체가 주석 처리돼 비활성 상태다 — 업스트림조차 실디스플레이 기반 프로토콜 테스트를 신뢰하지 못해 폐기했다는 신호로 읽어야 한다.

근거: `https://github.com/fcitx/fcitx5/blob/master/test/CMakeLists.txt`, `https://github.com/fcitx/fcitx5/blob/master/test/xvfb_wrapper.sh`, `https://github.com/fcitx/fcitx5/blob/master/.github/workflows/check.yml`

UNIM 적용: 코어(Rust)에 실제 X11/Wayland/GTK/Qt 클라이언트 없이 합성 키 이벤트를 직접 주입해 preedit/commit을 단정하는 인프로세스 테스트 프런트를 신설하면, `tests/harness`가 겪는 실세션·좌표·mutter 포커스 문제를 프론트엔드 어댑터 레이어 바깥으로 밀어낼 수 있다.

### 3.2 Debian installed-tests(gnome-desktop-testing-runner) 관례

ibus 1.5.32-2의 `debian/tests/control`은 `Tests: installed-tests` / `Depends: ibus-tests` / `Restrictions: allow-stderr, needs-root, breaks-testbed`로 선언하고, `-tests` 서브패키지에 `gnome-desktop-testing-runner` 호환 테스트 바이너리를 담아 autopkgtest가 "설치된 패키지"를 대상으로 통합 테스트를 돌리는 GNOME류 관례를 그대로 쓴다. root가 필요하고 testbed를 깨뜨린다는 주석까지 명시돼 있다(`/var/lib/AccountsService/username` 파일 생성 부작용).

근거: `https://sources.debian.org/src/ibus/1.5.32-2/debian/tests/control/`, `https://wiki.debian.org/ContinuousIntegration/autopkgtest`

UNIM 적용: `scripts/build-linux-matrix.sh`가 이미 만든 6배포판 컨테이너를 그대로 재사용해, "설치된 .deb/.rpm" 레벨에서 IM 모듈 로드·기능 타이핑을 검증하는 통합 테스트를 붙이는 것과 같은 방향이다. 단, fcitx5(5.1.12-2 추정 경로 404)의 autopkgtest 존재 여부는 이번 조사로 미확인 — IME 프로젝트가 이 관례를 실제로 쓰는 실사례는 ibus 하나만 확보했다.

### 3.3 Fedora dist-git tmt/fmf 플랜 (Fedora 계열 한정)

Fedora 패키지는 dist-git에 `tests/` + `.fmf` 플랜을 직접 두거나 공유 저장소(`https://src.fedoraproject.org/tests/shell`)를 참조하고, Packit이 PR CI 역할을 한다. `src.fedoraproject.org/rpms/ibus/blob/rawhide/f/tests` 경로에 실제 tests 디렉터리 존재를 확인했다(내부 파일 목록은 미추출 — 갭).

근거: `https://docs.fedoraproject.org/en-US/ci/`, `https://src.fedoraproject.org/rpms/ibus/blob/rawhide/f/tests`

UNIM 적용: fedora43/44·el10 레그의 대응물이 될 수 있으나, Fedora 공식 패키지화(dist-git 편입)가 전제라 진입장벽이 있다 — 투자는 그 목표가 실제로 잡혔을 때만.

### 3.4 dbus-run-session + gnome-shell --headless --virtual-monitor — 이미 채택, 확장만 하면 됨

`scripts/smoke-gnome-extension.sh`가 이미 이 패턴이며, GNOME Shell 자체 CI(freedesktop-ci-templates 경유, Fedora 컨테이너 기반)와 궤를 같이한다. `gnome-shell`의 `.gitlab-ci.yml`은 GNOME/citemplates 및 freedesktop-ci-templates(`fedora.yml`, `ci-fairy.yml`)를 include하지만, 실제 테스트 잡 커맨드(Xvfb 사용 여부 등)는 이번 조사에서 미추출이다.

근거: `https://gitlab.gnome.org/GNOME/gnome-shell/-/blob/main/.gitlab-ci.yml`

UNIM 적용: 이미 구현이 있으므로 §2 P1(배선 누락)을 해소하는 것이 첫 걸음. 이 조합 위에 X11 프런트(GTK/Qt/XIM) 검증까지 얹는 확장은 §4에서 다룬다.

### 3.5 dogtail(AT-SPI) — harness 좌표 문제의 근본 해법 후보

`gitlab.com/dogtail/dogtail`은 Python 기반 AT-SPI GUI 자동화 프레임워크로, 좌표 대신 접근성 트리로 위젯을 찾고 조작한다. `tests/harness/harness.py`가 겪는 CSD 원점 불일치·HiDPI 좌표 이슈(`xdotool getwindowgeometry`가 GTK3 CSD에서 실측 (114,115) vs 실제 (100,66)로 어긋남)를 좌표 계산이 아니라 위젯 조회로 우회할 수 있다.

근거: `https://gitlab.com/dogtail/dogtail`

UNIM 적용: 중비용/고가치. `tests/unim-test-gtk4` 하나에 먼저 프로토타입해 좌표 문제가 실제로 사라지는지 검증하는 스파이크를 권고한다.

### 3.6 openQA류 스크린-드리븐 VM 테스트 — 비권장

`openqa.gnome.org`는 GnomeOS 대상 needle(이미지 매칭) 기반 VM 테스트다. 인프라(needle 라이브러리·VM)가 무겁고 현재 UNIM 규모에는 과함.

근거: `https://openqa.gnome.org/`

### 3.7 미해결 갭 (이번 외부 조사에서 확보 못한 것)

- fcitx5의 `debian/tests/control` 실존 여부 미확인(버전 경로 추정 실패, 404).
- `weston --headless-backend.so` 정확한 커맨드라인 미추출.
- GTK/Qt IM 모듈 "로드 검증" 자체의 업스트림 관례(broadway 백엔드 활용, `GTK_IM_MODULE` 강제 + 최소 앱 스모크 등) — ibus 소스에서 확인한 `GTK_IM_MODULE` 관련 코드(`client/gtk2/ibusimcontext.c`)는 내부 fake `GtkTextView` 생성용일 뿐 검증 관례가 아니었다. 이 항목은 완전한 갭으로 남는다 — 별도 세션에서 GTK testsuite·Qt `qplatforminputcontext` 테스트 코드를 직접 조사해야 한다.

---

## 4. 자동 기능 테스트 설계안

### 4.1 층별 옵션

**L1 — 패키지 새너티(이미 존재, 확장만 필요)**
- 대상: 개수·0바이트·버전 파일명 게이트(기존) + 제거 스크립틀릿 실행(deb 3레그 신설)
- 방법: `scripts/ci/build-deb.sh`의 `--smoke` 블록에 `apt-get remove unim*` 뒤 잔존 파일 확인을 추가해 rpm 레그의 `%preun` 패턴과 대칭을 맞춘다.
- 비용: 최저. 이미 실설치까지 하는 스모크 단계에 몇 줄 추가.

**L2 — 설치 후 런타임 로드(신설, 최우선 권고)**
- 대상: GTK3/4 IM 모듈이 `gtk-query-immodules`에 잡히는지, Qt5/6 플러그인이 로드되는지, `unim-daemon`이 기동해 D-Bus에 응답하는지.
- 방법: `--smoke` 블록에서 실설치 직후
  - GTK3: `gtk-query-immodules-3.0 --update-cache` 후 `grep unim`. GTK4: **`gtk-query-immodules-4.0` 은 존재하지 않는다** — GTK4 모듈은 `g_io_extension_point_implement` 로 등록되므로, 설치 경로의 `.so` 를 dlopen 하고 `g_io_module_load` 심볼을 확인하는 한 줄(python3 ctypes 또는 10줄 C)로 대체
  - Qt는 `QT_IM_MODULE=unim` 상태에서 최소 Qt 앱을 헤드리스(`-platform offscreen` 또는 Xvfb)로 기동해 플러그인 목록에 `unim`이 잡히는지 확인 — 정확한 커맨드는 Qt5/Qt6 플랫폼별로 별도 검증 필요(§6)
  - `unim-daemon &` 격리 기동(§4.3) 후 D-Bus `GetGlobalMode` 응답 확인
- 비용: 낮음~중간. 컨테이너 인프라 추가 불필요(이미 설치까지 하는 단계 뒤에 붙임), Qt 플랫폼 헤드리스 백엔드(offscreen)만 추가 조사 필요.
- 효과: 최대 — §2 P0(매트릭스 존재 이유 미검증)를 직접 해소.

**L3 — 헤드리스 기능 타이핑(harness 재활용, 격리화 필요)**
- 대상: `tests/unim-test-{gtk3,gtk4,qt,xim}` 4종을 XTEST(xdotool) 경유 실 키 입력으로 preedit→commit 종단 검증. `TEST_APPS.md`가 정의한 필수 회귀 시나리오 8종(basic-compose·commit-then-preedit·click-commit·focus-switch·password-suppress·backspace-decompose·english-passthrough·mode-toggle)이 1차 커버 대상이며, 전체 시나리오는 13종(레이아웃 변경 등 선택 시나리오 포함, `--allow-layout-change` 필요)이다.
- 방법: Xvfb(가상 프레임버퍼, host display 불필요) + `dbus-run-session`(버스 격리) + `UNIM_CONFIG_DIR`/`UNIM_DATA_DIR`/`UNIM_CACHE_DIR`(§4.3) 격리 기동 + `tests/harness/harness.py`(§4.2 수정 적용) 조합.
- 비용: 중간. 컨테이너에 `xvfb dbus-x11 x11-utils xdotool` 설치만 필요(§4.4 실증), WM 없는 환경에서도 harness 1줄 수정으로 동작 확인됨.
- 효과: 높음 — GTK3/4·Qt5/6·XIM 5경로의 실제 입력 정확성(두벌식 조합 등)까지 검증하는 유일한 층.

**L4 — 데스크톱 특화(GNOME 확장·Wayland 네이티브)**
- 대상: `unim-test-gnome`·`unim-test-wayland`(harness 분류상 `xtest:False`, XTEST 도달 불가).
- 방법: `gnome-shell --headless --virtual-monitor` 확장(§3.4)에 실제 키 주입 수단을 얹어야 하는데, 이번 로컬 실험에서는 실 데몬 오염 사고(§4.3)로 노이즈에 묻혀 결론을 내지 못했다. 후보는 `org.gnome.Mutter.RemoteDesktop`/`ScreenCast`(headless 세션에서 실제로 뜨는지 미확인) 또는 `gjs Eval`(`--unsafe-mode`) 경로로 `ClutterVirtualInputDevice`를 직접 구동하는 소형 GJS 헬퍼.
- 비용: 높음(구조 작업). libei/RemoteDesktop 기반 가상 입력 주입기를 신설하거나, harness의 `Injector`를 XTEST 전용에서 libei/virtual-input 백엔드까지 추상화하는 리팩터링이 선행돼야 한다.
- 효과: 중간 — GNOME 확장·순수 Wayland 경로는 데스크톱 점유율상 중요하지만, 사용자 규모 대비 자동화 투자 우선순위는 L1~L3보다 낮다.

### 4.2 harness 격리화 최소 수정 목록

로컬 실험(갈래 ③)으로 실증된, 코드 변경이 필요한 지점은 다음 하나뿐이다:

- `tests/harness/harness.py:151-153` `Injector.activate()`의 `xdotool windowactivate --sync`를 `xdotool windowfocus --sync`로 교체. `windowactivate`는 창관리자(WM)가 없는 환경(Xvfb/Xephyr 단독)에서 "windowmanager claims not to support _NET_ACTIVE_WINDOW"로 100% 실패하지만, `windowfocus`는 WM 없이도 포커스 획득→타이핑→판정까지 정상 동작함을 실증했다.
- `daemon_alive()`/`get_mode()`/`set_mode()`는 `DBUS_SESSION_BUS_ADDRESS`를, `Injector`/`find_window()`는 `DISPLAY`를 `os.environ`에서 그대로 상속하므로, 격리 셸에서 이 두 환경변수만 맞추면 harness.py/run.py 자체는 **무변경**으로 동작한다.

부수적으로 (코드 변경은 아니지만) 격리 실행 절차에 필요한 것:
- `scripts/sandbox.sh`의 `setup_local_modules()`가 GTK3 로컬 모듈로 `unim-frontends/gtk3/build/libim-unim.so`를 찾는데 실제 빌드 산출물은 `im-unim.so`(lib 접두어 없음, GTK3 관례)다 — 이 조건이 항상 false라 GTK3 sandbox가 로컬 빌드 모듈이 아니라 시스템 설치본을 계속 쓴다. CI 컨테이너처럼 시스템 설치가 없는 환경에서는 GTK3 IM 모듈을 못 찾는 실패로 이어질 수 있는 잠재 버그(`scripts/sandbox.sh:95-97`).

### 4.3 실행 환경 격리 — 실증된 조합과 안전 함정

`src/paths.rs:15-35`의 `config_dir()`/`data_dir()`/`cache_dir()`은 원래 Windows UWP 리다이렉션 우회용으로 `UNIM_CONFIG_DIR`/`UNIM_DATA_DIR`/`UNIM_CACHE_DIR` 환경변수를 `dirs::*`보다 우선하도록 이미 구현돼 있다 — 이 세 변수만 스크래치 디렉터리로 export하면 harness/데몬 코드 변경 없이 실 설정과 완전히 분리된다(로컬 실험에서 fresh config 확인됨).

**안전 함정(실사고, 재발 방지 필요)**: 헤드리스 `gnome-shell`에 `HOME`/`XDG_CONFIG_HOME`을 격리하지 않고 실험했더니, 이 개발자의 실제 활성화된 `unim-gnome` 확장을 그대로 로드해 `org.atit.unim.InputMethod` D-Bus 접근이 격리 버스 안에서도 시스템 서비스파일을 통해 실제 `/usr/libexec/unim-daemon`을 D-Bus 활성화시켰고, 그 데몬이 실 `~/.config/ibus/bus/`에 주소 파일을 실제로 생성했다(확인 후 삭제 완료, `~/.config/unim/config.yaml` mtime 자체는 불변 — `GetAll` 읽기만 발생, `Set` 계열 호출 없었음). `dbus-run-session`의 버스 격리만으로는 불충분하며, **HOME까지 스크래치로 돌려야 "실데몬 무접촉"이 보장된다** — 이 점은 CI 컨테이너(dconf가 비어 있어 재현 안 됨)에서는 문제가 안 되지만, 개발자가 로컬에서 유사 실험을 반복할 도구를 만든다면 하네스 표준 문서(`TEST_APPS.md` 등)에 명문화해야 한다.

### 4.4 실행 형태 — 로컬(make 타깃) vs CI(매트릭스 leg)

**로컬(이 기계, 실세션)**: `dbus-run-session` + Xephyr(ambient `DISPLAY` 위에 얹힘, `-ac -host-cursor`) + `UNIM_CONFIG_DIR`/`DATA_DIR`/`CACHE_DIR` 격리 + `target/release/unim-daemon -n`(격리 기동) + `tests/unim-test-gtk3`(`GTK_IM_MODULE=unim`)를 XTEST(xdotool)로 클릭→포커스→키입력까지 실행하는 전 구간이 지금 그대로 동작함을 실증했다. JSONL 로그에 실제 두벌식 조합(h→'ㅗ', k→'ㅘ')이 `field.render`로 찍혔고, 실데몬(pid 별도)은 무접촉으로 유지됐다. `scripts/sandbox.sh` 인프라를 거의 재사용 가능(단 §4.2의 windowfocus 수정과 GTK3 모듈명 수정이 전제). 신규 셸 스크립트 1개 + Makefile 타깃 1개(예: `make check-runtime-x11`)로 감쌀 것을 권고 — 코드 변경 없이 셸 레벨에서 격리를 보장.

**CI(매트릭스 leg, GHA 컨테이너)**: 순수 `docker run --rm ubuntu:24.04`(ambient `DISPLAY`/`WAYLAND_DISPLAY` 전무) 컨테이너에서 `apt-get install -y xvfb dbus-x11 x11-utils xdotool`만으로 Xvfb 기동 성공, `xdotool key` 주입 성공까지 실증했다(uinput/DRM/GPU 불필요, `--privileged` 불필요). Xephyr는 host display가 있어야 얹히므로("Xephyr cannot open host display" 실패, 실증) 순수 headless 컨테이너에는 Xvfb가 유일한 선택지다. `scripts/build-linux-matrix.sh --smoke` 뒤에 §4.1 L2/L3 단계를 잇는 잡을 이어 붙이면 된다 — 이미 실설치 검증까지 하는 파이프라인의 다음 스텝.

별도로, `gnome-shell --headless --virtual-monitor` + 수동 `Xwayland :99 -rootless` 조합도 ambient display 0개인 완전 headless로 동작함을 실증했다(`xdpyinfo`/`xdotool key` 성공). Xvfb보다 순수하지만 gnome-shell/mutter 풀스택 설치 무게가 훨씬 크다 — X11 프런트(GTK/Qt/XIM) 검증에는 Xvfb가 더 실용적이고, 이 조합은 L4(GNOME 확장·Wayland 네이티브 경로) 자체를 얹을 셸이 필요할 때만 의미가 있다.

### 4.5 추천

L1(확장)과 L2(신설)를 최우선으로 권고한다 — 구축비용이 가장 낮으면서 §2 P0(매트릭스 존재 이유 미검증)를 직접 해소하기 때문이다. L3는 대표 레그(예: ubuntu24.04·fedora43) 한정으로 좁혀 시간 비용을 관리하는 것을 권고한다(§5). L4는 구조 작업 완료 후.

---

## 5. 단계별 로드맵

### 지금 바로 (구축 난도: 낮음, CI 시간: +수십 초~수 분)

- deb 레그 3곳에 제거 스크립틀릿(postrm) 실행 검증 추가 — rpm 쪽 `%preun` 패턴 이식(§2 P1).
- `--smoke` 블록에 GTK3 는 `gtk-query-immodules-3.0 --update-cache | grep unim` 한 줄, GTK4 는 dlopen 심볼 확인 한 줄(위 §4.1 L2 참조) 추가(6레그 전부) — L2의 GTK 절반. (초안의 `gtk-query-immodules-4.0` 은 실존하지 않는 도구라 교정함)
- `linux-ci.yml`에 `check-compat`(`check-gnome-api` + `smoke-gnome-extension`) job 추가 — 이미 로그아웃 불필요·headless로 설계돼 있어 배선만 하면 됨(§2 P1, §3.4).
- CI 컨테이너 부트스트랩에 `xvfb dbus-x11 x11-utils xdotool` 설치 스텝 추가(RPM 레그는 `xorg-x11-server-Xvfb` 등, 미검증·추정 — 확인 필요) — L3 착수의 전제 조건.

### 작은 수정 (구축 난도: 중간, CI 시간: leg당 +수 분)

- `tests/harness/harness.py:151-153` `windowactivate`→`windowfocus` 1줄 교체(§4.2) — WM 없는 환경 필수, 이미 실증됨.
- `scripts/sandbox.sh`의 GTK3 모듈 파일명 오타 수정(`libim-unim.so`→`im-unim.so`).
- Qt5/6 IM 모듈 실로드 확인 커맨드 확정(offscreen 플랫폼 또는 Xvfb 기반 최소 앱 + 플러그인 목록 확인) — L2의 Qt 절반, 정확한 커맨드는 아직 미검증(§6).
- `unim-daemon` 격리 기동 + D-Bus `GetGlobalMode` 스모크를 `--smoke` 블록 또는 별도 잡으로 추가(§2 P0 데몬 기동 항목).
- 신규 셸 스크립트 1개 + Makefile 타깃 1개로 로컬 격리 실행(§4.4)을 감싸고, `UNIM_CONFIG_DIR`/`DATA_DIR`/`CACHE_DIR`(+ 가급적 HOME) 스크래치 export를 표준화.
- `tests/unim-test-{gtk3,gtk4,qt,xim}` 4종을 Xvfb+dbus-run-session 조합으로 대표 레그(ubuntu24.04·fedora43 등) 한정 CI 잡으로 연결 — L3 착수(§4.5 추천 범위 축소안).
- install.sh 배포판 감지·라우팅에 대한 셸 함수 단위테스트(os-release 픽스처 여러 개 + `pick_manifest`/버전비교 함수만 소싱해 검증) — 컨테이너 불필요, 현재 전무(§2 P2).

### 구조 작업 (구축 난도: 높음, CI 시간: 설계에 따라 가변)

- L4(GNOME 확장·Wayland 네이티브) 실 키 주입 수단 확보 — `RemoteDesktop`/`ScreenCast` headless 가용성 재실험(§4.3의 오염 사고로 결론 미확보) 또는 `gjs Eval` 기반 `ClutterVirtualInputDevice` 헬퍼 신설.
- `tests/harness`의 `Injector`를 XTEST 전용에서 libei/virtual-input 백엔드까지 추상화하는 리팩터링.
- fcitx5식 인프로세스 "가짜 프론트엔드" 테스트(§3.1) — UNIM 코어(Rust)에 실제 클라이언트 없이 합성 키 이벤트를 직접 주입하는 신규 테스트 인프라. 구축비용은 절대치로는 낮지만(수백 줄), 기존 harness/데몬 아키텍처와의 통합 설계가 필요해 "구조 작업"으로 분류 — 다만 우선순위 자체는 §3.1에서 최고로 평가했다는 점에 유의(비용 대 가치가 가장 좋은 항목).
- 6배포판 컨테이너 전부에 대해 PR마다 harness 풀 매트릭스(13개 시나리오 × 6~7개 앱)를 돌리는 설계는 시간 비용이 커서, 대표 레그 한정 "런타임 회귀 게이트"로 범위를 좁힐지에 대한 명시적 설계 결정이 선행돼야 한다.
- 이번에 발견된 안전 함정(§4.3, 헤드리스 gnome-shell이 로컬 dconf 확장설정을 물려받아 실 데몬을 격리 버스 안에서 D-Bus 활성화)을 로컬 도구화할 경우, HOME까지 스크래치로 격리하는 것을 하네스 표준 문서에 명문화.

---

## 6. 잔여 불확실성

- **GTK4 모듈 실로드 검증의 정확한 형태 미확정** — immodules 캐시가 없어 dlopen 심볼 확인으로 대체를 제안했지만, GTK4 가 실제 IM 컨텍스트로 채택하는지까지 보증하려면 `GTK_IM_MODULE=unim` 최소 앱 기동(L3 축소판)이 더 정직한 검사다. 구현 시 둘 중 택1.

- **Qt5/6 헤드리스 로드 확인 커맨드 자체가 미검증**: `QT_IM_MODULE=unim` + offscreen/Xvfb 조합으로 실제 `unim` 플러그인이 잡히는지는 이번 조사에서 실행해보지 않았다 — L2 착수 전 별도 스파이크 필요.
- **RPM 레그의 Xvfb 패키지명 미확인**: fedora/el10에서 GTK/Qt 헤드리스 검증에 쓸 X 서버 패키지가 `xorg-x11-server-Xvfb`인지 다른 이름인지 확인하지 않았다.
- **L4(GNOME 확장·Wayland 네이티브) 실 키 주입 수단 미확정**: `RemoteDesktop`/`ScreenCast`가 headless gnome-shell 세션에서 실제로 뜨는지는 이번 실험이 안전 함정(실 데몬 오염)에 막혀 결론을 못 냈다 — 재시도 필요.
- **GTK/Qt IM 모듈 로드 검증의 업스트림 관례**: broadway 백엔드, `GTK_IM_MODULE` 강제 스모크 등 참고할 만한 기존 관례를 이번 외부 조사에서 찾지 못했다(§3.7). GTK testsuite·Qt `qplatforminputcontext` 테스트 코드를 직접 조사하는 후속 세션이 필요하다.
- **fcitx5 autopkgtest 실존 여부**: 버전 경로 추정(5.1.12-2)이 404로 실패해 확인하지 못했다. Debian installed-tests 관례(§3.2)를 실제로 쓰는 IME 프로젝트 실사례는 ibus 하나만 확보된 상태다.
- **weston `--headless-backend.so` 정확한 커맨드라인, gnome-shell `.gitlab-ci.yml`의 실제 테스트 잡 스크립트**(Xvfb 사용 여부 포함)는 미추출.
- **6레그 전체 harness 풀 매트릭스의 실제 CI 소요 시간**: 대표 레그 축소안(§4.5, §5)의 구체적 시간 예산은 실측 없이 추정치로만 다뤘다 — L3 착수 후 실측 필요.
- **el10 Qt5(EPEL) ABI 리스크의 실제 크기**: §2에서 "가장 높은 레그"로 지목했으나, 이는 RHEL 본체에서 Qt5가 제거됐고 EPEL이 별도 패키징한다는 구조적 사실에서 도출한 추론이지, 실제 로드 실패를 관측한 근거는 아니다(현재 로드 자체가 어느 레그에서도 검증되지 않으므로 관측이 원천적으로 불가능하다는 점이 이번 보고서의 핵심 발견이기도 하다).
