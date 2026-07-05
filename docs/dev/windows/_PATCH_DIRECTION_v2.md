# UNIM Windows 패치 — 마스터 구현 계획 (_PATCH_DIRECTION_v2)

> ## ⛔ D1(IMM32) 트랙 정정 (SUPERSEDED · 2026-06-22)
> 이 계획의 **D1 IMM32 트랙(STRINGTABLE 표시명 / CTF\Assemblies+Substitutes 듀얼모드 등)은
> 폐기**한다. 카톡 미동작의 진짜 원인은 IMM32가 아니라 **`unim_tsf.dll` x64 단독(32-bit COM
> 등록 부재)**이었고, 해결은 i686 TSF 빌드 + 32-bit TSF 등록이다(실증 완료).
> 최종 진실: **[imm32-win11-SOLUTION.md](imm32-win11-SOLUTION.md)**.
> D2(TSF inline 불변식)·D3(PARITY)·D4(docs) 트랙의 사실관계는 유효하다.

> 기준: 브랜치 `feat/windows-msi-redesign` @ `92e0ba7`, 워킹트리 클린(실측).
> 문서가 아니라 **코드 실측**을 기준으로 한다. 기존 docs 다수는 stale/모순(D4가 정리).
> 작성일 2026-06-21. 4트랙(D1 IMM32 / D2 TSF inline / D3 PARITY / D4 docs) 종합.

---

## 0. 한 줄 현황

- **TSF 터미널 inline**: `composition.rs:157 fInterimChar=BOOL(1)`(조합경로) + `:128 BOOL(0)`(커밋경로) **이미 커밋**. non-sticky 오버레이 폴백(`text_service.rs:507-516`)과 화해 완료. → **불변식. 깨지 말 것.**
- **TSF 팝업/ATF/reload**(D3): `popup_ipc.rs`(1323줄)·`auto_typefix.rs`(464줄)·`maybe_reload_config` 전부 **완성·빌드 통과**. PARITY_PLAN의 "❌ 미구현"은 거짓. → 신규 이식 불필요, 검증만.
- **IMM32 .ime**(D1): COMPOSITIONSTRING(IMCC) 경로·ATF should_consume 배선 **완성**. 잔여 = (A) STRINGTABLE 표시명, (B) CTF\Assemblies+Substitutes 듀얼모드 단일항목.
- **docs**(D4): stale 문서 5종 SUPERSEDED 배너 + PARITY 표 정정.

---

## 1. 충돌맵 (파일 × 트랙)

| 파일 | D1 IMM32 | D2 TSF inline | D3 PARITY | D4 docs | 충돌? |
|---|---|---|---|---|---|
| `unim-windows-common/src/activation.rs` | **편집(대)** | — | — | — | 단독 |
| `unim-imm32/build.rs` | **편집** | — | — | — | 단독 |
| `unim-imm32/unim_imm32.rc` (신규) | **신규** | — | — | — | 단독 |
| `unim-imm32/src/register.rs` | **편집** | — | — | — | 단독 |
| `unim-imm32/src/globals.rs` | **편집** | — | — | — | 단독 |
| `unim-imm32/src/input.rs` | 편집(소/주석) | — | — | — | 단독 |
| `installer/wix/unim.wxs` | **편집** | — | — | — | 단독 |
| `unim-tsf/src/composition.rs` | — | **불변식 가드(무변경)** | 불변식 의존 | 참조만 | **핫스팟** |
| `unim-tsf/src/text_service.rs` | — | **불변식 가드(무변경)** | 불변식 의존 | 참조만 | **핫스팟** |
| `unim-tsf/src/key_handler.rs` | — | 불변식 가드 | **편집(1줄, unused import)** | 참조만 | 경미 |
| `unim-tsf/src/synth_input.rs` | (dead code 재사용 금지 경계) | 불변식 가드 | 참조만 | 참조만 | 격리필요 |
| `unim-tsf/src/auto_typefix.rs` | (코어 공유, 사본 아님) | — | 검증대상 | 참조만 | 공유코어 |
| `docs/dev/windows/*.md` (5종) | — | — | — | **편집** | 단독 |

### 핵심 판정
- **D1은 TSF 4파일을 전혀 건드리지 않는다.** IMM32 크레이트(`unim-imm32`, `unim-windows-common`)와 WiX만 만진다 → **완전 독립**.
- **D2는 코드 변경이 0이다**(결론=조치 불필요). 역할은 "불변식 3개를 다른 트랙이 깨지 않게 가드"하는 감시자.
- **D3의 실제 코드 변경은 `key_handler.rs:7` unused import 제거 1줄뿐.** 나머지는 런타임 검증.
- **D4는 docs만**. 코드 무관.

### TSF 4파일을 동시에 만지는 트랙은 없다
실제 코드 편집이 TSF 핫스팟(`composition.rs`/`text_service.rs`)에 들어가는 트랙은 **없음**. D3의 `key_handler.rs` 1줄 편집만 TSF 디렉터리를 건드린다. 따라서 **worktree 충돌은 사실상 없다.**

---

## 2. 불변식 (어떤 트랙이든 위반 금지 — D2 가드)

1. **`composition.rs:157 fInterimChar=BOOL(1)`(조합경로) 보존.** BOOL(0)으로 되돌리면 wezterm inline 깨짐(NavilIME/saenaru/kolemak 패턴). `:128 BOOL(0)`(move_caret_to_end/커밋경로)도 보존.
2. **`text_service.rs:507-516` 단어경계 리셋 보존.** 제거하면 `composition_unsupported` 영구 고착 회귀.
3. **`synth_input.rs:75 send_replacement`(SendInput, dead code)를 정식 TSF 경로에 절대 재연결 금지.** 정식 TSF 앱 동기 문서모델과 충돌 회귀. (.ime의 ATF는 IMCC `build_and_emit` 경로가 정석 — SendInput 불필요.)
4. **락 순서 보존**: engine→config→composition→popup_ipc→last_context.
5. **역채널 stale 가드(owner_hwnd+seq)·조합/폴백 중 reload 보류 게이트 보존.**

---

## 3. 구현 순서 (웨이브)

### Wave 1 — 독립·저위험 병렬 (worktree 분리 안전)
| 트랙 | 작업 | parallel | worktree |
|---|---|---|---|
| **D4 docs** | stale 문서 5종 SUPERSEDED 배너 + PARITY 표 AutoTypeFix 행 ✅ 정정 + MEMORY 노트 갱신 | ✅ | ✅ |
| **D3 cleanup** | `key_handler.rs:7` unused import 제거 (`cargo fix --lib -p unim-tsf`) | ✅ | ✅ |

두 트랙은 서로 다른 파일군(docs vs TSF 1줄)이라 병렬 안전. 빌드/동작 영향 0(D4) 또는 경고 제거뿐(D3).

### Wave 2 — IMM32 듀얼모드 (독립 크레이트, 코드우선)
| 트랙 | 작업 | parallel | worktree |
|---|---|---|---|
| **D1 IMM32** | (A) STRINGTABLE: `unim_imm32.rc` 신규 + `build.rs` winres/embed-resource + `register.rs`·`unim.wxs`·`globals.rs` 인디렉트 표시명 전환. (B) Assemblies+Substitutes: `activation.rs` `write_substitute_and_assembly()` 신규 | 내부 직렬(서브태스크 A→B) | ✅ |

D1은 TSF를 안 건드리므로 Wave 1과도 병렬 가능하지만, **effort=medium + blind 위험**이라 별도 웨이브로 분리해 집중. A(표시명)→B(Assemblies) 순서: A는 빌드 검증 가능(RC 임베드 성공 여부), B는 레지스트리 스키마라 무검증.

### Wave 3 — 런타임 검증 라운드 (코드 미변경)
| 트랙 | 작업 | parallel | worktree |
|---|---|---|---|
| **D3 검증** | KakaoTalk/메모장/wezterm에서 팝업 클릭·순/역방향 ATF·GUI 저장 즉시반영 실측 | (단일) | ❌(설치 필요) |
| **D1 검증** | 카톡/한컴에서 .ime 연결·언어바 단일항목·표시명·순방향 ATF 실측 | (단일) | ❌ |

Wave 3은 코드가 아니라 **설치+수동 테스트**라 worktree 부적합. 사용자가 "검증 생략·코드 우선" 결정했으므로 **별도 라운드로 분리 권고**(§5).

---

## 4. 트랙별 effort + breaksWorkingCode 위험

| 트랙 | effort | breaks? | 핵심 위험 |
|---|---|---|---|
| **D1 IMM32** | medium | false (단, blind) | (1) Substitutes 자기참조(E0200412→자기자신) = 무한대체/로드실패 → **반드시 베이스 `00000412`로만**. (2) Assemblies Profile GUID LE 16바이트 오류 시 기존 TSF 항목 충돌 → 입력기 목록 깨짐. (3) winres dual-build(x64+i686) RC 임베드 누락 시 표시명 빈 문자열. (4) `register.rs:111 {:?}`와 동일 바이트 검증 필수. |
| **D2 TSF inline** | small | false (무변경) | 코드 변경 0 → 회귀 위험 0. 단 **불변식 §2 가드 책임**. fInterimChar 보존이 최우선. |
| **D3 PARITY** | small | false | 완성된 코드를 "이식"이라며 재작성하면 검증된 코드 회귀 → **재작성 금지, unused import 1줄 + 런타임 검증만**. |
| **D4 docs** | small | false | 편집 전용, regression 0. |

### fInterimChar inline 보존 (최우선 불변식)
- 어떤 트랙도 `composition.rs:152-157`을 편집하지 않는다(충돌맵 확인).
- D1은 IMM32 크레이트만, D3는 `key_handler.rs` 1줄, D4는 docs. → fInterimChar는 **구조적으로 안전**.
- 향후 라운드에서 TSF composition을 만질 경우 이 문서 §2를 가드로 명시할 것.

---

## 5. 무검증 코드우선의 한계 (blind 구현 위험 + 가드)

사용자 결정 = "검증 생략·코드 우선". 트랙별 blind 안전도:

| 트랙 | blind 안전? | 이유 / 가드 |
|---|---|---|
| **D4 docs** | ✅ 안전 | 사실 반영뿐. |
| **D3 import 제거** | ✅ 안전 | `cargo fix`가 검증. |
| **D1 (A) STRINGTABLE** | ⚠️ 부분 | RC 임베드 성공은 **빌드로 검증 가능**. dual-build 양쪽 적용 여부만 빌드 로그로 확인. 표시명 실제 렌더는 런타임 필요. |
| **D1 (B) Assemblies/Substitutes** | ❌ **고위험 blind** | 레지스트리 스키마(Default=CLSID vs Profile binary, KeyboardLayout DWORD 유무)가 Windows 버전마다 다름. 정상 MS Korean의 `HKCU\...\CTF\Assemblies\0x00000412` 덤프 없이는 추정. 잘못 쓰면 **입력기 목록 자체가 깨짐**. |

### D1(B) blind 진행 시 가드 (코드우선 준수하되 롤백 용이하게)
1. **fail-soft 유지**: `ensure_imm32_active`의 `:58` 기존 정책대로 실패는 로깅만, panic 금지.
2. **idempotent + 기존키 값비교 후 갱신**: 기존 Assemblies 키 존재 시 값 비교, 다를 때만 write.
3. **HKCU만 기록**(HKLM 안 건드림) → 사용자 단위 롤백 용이. `reg delete HKCU\...\CTF\Assemblies\0x00000412` 로 즉시 원복.
4. **언인스톨 대칭**: `remove_substitute_and_assembly()` 작성 + MSI `ForceDeleteOnUninstall` 위임.
5. **Substitutes는 베이스 `00000412`로만**(자기참조 금지) — 무한대체 방지.
6. **검증 보류 = openQuestion으로 명시**(아래) → 사용자가 실패 보고 시 1차로 볼 곳.

### 롤백 용이성
- D1 전체 = IMM32 크레이트 + WiX + HKCU 레지스트리. **TSF 무관**이라 D1만 revert해도 TSF inline/팝업/ATF 무영향.
- HKCU 레지스트리는 `reg delete`로 즉시 원복 → 입력기 목록 깨짐 시 복구 빠름.

---

## 6. 별도 라운드 권고 (deferToOwnRound)

1. **Wave 3 런타임 검증 라운드** — 설치+수동 테스트(카톡/한컴/wezterm/메모장). 코드 아님, worktree 부적합. 코드우선 결정이라 1차 구현에서 분리하되 **반드시 후속 라운드 확보**.
2. **D1(B) Assemblies 스키마 실측** — 정상 동작 MS Korean의 실 레지스트리 덤프로 스키마 검증. blind 실패 시 최우선.
3. **PARITY 추가공사**(있다면) — 현행 팝업/ATF/reload는 완성이므로 신규 이식 없음. 단 `ImeConfigure`(register.rs:153 FALSE) 설정앱 연결·UILess `ITfCandidateListUIElement`는 미구현 → 원하면 독립 라운드.
4. **공용 ATF 오케스트레이션 크레이트 추출** — TSF `auto_typefix.rs` ↔ .ime ATF 래퍼 사본화 여지(코어는 이미 공유). 별건, 비긴급.

---

## 7. 미해결 오픈 (추적)

- **O1 `ITfContextOwnerCompositionSink`**: RETROSPECTIVE가 "오진"으로 기각, fInterimChar로 화해. 런타임 무검증 → 추진 금지, "특정 앱 즉시종료 잔존" 실측 보고 시에만 조건부 검토.
- **O5 fallback_pending 위치 어긋남 / 200ms 매직넘버**: 느린 머신/RDP 오학습 이론상 가능, 실측 보고 없음 → 추적만.
- **O8 winres vs embed-resource dual-target / `.def` /DEF 충돌**: 빌드 검증으로 1차 확인.
- **O9 Substitutes+KLF_SUBSTITUTE_OK 로드 간섭**: Substitute가 '표시통합'용인지, IMM32 로드를 막는지 실측 필요(activation.rs:104).
- **O 표시명 인디렉트 부호(-1 vs -1000)**: RC id=1 → 인디렉트 -1. SHLoadIndirectString 음수절댓값=리소스id 규약 확인.

---

## 부록: 트랙 범위 한 줄 요약

- **D1** = IMM32 듀얼모드(Assemblies 단일항목 + STRINGTABLE 표시명) + 순방향 ATF 재배선(잔여 소).
- **D2** = 조치 불필요. fInterimChar 등 불변식 3개 가드 감시자.
- **D3** = 완성 코드 검증 트랙으로 재정의. 코드는 unused import 1줄.
- **D4** = stale 문서 정합화(SUPERSEDED 배너 + 표 정정) + 본 문서.
