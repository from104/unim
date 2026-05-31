# Windows 콘솔/IMM32 앱 한글 조합 버그 (wezterm·텔레그램 등)

> 상태: **부분 해결(폴백 구현 중)** · 최종 업데이트 2026-06-01
> 관련 조사: `docs/wezterm-composition-research.md`, `docs/wezterm-composition-research-3rd.md`

## 1. 증상

UNIM TSF(`unim-tsf`)로 한글 입력 시, **특정 앱에서 조합(composition)이 유지되지 않고 중간 글자가 문서에 잔류**한다.

- `ㅎ ㅏ ㄴ ㄱ ㅡ ㄹ`("한글")을 입력하면 화면에 **`ㅎ하한ㄱ그글`** 처럼 모든 중간 단계가 남는다.
- preedit(밑줄 조합 글자)가 뜨지 않고, 각 자모/음절이 개별 확정된 것처럼 보인다.

### 재현 환경
- **wezterm** (확인됨)
- **Telegram Desktop** (확인됨 — 동일 증상)
- 공통점: 둘 다 **자체 TSF text store(ITextStoreACP)를 구현하지 않는** 앱. Windows의 CUAS(Cicero Unaware Application Support, msctf의 IMM32↔TSF 브리지)를 통해 입력을 받는다.

### 정상 동작 환경 (대조군)
- 메모장, 일반 Win32/WinUI 편집 컨트롤 등 정식 TSF text store 보유 앱 → UNIM 정상.
- **MS 기본 한국어 IME는 wezterm·텔레그램에서도 정상** (MS IME도 순수 TSF TIP이지만 IMM32 네이티브 폴백 경로를 함께 가짐).

## 2. 근본 원인 (실측 로그 + 1차 자료로 확정)

진단 로그(`%TEMP%\unim-tsf.log`, `register::dbg_log`, `UNIM_DEBUG_LOG=true`)로 다음을 확정:

```
handle_key_down: preedit_changed=true was_composing=false comp_active=false
preedit branch: START preedit_len=1
acquire_insert_range: InsertAtSelection QUERYONLY ok
StartComp.DoEditSession: StartComposition ok       ← 성공 (hr=Ok(0))
start_composition: composition CREATED             ← composition 객체 생성됨 (NULL 아님)
OnCompositionTerminated: ...                        ← StartComposition 직후 즉시!
```

- 모든 TSF 호출(`InsertTextAtSelection`/`SetText`/`StartComposition`/`RequestEditSession`)이 `hr=Ok(0)` 으로 **성공**하고 composition 객체도 생성된다(거부 아님).
- 그러나 wezterm/텔레그램(CUAS 경유)은 **StartComposition 직후 곧바로 `ITfCompositionSink::OnCompositionTerminated`를 호출**해 composition을 강제 종료시킨다.
- `OnCompositionTerminated`는 "우리(서비스) 외의 주체가 composition을 끝낼 때만" 호출되는 콜백(MS Learn 확증) → 이 콜백 수신 자체가 **"이 앱은 composition을 유지 못 함"** 신호.

### 기각된 가설 (추측 아닌 실측으로)
1. ~~caret/SetSelection 방식 (collapsed-end → range 전체 TF_AE_NONE)~~ — 변경해도 증상 동일. **기각.**
2. ~~TF_ES_SYNC edit session 거부~~ — `hr=Ok(0)`로 승인됨. **기각.**
3. ~~display attribute(GUID_PROP_ATTRIBUTE) 미설정~~ — 밑줄(시각)에만 영향, composition 수명과 무관. **기각(약화).**
4. ~~wezterm이 TSF를 구조적으로 전혀 미지원~~ — MS IME가 되는 것과 모순. 실제로는 CUAS로 TSF를 받음. **기각.**

### UNIM 측 2차 결함 (자폭)
원래 `OnCompositionTerminated` 핸들러가 `engine.reset()`을 무조건 호출 → 강제 종료를 받을 때마다 한글 엔진 버퍼를 비워, `preedit_len`이 영원히 1에 머묾(`ㅎ`+`ㅏ`가 `하`로 안 모임). **이건 UNIM 코드 결함이라 수정함**(아래).

## 3. 해결 방향

wezterm/텔레그램은 composition을 구조적으로 유지 못 하므로(코드로 강제 불가), **앱 감지 + backspace 재삽입 폴백**을 사용한다.

### 단계별 수정 이력
1. **`OnCompositionTerminated`에서 `engine.reset()` 제거** → 엔진 버퍼 보존. 결과: `ㅎ→하→한` 누적은 되나 중간 글자 잔류(`ㅎ하한`).
2. **폴백 모드 신설** (현재):
   - `OnCompositionTerminated`가 **마지막 키 입력 후 200ms 내** 발생하면 = "composition 미지원 앱"으로 판정(`composition_unsupported` 플래그 set). 한참 뒤 종료는 포커스 이탈 등 정상 종료로 보고 `engine.reset()`.
   - 폴백 모드에서는 composition을 만들지 않고, 매 키마다 **이전 미확정 글자(`fallback_pending`개)를 삭제하고 `[확정문자 + 새 preedit]`를 직접 삽입**(`comp_mgr.replace_surrounding`, AutoTypeFix와 동일한 검증된 경로). 정상 앱(메모장 등)은 기존 composition 경로 그대로.

### 관련 파일
- `unim-tsf/src/text_service.rs` — `composition_unsupported`/`fallback_pending`/`last_key_instant` 상태, `OnCompositionTerminated` 분기(즉시 vs 정상), `OnKeyDown` 타임스탬프.
- `unim-tsf/src/key_handler.rs` — `handle_key_down` 폴백 분기(backspace+reinsert) vs 정상 composition 분기.
- `unim-tsf/src/composition.rs` — `replace_surrounding`(삭제+삽입), `select_composition_range`(정상 경로 caret).

## 4. 검증 방법

진단 로그 활성 상태(`UNIM_DEBUG_LOG=true`)에서:
```powershell
powershell -ExecutionPolicy Bypass -File .\scripts\unim-tsf-reload-log.ps1
```
→ wezterm/텔레그램에서 `ㅎ ㅏ ㄴ ㄱ ㅡ ㄹ` 입력 → 로그 확인.

**기대 로그(폴백 정상 시)**: 첫 키 `START` → `OnCompositionTerminated: IMMEDIATE -> fallback` → 이후 `fallback: del=1 insert='하'` → `del=1 insert='한'` → `del=0 insert='ㄱ'` … (del/insert가 글자 누적과 일치).
**기대 화면**: wezterm/텔레그램에 잔류 없이 `한글`만 입력.

## 5. 남은 리스크 / TODO
- 폴백 모드에서 백스페이스·방향키·마우스 클릭으로 커서가 이동하면 `fallback_pending` 위치가 어긋날 수 있음 → 커서 이동 감지 시 pending 리셋 필요.
- 200ms 임계값은 휴리스틱 — 느린 시스템에서 정상 앱이 오판될 가능성. 앱별(HWND/프로세스) 캐싱 고려.
- `OnCompositionTerminated` 정상 종료 분기의 `engine.reset()`가 폴백 앱에서 포커스 이탈 시에도 pending을 안 비움 → 점검 필요.
- 릴리스 전 `UNIM_DEBUG_LOG=false` 로 되돌릴 것.
