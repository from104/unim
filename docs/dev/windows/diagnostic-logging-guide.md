# Windows 진단 로그 활성화 가이드

> 상태: **현행** · 최종 업데이트 2026-08-26 (D-3)
> 대상 독자: 이 문서 **자체**는 개발자가 사용자에게 그대로 링크해 줄 수 있는 수준으로 쓴다 —
> 비개발자가 따라 해서 로그를 모아 보내는 데 필요한 절차만 담는다. UNIM 자체 진단(코드
> 흐름, 락 순서 등)이 필요하면 이 폴더의 다른 문서(`windows-console-composition-bug.md`,
> `TSF_INPUT_FIX_PLAN.md` 등)를 본다.
>
> 사용자 대상 트러블슈팅 전체 흐름(증상별 1차 진단)은
> [`docs/user/troubleshooting/README-ko.md`](../../user/troubleshooting/README-ko.md#windows) 가
> SoT 다 — 이 문서는 그 "진단 로그 켜기" 절차만 발췌 + D-3(배너·회전)로 갱신된 세부를
> 덧붙인 버전. 사용자 문서가 갱신되면 이 문서도 함께 갱신할 것(내용 포크 금지 —
> 절차 자체를 고치려면 사용자 문서를 먼저 고치고 여기 반영한다).

## 1. 켜기

관리자 권한 불필요 (사용자 환경 변수):

```bat
setx UNIM_DEBUG_LOG 1
```

- 환경 변수는 **프로세스가 시작될 때 한 번만** 읽힌다. 켜기 전에 이미 떠 있던 앱은 이 값을
  못 본다 — 진단할 앱을 완전히 종료 후 재실행하거나, 확실히 하려면 로그아웃 후
  재로그인한다.
- 로그 파일: `%TEMP%\unim-tsf.log`. 여러 앱이 같은 파일에 이어 쓰며 `[unim-tsf <PID>]`
  태그로 구분된다.
- 다른 구성 요소는 각자 파일에 남긴다 — 팝업 렌더러 `unim-popup-win.log`, 설정 앱
  `unim-settings.log`.
- 콘텐츠(실제 입력 문자열)까지 필요하면 `setx UNIM_DEBUG_CONTENT 1` 을 **추가로** 켠다 —
  비밀번호 포함 평문 키스트로크가 남으므로 진단 요청을 받았을 때만, 끝나면 즉시 끈다.

두 게이트 모두 기본 OFF(무음·비용 0). 구현: `unim-tsf/src/register.rs` 의
`logging_enabled()` / `content_enabled()` (env 값은 프로세스당 1회 캐시).

## 2. 로그 첫 줄 — 배너 (D-3)

`UNIM_DEBUG_LOG=1` 상태로 UNIM 이 프로세스에 처음 로드되면(TIP `Activate`), 다른 어떤
이벤트 로그보다 먼저 배너 한 줄이 찍힌다:

```
[unim-tsf 12345] ===== UNIM startup banner ===== version=0.4.1 build=2026-08-26 03:00:00 UTC dll=C:\Program Files\UNIM\unim_tsf.dll dll_mtime_epoch=1756166400
```

- `version` — `CARGO_PKG_VERSION` (릴리스 버전).
- `build` — 컴파일 타임 UTC 타임스탬프(`unim-tsf/build.rs` 가 `UNIM_BUILD_TIMESTAMP` env 로
  굽는다). "사용자가 재빌드된 DLL 을 실제로 받았는가"를 버전 번호 없이도(개발 중 빌드는
  버전이 안 바뀔 수 있다) 확정하는 용도.
- `dll` / `dll_mtime_epoch` — 실제 로드된 DLL 경로와 그 파일의 mtime(Unix epoch 초). 여러
  설치본이 섞였거나(구버전 잔존, 잘못된 경로 등) MSI 재설치가 실제로 파일을 갈아치웠는지
  확인하는 용도.
- **프로세스당 정확히 1회**만 찍힌다(`OnceLock` — 같은 프로세스에서 여러 문서/스레드가
  동시에 Activate 해도 중복 없음). 그래서 로그 맨 앞에서 찾아야 한다 — 중간에 재현한
  부분만 스크롤해서 잘라 보내면 배너가 잘려 나갈 수 있다.

구현: `unim-windows-common/src/debug.rs::log_startup_banner` (게이트 없는 저수준) +
`unim-tsf/src/register.rs::log_startup_banner` (얇은 wrapper, `UNIM_DEBUG_LOG` 게이트) +
호출부 `unim-tsf/src/text_service.rs` `ActivateEx`.

## 3. 로그 파일 크기 — 5MB 회전 (D-3)

`unim-tsf.log` 는 5MB 를 넘기면 그 시점에 `unim-tsf.log.1` 로 이름을 바꾸고(기존 `.1` 은
덮어씀 — **1세대만 보관**) 새 빈 파일로 이어 쓴다. 무회전 누적으로 로그가 무한정 자라던
과거 동작을 막는다.

**진단 요청 시 유의**: 오래 켜 둔 세션이면 배너(§2, 프로세스 시작 시 1회)가 이미
`unim-tsf.log` 밖으로 밀려나 `unim-tsf.log.1` 에만 남아 있을 수 있다 — 두 파일 다 요청한다.

구현: `unim-windows-common/src/debug.rs::dbg_log` (스레드-로컬 캐시에 파일 핸들 + 현재
크기를 같이 들고 있다가, 다음 줄을 쓰면 5MB 를 넘길 때 회전).

## 4. 수집 + 전달 (사용자에게 그대로 전달할 명령)

```bat
copy "%TEMP%\unim-tsf.log" "%USERPROFILE%\Desktop\unim-report-tsf.log"
copy "%TEMP%\unim-tsf.log.1" "%USERPROFILE%\Desktop\unim-report-tsf.log.1"
copy "%TEMP%\unim-popup-win.log" "%USERPROFILE%\Desktop\unim-report-popup.log"
copy "%APPDATA%\unim\config.yaml" "%USERPROFILE%\Desktop\unim-report-config.yaml"
```

(`.1` 파일이 없으면 `copy` 가 "찾을 수 없습니다" 라고만 뜨고 지나간다 — 정상, 회전이
아직 안 일어난 것이다.)

> ⚠️ **첨부 전에 반드시 열어서 훑어본다.** `UNIM_DEBUG_CONTENT` 를 함께 켰다면 실제로
> 입력한 문자열이 로그에 남아 있다. 비밀번호·개인 정보가 보이면 지우고 보낸다.

## 5. OnTestKeyDown / OnKeyDown 진입 로그

키 이벤트가 sink 에 도달하는지(=IME 가 아예 안 불리는 케이스와 구분) 확인용 무조건
로그가 이미 있다 — `UNIM_DEBUG_LOG=1` 이면 매 키마다:

```
[unim-tsf 12345] OnTestKeyDown ENTER vk=0x42 ctrl=false alt=false shift=false super=false
[unim-tsf 12345] OnKeyDown ENTER vk=0x42
```

`OnTestKeyDown ENTER` 는 찍히는데 `OnKeyDown ENTER` 가 전혀 안 찍히면, 그 키는 항상
`eaten=false` 로 앱에 패스쓰루되고 있다는 뜻이다(TSF 정상 동작 — 대부분의 단축키/조합
키가 이런 식으로 앱에 도달한다. 문제가 아니라 진단 기준선이다).

구현: `unim-tsf/src/key_handler.rs::test_key_down` 진입부, `unim-tsf/src/text_service.rs`
`OnKeyDown` 진입부, 둘 다 `crate::register::dbg_log_ev!` 매크로 사용.
