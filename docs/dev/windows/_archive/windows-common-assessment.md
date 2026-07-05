# `unim-windows-common` 추출 타당성 검토 (honest assessment)

작성: 2026-06-17 / 브랜치 `feat/windows-msi-redesign`

## 결론: **partial** (작은 공용 크레이트 1개 + 와이어타입은 별도 처리)

"세 개가 Win32 코드를 공유한다"는 전제는 **부정확**하다. 실제로 저수준 Win32/COM glue를
의미 있게 공유하는 것은 **2개 — `unim-tsf` 와 `unim-imm32`** 뿐이다.

| crate | 역할 | 저수준 Win32/COM glue 공유? |
|---|---|---|
| unim-tsf | TSF in-proc COM cdylib | YES (registry, dll-path, dbg_log, modifier, DllMain hinst) |
| unim-imm32 | IMM32 .ime cdylib (신규) | YES (위와 동일 — tsf에서 lift됨) |
| unim-tsf-settings | Slint 설정 exe | **NO** — raw Win32 0줄. `unim::config` 코어만 사용. 강제 편입 금지 |
| unim-popup-win | 팝업 렌더러 exe | 부분적 — Win32는 D2D/DWrite/pipe 전용(tsf와 안 겹침). 단 **serde 와이어타입**은 tsf와 의도적 중복 |

## 파일 단위 중복 감사 (tsf ↔ imm32)

### 1. `get_dll_path` — clean
- tsf `register.rs:21-32`, imm32 `register.rs:30-43`
- `GetModuleFileNameW` 로 모듈 경로 추출. 차이는 HINSTANCE 소스(`dll_instance()` vs `ime_state::hinst()`)뿐 → HMODULE 인자로 받으면 동일. ~12줄 × 2.

### 2. `set_reg_value` (REG_SZ) — clean (identical)
- tsf `register.rs:68-86`, imm32 `register.rs:47-65`
- `RegSetValueExW` UTF-16 wrapper. **바이트 단위 동일**. ~18줄 × 2.

### 3. `set_reg_dword` (REG_DWORD) — clean
- tsf `register.rs:89-104`만 존재(imm32는 아직 미사용이나 IMM32 등록 확장 시 필요). ~15줄.

### 4. `dbg_log` + `process_tag` + `UNIM_DEBUG_LOG` — needs-abstraction
- tsf `register.rs:191-210`(파일만), imm32 `register.rs:240-273`(파일 + `OutputDebugStringW`)
- 차이: (a) 로그 파일명·태그 문자열, (b) imm32는 OutputDebugStringW 추가, (c) tsf는 `UNIM_DEBUG_LOG=true` 상수, imm32는 `cfg!(debug_assertions)`.
- `%TEMP%` append + PID 태그 코어는 동일. 파라미터(태그, 파일명, debug-string on/off)를 받는 작은 logger로 통합 가능. ~20~33줄 × 2.

### 5. modifier reading — needs-abstraction (핵심)
- tsf `key_handler.rs:20-38` (live `GetKeyState`), imm32 `input.rs:45-61` (`lpbKeyState` 256B 배열)
- 비트 의미(bit7=down, bit0=toggled)·VK 집합·`ModifierState` 조립이 동일. 입력 소스만 다름.
- 권장 형태: `fn modifier_state_from(get: impl Fn(usize)->i16) -> ModifierState` 코어 + 두 얇은 wrapper(GetKeyState / 배열 인덱스). ~17줄 × 2.

### 6. DllMain hinst 저장 패턴 — needs-abstraction (작음)
- tsf `lib.rs:51,57-58,80` (`Mutex<usize>`), imm32 `ime_state.rs:83,112,116-117` (`OnceLock<isize>`)
- 개념 동일(모듈핸들 저장 후 GetModuleFileNameW용). 자료구조가 달라 통합 이득 작음. ~6줄 × 2. 굳이 추출 안 해도 됨.

### 7. windows/windows-core 0.62 feature set — needs-abstraction
- tsf/imm32 둘 다 0.62. `Win32_Foundation/LibraryLoader/Registry/UI_Input_KeyboardAndMouse` 4개 겹침. 나머지(TextServices vs UI_Input_Ime)는 crate별 고유.
- common crate가 겹치는 feature를 켜고 재노출(`pub use windows;`)하면 일관성↑. 단 windows-rs는 feature additive라 효과는 "약간".

### 8. `synth_input.rs` (SendInput) — **divergent / skip**
- tsf `synth_input.rs` 전체가 `#[allow(dead_code)]`, 현재 호출부 없음. TSF sink 재진입 카운터·F24 센티널·preedit replay 로직이 TSF 특화. imm32는 자체 comp string을 쓰므로 SendInput 불필요. 공용화 가치 없음.

### 9. popup 와이어타입 (RenderState/WireCell/WireMsg/RevEvent + 상수) — needs-abstraction (별건)
- tsf `popup_ipc.rs:33-231`, popup-win `protocol.rs:1-338`
- **소스 주석에 "양 크레이트 동일 사본" 명시된 의도적 중복**(~200줄). 가장 큰 단일 중복 덩어리.
- 단 이것은 **Win32 glue가 아니라 serde IPC 프로토콜**이며 공유 짝이 tsf↔popup-win(설정·imm32와 무관). 별도 `unim-popup-wire`(serde-only, no windows dep) 크레이트가 맞다 — windows-common에 넣으면 안 됨.
- pipe 보안(SDDL→SECURITY_ATTRIBUTES)·session_id 로직은 client(tsf)/server(popup-win)가 비대칭이라 divergent.

## BENEFIT vs COST

**Benefit**: register glue(set_reg_value/dword/get_dll_path) + dbg_log + modifier reader 단일 진실원.
실제 dedup 가능 라인 ≈ **90~110줄**(register ~45, dbg_log ~25, modifier ~17, dll-path ~12). 신규 IMM32가 tsf에서 베껴온 직후라 지금이 drift 전 봉합 적기.

**Cost**: 워크스페이스 멤버 +1, cdylib 2개의 빌드 그래프에 공용 lib 1개 추가(작음), modifier/dbg_log는 추상화(closure/param) 필요 → 과추상화 위험 소. DllMain hinst·feature set·synth_input은 이득 대비 churn이 커서 제외.

## 권장 실행안

1. **`unim-windows-common`** (rlib, `cfg(windows)`, windows 0.62 의존) 신설 → `get_dll_path(HMODULE)`, `set_reg_value`, `set_reg_dword`, `dbg_log(tag, file, also_debug_string)`, `modifier_state_from(closure)` + 두 wrapper. consumer: **unim-tsf, unim-imm32**.
2. **별건**: popup 와이어타입은 `unim-popup-wire`(serde-only, windows dep 없음)로 분리. consumer: unim-tsf, unim-popup-win. (windows-common과 합치지 말 것 — 의존성/짝이 다름)
3. **제외**: unim-tsf-settings(raw Win32 없음), synth_input(dead+TSF특화), DllMain hinst storage, feature-set 재노출.

순이득: 약 90~110줄 dedup + registry/dll-path/dbg_log/modifier 단일화. 와이어타입까지 별 크레이트로 빼면 추가 ~200줄. 비용은 작은 rlib 1~2개. **partial(=핵심 glue만) 추천**, 와이어타입은 후속 별 PR.
