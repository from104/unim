# wezterm/텔레그램 CUAS composition 복구 — 단계별 구현 계획

> 상태: **계획만**. 코드 미수정. feat/windows-msi-redesign 기준.
> 대상 크레이트: `unim-tsf` (Windows 전용 cdylib). 코어(`src/`) 무변경 목표.
> 모든 라인 번호는 작성 시점(2026-06-01) 소스 기준 — 편집 직전 재확인 필수.

---

## 문제 요약 (CUAS 계약 위반)

wezterm/텔레그램 등 IMM32 소비 앱은 Windows **CUAS(Cicero Unaware Application Support)** 가 우리 TSF composition을 `WM_IME_COMPOSITION`으로 변환해 받는다. MS 기본 한국어 IME(순수 TSF)는 동일 wezterm에서 정상 동작하므로 **환경 한계가 아니라 unim-tsf가 CUAS 계약을 어겨서** composition이 매 자모마다 즉시 종료된다. 우리는 IMM32를 에뮬레이트할 필요가 없다 — CUAS가 기대하는 TSF 계약(비동기 edit session, display attribute property, composition 수명 보존)을 지키면 된다.

## 근본 원인 (실측 로그)

`docs/dev/windows/windows-console-composition-bug.md` · `docs/dev/windows/wezterm-composition-research-3rd.md` 의 `%TEMP%\unim-tsf.log` 실측:

```
handle_key_down: preedit_changed=true was_composing=false comp_active=false
acquire_insert_range: InsertAtSelection QUERYONLY ok
StartComp.DoEditSession: StartComposition ok        ← hr=Ok(0) 성공
start_composition: composition CREATED              ← 거부 아님(NULL 아님)
OnCompositionTerminated: ...                         ← StartComposition 직후 즉시!
(매 자모 반복, preedit_len 항상 1)
```

모든 TSF 호출(`InsertTextAtSelection`/`SetText`/`StartComposition`/`RequestEditSession`)이 `hr=Ok(0)`로 성공하고 composition 객체도 CREATED 된다. 그럼에도 CUAS owner가 StartComposition 직후 `OnCompositionTerminated`를 호출 → 현재 핸들러(`text_service.rs:388-389`)가 이를 받아 `engine.reset()`을 호출해 **스스로 preedit 버퍼를 파괴**한다(자폭). 메모장(정식 TSF text store)은 composition을 직접 렌더링하므로 attribute/owner-sink 없이도 정상, wezterm(CUAS 더미 스토어)은 계약 위반 시 즉시 terminate — 이것이 메모장 OK / wezterm NG의 분기점.

---

## 구현 단계

각 Phase는 독립 검증 가능하도록 배치. **Phase 0/1(sticky 해제 + reset 재배치)은 상호 의존이라 한 묶음·한 커밋으로 단독 검증 → 부족 시 Phase 3(VARIANT 명세 확정 후) → 그래도 안 되면 Phase 2(구조 변경 동반, 별도 PR).** 권장 구현 순서는 문서 하단 "구현 순서 확정" 참조.

### Phase 0/1 — sticky 플래그 해제 + 정상 종료 reset 재배치 (한 묶음, 최우선)

Phase 0(sticky 해제)과 Phase 1(OnCompositionTerminated reset 제거)은 **상호 의존**이라 분리 불가다. Phase 1이 `OnCompositionTerminated` else 분기의 `engine.reset()`를 없애면 "정상 종료(포커스 이탈) 시 엔진 preedit 정리"가 사라진다. 이 정리 책임을 Phase 0이 손대는 **ThreadMgrEventSink::OnSetFocus(L408)** 로 옮긴다. 따라서 두 변경은 **한 커밋**이며 같은 함수(L408)를 공유한다.

#### ⚠️ OnSetFocus 위치 확정 — L223과 혼동 절대 금지

`text_service.rs`에 `fn OnSetFocus`가 **두 개** 존재한다:

| 위치 | 트레이트 | 시그니처 | 용도 | 본 작업 대상? |
|---|---|---|---|---|
| **L223** | `ITfKeyEventSink::OnSetFocus` | `(&self, _fforeground: BOOL)` | config.yaml mtime 비교 후 조용히 reload | **❌ 손대지 말 것** |
| **L408** | `ITfThreadMgrEventSink::OnSetFocus` | `(&self, _pdimfocus: Ref<ITfDocumentMgr>, _pdimprevfocus: Ref<ITfDocumentMgr>)` | 현재 `Ok(())` 빈 껍데기. AdviseSink는 L119-120에서 이미 완료 | **✅ 여기에 추가** |

L223은 시그니처가 `BOOL` 한 개라서 즉시 구별된다. 잘못 L223에 넣으면 config-reload 경로에 sticky/reset이 섞여 reload 동작이 오염된다.

#### 변경 1 — L408 ThreadMgrEventSink::OnSetFocus 본문 (Phase 0 + Phase 1 정리 책임)

- **대상**: `text_service.rs:408-414` (현재 `Ok(())`).
- **변경 후 스케치**:
  ```rust
  fn OnSetFocus(
      &self,
      _pdimfocus: Ref<'_, ITfDocumentMgr>,
      _pdimprevfocus: Ref<'_, ITfDocumentMgr>,
  ) -> Result<()> {
      // (Phase 0) 포커스가 새 문서 컨텍스트로 이동 → 폴백 상태 낙관적 리셋.
      //   wezterm이면 다음 키에서 다시 즉시-terminate가 나서 자동 재감지된다.
      self.composition_unsupported.store(false, Ordering::SeqCst);
      self.fallback_pending.store(0, Ordering::SeqCst);
      // (Phase 1 보완) OnCompositionTerminated에서 옮겨온 "정상 종료" 정리.
      //   포커스 이탈 = 사용자가 조합을 떠남 → 엔진 preedit 비움.
      self.engine.lock().unwrap().reset();
      if let Some(ref mut win) = *self.popup_window.lock().unwrap() {
          win.hide();
      }
      self.atf_state.lock().unwrap().reset_on_focus();
      Ok(())
  }
  ```
- **근거**: research-3rd "남은 리스크/TODO"(폴백 pending 안 비움) + 사용자 지시(P1). reset/popup/atf 정리 블록은 기존 OnCompositionTerminated else 분기에서 그대로 이동.

#### 변경 2 — L356-396 OnCompositionTerminated에서 engine.reset() 제거 (Phase 1 본체)

**문제**: text_service.rs:388-389 `else` 분기가 `self.engine.lock().unwrap().reset()` 호출. CUAS의 정당한 단발 종료를 "사용자 조합 취소"로 오인해 한글 엔진 preedit 버퍼까지 날린다 → preedit_len이 영원히 1(자폭).

- **대상**: `text_service.rs:356-396` `OnCompositionTerminated`.
- **변경 전**: immediate(<200ms) → 폴백 set. else → `engine.reset()` + popup hide + atf reset.
- **변경 후 스케치**:
  ```rust
  // composition 객체 참조만 비운다. 엔진 preedit 버퍼는 보존.
  self.composition_mgr.lock().unwrap().clear();
  // engine.reset() 호출하지 않음 — 다음 키에서 누적 preedit를 재조합한다.
  // 정상 종료(포커스 이탈)는 ThreadMgrEventSink::OnSetFocus(L408, 변경 1)가 정리.
  // immediate 휴리스틱(200ms)은 폴백 진입 안전망으로 유지(아래 Phase 4 참조).
  if immediate {
      self.composition_unsupported.store(true, Ordering::SeqCst);
      let pending = self.engine.lock().unwrap().preedit_str().chars().count();
      self.fallback_pending.store(pending, Ordering::SeqCst);
  }
  // else 분기의 engine.reset()/popup hide/atf reset 블록 전체 삭제 → OnSetFocus로 이동.
  ```
  `key_handler.rs:359` `start_composition` 분기는 `was_composing==false && comp_active==false`여도 `result.preedit_changed`면 새 composition을 만든다 → 엔진 preedit가 보존되면 다음 키에서 누적 텍스트로 자연 재진입.
- **근거**: research-3rd 최종 판정 "A안 최우선 1줄급 수정". 단계별 수정 이력 #1.

#### 검토 항목 — L223 ↔ L408 이중 reset 상호작용

L223(KeyEventSink::OnSetFocus, config-reload)과 L408(ThreadMgrEventSink::OnSetFocus, 신규 reset)은 **포커스 전환 시 둘 다 발사될 수 있다**. L223의 reload 경로가 엔진을 재생성/갱신하고 L408이 직후 `engine.reset()`을 호출하면 순서에 따라 효과가 달라질 수 있다(둘 다 "입력 비활성 시점"이라 race는 낮으나 호출 순서는 TSF가 결정). **구현 시 확인**: L223 reload가 실제로 엔진 상태를 바꾸는지, 두 콜백의 발사 순서(보통 ThreadMgr sink가 KeyEventSink보다 먼저/나중인지)를 로그로 확인하고, 이중 reset이 무해(idempotent)한지 검증. 두 reset 모두 빈 버퍼를 비우는 것이라 기능상 무해할 가능성이 높으나 **명시적 검증 필요**.

> **참고(불필요 항목 삭제)**: 초안의 "Escape 키 핸들러에서 reset 확인" 항목은 **삭제**한다. `key_handler.rs`에 `engine.reset()` 호출은 **0건**이며, Escape는 엔진 내부 `process`가 preedit를 비워 `result.preedit_changed && preedit.is_empty()` → `key_handler.rs:354 end_composition`으로 정리되므로 별도 reset 경로가 없다.

### Phase 2 — edit session 플래그 TF_ES_SYNC → TF_ES_ASYNCDONTCARE (구조 변경 동반, 별도 PR)

> **위험도 상. "6곳 플래그 일괄 치환"은 오처방이다.** Phase 0/1·3 완료·VM 검증 후에도 미해결일 때만, **별도 작업·별도 PR**로 진행할 것.

**문제(가설)**: composition 관련 RequestEditSession이 모두 `TF_ES_READWRITE | TF_ES_SYNC`. CUAS 더미 컨텍스트는 SYNC RW grant를 약식 처리해 종료 타이밍을 앞당길 수 있다. SampleIME/weasel은 비동기 사용.

**핵심 — 6곳은 동질이 아니다.** `start_composition`(composition.rs:152)과 `replace_surrounding`(composition.rs:251)은 RequestEditSession **직후 같은 함수 안에서** `composition_slot.lock().take()`로 세션 결과(생성된 ITfComposition)를 꺼내 `self.composition`에 보관한다. 이는 **SYNC inline 실행(= RequestEditSession 반환 시점에 DoEditSession이 이미 끝남)** 을 전제한 코드다. ASYNCDONTCARE면 DoEditSession이 deferred → `take`가 **빈 슬롯** → `self.composition = None` → **composition 추적 영구 상실**.

| composition.rs 지점 | 함수 | 세션 후 슬롯 take? | ASYNC 안전성 |
|---|---|---|---|
| L171 | `update_composition` | 없음 (clone된 composition 사용) | ✅ 안전 |
| L186 | `end_composition_with_text` | 없음 | ✅ 안전 |
| L202 | `end_composition` | 없음 | ✅ 안전 |
| L215 | `insert_text` | 없음 (composition 무관) | ✅ 안전 |
| **L152** | **`start_composition`** | **L155 슬롯 take → self.composition** | **❌ 구조 변경 필요** |
| **L251** | **`replace_surrounding`** | **L249-261 슬롯 take → self.composition** | **❌ 구조 변경 필요** |
| L566 | `read_selection_text` | (TF_ES_READ, 읽기 전용) | 변경 제외 |

- **안전한 4곳(L171·L186·L202·L215)**: 단순 플래그 치환 `TF_ES_SYNC` → `TF_ES_ASYNCDONTCARE`.
- **start/replace 2곳(L152·L251)**: 플래그 치환만으론 깨진다. **DoEditSession 콜백 안에서 `self.composition`(공유 슬롯)을 직접 갱신**하도록 구조 변경 동반. 즉 "함수가 RequestEditSession 후 슬롯에서 take" 패턴을 버리고, "DoEditSession이 생성된 composition을 슬롯에 넣고, 다음 사용 시점에서 슬롯을 읽는" 비동기-안전 패턴으로 재작성. 이미 `composition_slot: Arc<Mutex<Option<...>>>`이 있으므로 슬롯을 단일 진실원천(source of truth)으로 승격하면 됨(함수 반환 직후 take 제거).
- **변경 후 스케치(안전 4곳)**: `TF_ES_READWRITE | TF_ES_SYNC` → `TF_ES_READWRITE | TF_ES_ASYNCDONTCARE`
- **근거**: research-3rd Q2 2순위, 최종 판정 검증사다리(2), mozc #821(SYNC→ASYNC).
- **리스크 (상)**: (1) start/replace 구조 변경이 누락되면 composition 추적 상실. (2) ASYNC deferred 실행과 다음 키 입력 사이 race 가능성(단일 메시지 펌프 스레드라 실제로는 낮으나 deferred 세션이 다음 OnKeyDown 전에 완료된다는 보장을 확인 필요 → 미해결 항목 추가). **롤백**: 6곳 모두 `TF_ES_SYNC` 복원 + start/replace 구조 변경 되돌리기(별도 PR 단위라 revert 용이).

### Phase 3 — composition range에 GUID_PROP_ATTRIBUTE SetValue (보강, 밑줄+미확정 신호)

**문제**: 현재 `display_attr.rs`는 `ITfDisplayAttributeProvider`/`ITfDisplayAttributeInfo`만 구현(InputDisplayAttribute, GUID=`UNIM_DISPLAY_ATTR_INPUT` globals.rs:13). composition range에 attribute property를 `SetValue`하는 코드는 **0건**. CUAS는 attribute property를 읽어 WM_IME_COMPOSITION의 미확정 attribute 바이트로 변환 — 없으면 CUAS가 "완료된 result string"으로 오인해 즉시 commit+terminate 가능.

- **windows-rs 0.62.2 feature**: **추가 불필요 확인 완료(bindings 직접 검증).** `GUID_PROP_ATTRIBUTE`/`ITfCategoryMgr`/`ITfProperty`/`RegisterGUID`/`GetProperty`는 전부 `Win32_UI_TextServices`(이미 활성)에 존재. `VARIANT`/`VARIANT_0`/`VARIANT_0_0`/`VARIANT_0_0_0`/`VT_I4`/`VARENUM`은 `Win32_System_Variant` 경로지만 **VARIANT 정의 자체가 `#[cfg(all(feature = "Win32_System_Com", feature = "Win32_System_Ole"))]`** 게이트다 — 두 feature 모두 Cargo.toml L25-26에 **이미 활성**. **Cargo.toml 변경 0.**
- **대상**: `composition.rs` — composition 생성/갱신 시 SetText 직후. 적용 위치:
  - `StartCompositionEditSession::DoEditSession` (L273-, SetText는 L278, select_composition_range는 L311) → SetText 후 attribute set.
  - `UpdateCompositionEditSession::DoEditSession` (L328-, SetText는 L332) → 매 update set.
  - `ReplaceSurroundingEditSession::DoEditSession` 의 preedit 분기(L431-435, StartComposition 경로) → set.
- **TfGuidAtom 캐시**: `CompositionManager`에 `attr_atom: Option<u32>` 추가, 최초 1회 `ITfCategoryMgr::RegisterGUID(&UNIM_DISPLAY_ATTR_INPUT)`로 획득해 재사용. `ITfCategoryMgr`는 `thread_mgr.cast::<ITfCategoryMgr>()`로 획득(register.rs에 cast 선례 있음). atom을 EditSession 구조체에 전달.
- **import 추가**: composition.rs 상단에 `use windows::Win32::System::Variant::{VARIANT, VT_I4};` (현재 composition.rs는 `Win32::UI::TextServices::*`만 import → Variant 경로 명시 필요).
- **변경 후 스케치 — VARIANT 필드 경로 확정(windows 0.62.2 bindings 직접 확인, 구현자 위임 아님)**:
  windows 0.62.2 레이아웃: `VARIANT.Anonymous`(=`VARIANT_0` union) → `.Anonymous`(=`ManuallyDrop<VARIANT_0_0>`) → 여기에 `.vt: VARENUM`와 `.Anonymous`(=`VARIANT_0_0_0` union, `.lVal: i32` 포함). `ManuallyDrop`은 `DerefMut`이라 점 접근이 그대로 통한다.
  ```rust
  // SetText 직후, DoEditSession 내부(ec 보유). atom: u32 = 캐시된 attr atom.
  // CUAS가 "미확정(밑줄) 조합"으로 인식하게 하는 신호.
  if let Ok(prop) = context.GetProperty(&GUID_PROP_ATTRIBUTE) {
      // VARIANT는 0.62에 From<i32> 없음 → VT_I4를 union 필드로 직접 구성.
      let mut var = VARIANT::default(); // zeroed (vt=VT_EMPTY)
      unsafe {
          var.Anonymous.Anonymous.vt = VT_I4;                       // VARENUM(3)
          var.Anonymous.Anonymous.Anonymous.lVal = atom as i32;     // TfGuidAtom
      }
      // SetValue(ec, range, *const VARIANT) — range 인자는 Param<ITfRange>로 &range 가능.
      let _ = prop.SetValue(ec, &range, &var); // 실패해도 조합은 계속(거부 무시)
  }
  ```
  > 필드 경로는 `windows-0.62.2/src/Windows/Win32/System/Variant/mod.rs`에서 검증: `VARIANT`(L743) → `Anonymous: VARIANT_0`, `VARIANT_0`(L754) → `Anonymous: ManuallyDrop<VARIANT_0_0>`, `VARIANT_0_0`(L772) → `vt: VARENUM` + `Anonymous: VARIANT_0_0_0`, `VARIANT_0_0_0`(L793) → `lVal: i32`. `VT_I4 = VARENUM(3)`(L930). union 필드 접근이라 `unsafe` 블록 필수.
- **근거**: research-3rd Q2 3순위, 검증사다리(3), Q4 SampleIME `_SetCompositionDisplayAttributes`/`ITfProperty::SetValue`. 최종 판정은 attribute를 "수명 무관(강등)"으로 봤으나, Q2는 메모장 OK/wezterm NG 차이의 "유력한 차이 설명"으로 봄 → **확실치 않으므로 Phase 1·2 후에도 미해결 시 시도**.
- **리스크 (중)**: SetValue가 잘못된 VARIANT로 실패하면 조합 자체가 깨질 수 있으므로 **반드시 실패 무시(`let _ =`)** 로 감싼다. attribute set이 정상 앱(메모장) 밑줄 렌더링에 영향 → 시각 회귀 확인 필요.

### Phase 4 — caret TF_AE_END → TF_AE_NONE / CUAS 감지 방식 (조건부, 이미 부분 반영)

- **현 상태**: `select_composition_range`(composition.rs:41-54)는 **이미 `TF_AE_NONE`** 로 composition 경로(start/update)에 적용됨. `move_caret_to_end`(L16-30)는 `TF_AE_END`로 **비조합 commit / replace_surrounding 확정 경로**에만 사용 → composition 수명과 무관하므로 **변경 불요**. research-3rd 기각 이력 #1(caret 변경해도 증상 동일)과 일치.
- **CUAS 런타임 감지**: 현재 "키 후 200ms 내 terminate" 휴리스틱(text_service.rs:367-373)은 Phase 0/1 적용 후에도 **폴백 모드 진입 트리거로 유지**(Phase 0/1이 실패했을 때의 안전망). 개선 옵션(미해결, 아래 참조): HWND/스레드 단위 캐시 또는 weasel식 GetTextExt caret rect 높이==0 판정. **이번 범위에서는 200ms 유지**, 캐시는 별도 작업으로 분리.

---

## 5지점 동기화 / Linux 회귀 영향

- **Config 5지점 동기화(엔진 src/config.rs · GUI unim-gui-gtk · CLI unim-cli) — 영향 없음.** 본 변경은 **설정 항목을 추가하지 않는다.** 모든 동작은 런타임 CUAS 감지로 자동 분기하므로 사용자 노출 설정이 불필요. (만약 향후 "폴백 모드 강제 토글"을 설정으로 빼면 그때 5지점 동기화 발동 — 현재 계획에서는 제외.)
- **Linux 프런트엔드(GTK/Qt/XIM) 회귀 — 없음(코어 무변경 전제).** 모든 변경이 `unim-tsf/` 내부(Windows cdylib). `InputEngine`/`Config` 등 코어 API **호출 방식만** 사용하고 코어 시그니처/구현은 건드리지 않는다. Phase 1에서 `engine.reset()` **호출 위치를 OnCompositionTerminated → OnSetFocus(L408)로 옮길 뿐** `reset()` 자체는 변경 없음 → 코어 `src/` diff 0 → Linux 회귀 0.
- **검증**: `cargo build -p unim` + Linux 프런트엔드 빌드가 unim-tsf 변경과 무관하게 통과해야 함(`make check`).

---

## 리스크 & 롤백 경로

| Phase | 변경 | 위험도 | 롤백 |
|---|---|---|---|
| 0/1 | sticky 해제 + reset를 OnSetFocus(L408)로 이동 + OnCompositionTerminated reset 제거 | 중 | L408 본문 `Ok(())`로 복원 + OnCompositionTerminated else 분기에 reset 블록 복원 (한 커밋 revert) |
| 2 | SYNC→ASYNCDONTCARE (안전 4곳 치환 + start/replace 2곳 구조 변경) | **상** | 6곳 `TF_ES_SYNC` 복원 + start/replace 슬롯-take 패턴 복원 (별도 PR revert) |
| 3 | GUID_PROP_ATTRIBUTE SetValue (3 EditSession) | 중 | SetValue 블록 제거 + attr_atom 필드 제거 |
| 4 | (변경 없음/200ms 감지 유지) | — | — |

**핵심 리스크 3가지:**
1. **Phase 2 ASYNC deferred 실행** — `start_composition`(L152)/`replace_surrounding`(L251)의 세션 후 `composition_slot.take()`가 SYNC inline 완료를 전제. ASYNC면 빈 슬롯 take → `self.composition=None` → composition 추적 영구 상실. **완화**: 안전 4곳과 분리, start/replace는 DoEditSession이 슬롯을 갱신하는 구조 변경 동반, **별도 PR**로 Phase 0/1·3 검증 후 마지막에 시도, 회귀 시 SYNC 롤백.
2. **Phase 0/1 정상 종료 잔류 / 이중 reset** — reset를 OnCompositionTerminated에서 OnSetFocus(L408)로 옮기므로 포커스 이탈 정리는 보존되나, (a) L223(KeyEventSink config-reload)과 L408(신규) 두 OnSetFocus가 같은 포커스 전환에 모두 발사 → **이중 reset 상호작용**(idempotent 검증 필요), (b) **메모장에서 조합 중 다른 창 클릭(포커스 이탈) 시 L408 reset가 미확정 글자를 유실**시킬 수 있음(아래 VM 체크). **완화**: 이중 reset 무해성 + 포커스 이탈 시점이 commit 후인지 로그 확인.
3. **Phase 3 VARIANT 오구성** — 잘못된 VT_I4 VARIANT가 SetValue로 들어가면 정상 앱 조합까지 깨질 수 있음. **완화**: 필드 경로는 본문에 0.62.2 bindings로 확정함, `let _ =`로 실패 무시, 메모장 회귀 우선 확인.

---

## 구현 순서 확정

1. **Phase 0/1 (한 커밋)** — L408 ThreadMgrEventSink::OnSetFocus에 sticky 해제 + reset/popup/atf 정리 추가, L389 OnCompositionTerminated else 분기의 reset 블록 제거. 상호 의존이라 분리 불가. → 빌드(`make check-windows`) → **VM 단독 검증**(메모장 회귀 + wezterm/텔레그램 조합).
2. **부족 시 Phase 3** — VARIANT 필드 경로(본문 확정)로 GUID_PROP_ATTRIBUTE SetValue 추가. → VM 검증(wezterm 밑줄 + 즉시-terminate 해소 여부).
3. **그래도 안 되면 Phase 2 (별도 PR)** — 안전 4곳 플래그 치환 + start/replace 2곳 구조 변경(난도 상). 단독으로 회귀 위험이 가장 크므로 별도 PR·별도 검증.

> 한 번에 여러 Phase를 켜지 말 것 — 어느 변경이 결정타인지 VM 로그로 분리 식별 불가해진다.

---

## Windows VM 런타임 검증 체크리스트

> Linux 크로스컴파일은 sanity(`make check-windows`)만. 실거동은 MSVC MSI를 VM에 설치해 확인.

- [ ] **메모장 (정상 앱 회귀 없음)**: `한글날` 조합/확정 정상, 밑줄 표시 정상, 거꾸로 쌓임 없음.
- [ ] **wezterm**: `안녕하세요` 자모 누적 조합되며 **밑줄 표시**, 매 자모 초기화 없음(preedit_len 증가 로그 확인).
- [ ] **텔레그램 데스크톱**: wezterm과 동일 조합 동작.
- [ ] **폴백 sticky 해제**: wezterm에서 입력(폴백 진입) → 메모장으로 alt-tab → 메모장에서 **즉시 정상 composition 경로** 사용(폴백 잔류 아님). 로그에 `composition_unsupported`가 false로 돌아옴 확인(L408 OnSetFocus 발사 확인).
- [ ] **L408 OnSetFocus reset 회귀**: 메모장에서 **조합 중(미확정 글자 떠있는 상태) 다른 창 클릭(포커스 이탈)** → 떠있던 미확정 글자가 commit되지 않고 **유실되지 않는지**. (L408 `engine.reset()`가 미확정을 날리면 회귀. 정상 앱은 포커스 이탈 시 OS가 먼저 composition을 commit/terminate하는지 순서 확인.)
- [ ] **L223↔L408 이중 reset**: config.yaml을 바꾼 직후 포커스 전환 시 reload(L223)와 reset(L408)가 충돌 없이 동작하는지.
- [ ] **폴백 모드 자체(Phase 0/1·3·2 다 실패 시 안전망)**: 폴백 경로에서 backspace/방향키 후 pending 어긋남 없는지.
- [ ] **Linux 회귀**: GTK/Qt/XIM 프런트엔드 한글 조합·AutoTypeFix·한자/특수문자 팝업 정상(코어 무변경 확인용 스모크).
- [ ] **로그 위생**: 검증 후 `UNIM_DEBUG_LOG=false` 복원.

---

## 미해결 / 불확실 항목

1. **런타임 미검증** — 본 계획의 모든 Phase는 Linux 크로스컴파일 환경에서 빌드 sanity만 가능. wezterm/텔레그램 실거동은 Windows VM 필수. 어느 Phase가 결정타인지는 VM에서만 확정.
2. **Phase 0/1 vs 3 vs 2 우선순위 모순** — research-3rd 최종 판정은 Phase 1(reset 제거)을 "1줄급 결정타·고확률"로, Phase 3(attribute)을 "수명 무관·강등"으로 봄. 그러나 Q2 진단은 attribute를 "메모장/wezterm 차이의 유력 설명"으로 봄. **확정 순서: Phase 0/1 한 묶음 단독 검증 → 부족하면 3 → 그래도면 2(별도 PR).** 한 번에 다 켜지 말 것(어느 변경이 효과인지 분리 불가해짐). 하단 "구현 순서 확정" 참조.
3. **CUAS 감지 방식 선택** — 현재 200ms 휴리스틱 유지. weasel식 `GetTextExt` caret rect 높이==0 판정 또는 HWND/스레드 캐시는 더 견고하나 별도 작업으로 분리(느린 시스템 오판·앱별 캐싱 미구현).
4. **Phase 3 TfGuidAtom 수명** — `RegisterGUID` atom을 `CompositionManager` 생애 동안 캐시 가정. 스레드/프로파일 재활성 시 atom 재획득 필요 여부 미검증.
5. **Phase 2 ASYNC deferred 세션과 다음 키 입력 race** — ASYNCDONTCARE로 deferred된 DoEditSession이 **다음 OnKeyDown 전에 완료된다는 보장**을 미확인. TSF는 단일 메시지 펌프 스레드라 실제 동시성은 낮으나, deferred 세션이 메시지 큐에 쌓여 다음 키 처리 시점에 슬롯/composition 상태가 기대와 다를 수 있음 → start/replace 구조 변경 시 슬롯 read 타이밍을 deferred-safe하게 설계 필요. VM 로그로 세션 완료/키 입력 순서 확인.
6. **L223 ↔ L408 OnSetFocus 발사 순서** — 같은 포커스 전환에서 두 sink의 콜백 호출 순서가 TSF 구현에 의존. 이중 reset이 무해(idempotent)한지, reload(L223)가 reset(L408)보다 먼저/나중인지 VM 로그로 확정 필요(Phase 0/1 검토 항목과 연동).
7. **ITfContextOwnerCompositionSink 미구현** — research-3rd Q2 1순위 용의자였으나 최종 판정에서 강등. 이 계획에는 **미포함**. Phase 0/1·3·2 전부 실패 시 차기 후보(난도 높음 — sink advise 구조 추가 필요).

## 기각된 대안 (왜 안 하는지)

- **별도 IMM32 `.ime` DLL**: mozc가 이미 폐기한 경로. 유지보수 부담 + CUAS가 이미 브리징하므로 불필요.
- **IMM32 API(`ImmSetCompositionString`) 직접 주입**: TIP 설계 외 + CUAS 자체 composition과 이중 충돌. research-3rd Q6/최종판정에서 "최후수단" 판정. 본 계획은 A안(CUAS 계약 복구)만 다룬다.
