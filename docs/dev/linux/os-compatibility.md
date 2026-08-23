# 배포판·데스크톱 하위호환 규칙

UNIM 은 한 소스로 **여러 세대의 우분투와 GNOME** 을 동시에 받친다. 개발 기계가
새 LTS 로 넘어갔다고 해서 옛 LTS 사용자가 떨어져 나가면 안 되고, 반대로 새
LTS 에서 조용히 죽어 있어도 안 된다. 이 문서는 그 두 방향을 어떻게 지키는지를
적는다.

## 왜 이 문서가 있나

2026-08-23, 개발 기계가 우분투 24.04 에서 26.04 로 올라가면서 두 가지가 한꺼번에
드러났다.

빌드가 먼저 죽었다. 릴리스 업그레이드가 `-dev` 패키지 8종을 떨궈서
`glib-2.0.pc` 가 사라졌고, `glib-sys` 의 build.rs 가 첫 관문에서 멈췄다. 이건
알아채기 쉬운 실패다 — 컴파일러가 소리를 지른다.

**나머지 하나가 진짜 문제였다.** GNOME Shell 50 이 전역
`Meta.is_wayland_compositor()` 를 없앴는데, 확장의 JS 는 컴파일 검사를 받지
않으므로 빌드도 테스트도 전부 초록이었다. 확장 목록에는 "사용 중" 으로
표시되는데 `enable()` 이 중간에 예외로 끝나 IME 등록이 통째로 롤백돼 있었다.
증상은 "한 글자도 입력되지 않음" 하나뿐이고, 어디에도 실패 표시가 없었다.

교훈은 이것이다. **타입 검사가 없는 층에서는 환경 변화가 침묵으로 온다.**
그래서 침묵을 깨는 검사를 따로 세워야 한다.

## 지원 범위

| 층 | 범위 | 어떻게 지키나 |
|---|---|---|
| 우분투 | 현행 LTS 와 다음 LTS | CI 두 잡 — 러너(현행) + 컨테이너(다음) |
| GNOME Shell | 45 ~ 50 | `metadata.json` 의 `shell-version` + 기능 탐지 |
| GTK | 3 / 4, 버전 하한 없음 | `pkg-config` 로 잡고 버전을 박지 않음 |
| Qt | 5 / 6, 버전 하한 없음 | `find_package` 가 찾는 것을 씀 |
| Rust | 1.78 이상 | `Cargo.toml` 의 `rust-version` |

버전 하한을 **일부러 적지 않는** 곳이 많다. 하한은 지킬 수 있을 때만 뜻이 있고,
검사하지 않는 하한은 문서에 적힌 거짓말이 된다. 대신 CI 가 실제로 두 세대를
빌드해서 사실로 증명한다.

## 규칙

### 1. GNOME 확장에서 API 를 직접 부르지 않는다

셸이 없앤 API 를 그냥 부르면 `enable()` 이 통째로 죽는다. 기능 탐지로 감싸고,
새 경로 → 옛 경로 → 마지막 보루 순으로 물러난다. `extension.js` 의
`isWaylandCompositor()` 가 본보기다.

```js
const context = global.backend?.get_context?.();
if (context?.get_wayland_compositor)                 // 셸 50 이후
    return context.get_wayland_compositor() !== null;
if (Meta.is_wayland_compositor)                      // 셸 49 이하
    return Meta.is_wayland_compositor();
return GLib.getenv('XDG_SESSION_TYPE') === 'wayland'; // 그 밖
```

폴백을 쓸 때 **마지막 보루가 틀리면 어떻게 되는지**를 주석에 적는다. 위의 경우
잘못 판정하면 팝업이 안 뜨거나 두 번 그려진다 — 조용히 이상해지는 종류라서,
읽는 사람이 그 대가를 알고 손대야 한다.

### 2. 셸이 올라가면 `shell-version` 을 늘린다

`unim-gnome-extension/metadata.json` 의 배열에 새 메이저를 넣지 않으면 셸이
`OUT OF DATE` 로 **로드 자체를 거부한다**. 늘리기 전에 아래 검사를 통과시킨다 —
버전만 늘리고 API 가 깨져 있으면 1번 사고를 그대로 재현한다.

### 3. 두 검사를 통과시킨다

```
make check-compat        # 아래 둘을 함께
make check-gnome-api     # 정적 — 쓰는 GI 심볼이 이 셸에 실재하는가
make smoke-gnome-extension   # 동적 — headless 셸에서 끝까지 활성화되는가
```

둘 다 **로그아웃을 요구하지 않는다.** 동적 쪽은 `dbus-run-session` 으로 세션
버스를 갈라 headless 셸을 따로 띄우므로 실제 세션의 셸도 데몬도 건드리지 않는다.

정적 검사만으로는 부족하다. 심볼이 다 있어도 `enable()` 안의 순서나 논리가
틀리면 여전히 조용히 죽는다. 동적 검사만으로도 부족하다. 활성화 경로를 지나지
않는 코드(팝업·한자 변환 등)의 심볼은 실행되지 않아서 안 걸린다. **둘이 겹쳐야
덮인다.**

GNOME 이나 `gjs` 가 없는 기계에서는 두 스크립트가 스스로 건너뛰고 통과한다 —
빌드 서버에서 이것 때문에 빨개지지 않게.

#### 예외 마커

옛 API 의 **존재를 확인하는** 코드는 부르는 것이 아니라 묻는 것이다. 그 줄
끝에 `// api-check: allow` 를 달면 정적 검사가 통과시킨다. 마커는 폴백 안에서만
쓴다 — 검사를 조용히 시키려고 붙이기 시작하면 검사가 없는 것과 같아진다.

### 4. 다음 LTS 를 CI 에서 미리 밟는다

`linux-ci.yml` 의 `build-next-lts` 잡이 다음 LTS 컨테이너에서 빌드와 테스트를
돌린다. 러너 이미지가 아니라 컨테이너인 이유는, GitHub 이 새 LTS 러너를 내주기
전부터 검사할 수 있어야 하기 때문이다.

LTS 가 바뀌면 그 잡의 이미지 태그를 올리고, 기존 러너 잡은 **그대로 둔다** —
옛 LTS 사용자가 남아 있는 동안은 그쪽이 지켜야 할 계약이다.

## 릴리스 업그레이드 뒤 점검 순서

개발 기계를 새 우분투로 올린 직후에는 이 순서로 확인한다. 위에서부터 확인해야
아래 단계의 실패를 오진하지 않는다.

1. **빌드 의존성이 살아 있나** — 릴리스 업그레이드는 `-dev` 패키지를 떨군다.
   ```
   for p in $(sed -n '/^Build-Depends/,/^Standards-Version/p' debian/control |
              grep -oE '^ +[a-z0-9.+-]+' | tr -d ' '); do
       dpkg-query -W -f='${Status}' "$p" 2>/dev/null |
           grep -q '^install ok installed' || echo "MISSING $p"
   done
   ```
2. **`make build` 와 `cargo test --workspace`**
3. **`make check-compat`** — 확장이 새 셸에서 살아 있는지
4. **`scripts/gen-compile-commands.sh`** — clangd 의 시스템 헤더 경로가 바뀌었다.
   다시 돌리지 않으면 편집기가 옛 경로를 붙들고 헛것을 본다
5. 실제 로그인 세션에서 한 번 쳐 본다. Wayland 에서는 셸 재시작이 안 되므로
   **로그아웃 → 로그인** 이 필요하다

## 알려진 흠

- **GTK4 4.22 의 deprecation 3건** — `unim-frontends/gtk4/src/immodule.c` 의
  `gdk_x11_display_get_xdisplay`·`gdk_x11_surface_get_xid`. 빌드는 통과하지만
  경고가 난다. 대체 API 가 X11 창 ID 취득과 얽혀 있어 조사가 필요하다.
- **`tests/common` 의 CMake 구성 실패** — clangd 컴파일 데이터베이스에서만
  빠지고 본 빌드에는 영향이 없다.
