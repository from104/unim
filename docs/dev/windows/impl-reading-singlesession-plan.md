# GUID_PROP_READING + 단일세션 구현안 (적대 검토 통합·검증 완료)

상태: **적용 완료 + 빌드/테스트 검증 통과** (이 문서는 적용된 변경의 정본 기록)

대상: `unim-tsf/src/composition.rs`
검토자: reviewer (windows 모드)
근거 1차소스: windows-rs 0.62.2 레지스트리 바인딩, MS Learn ITfProperty::SetValue, Mozc tip_edit_session_impl.cc

---

## 1. 1차소스 검증 결과

| 항목 | 결과 | 근거 |
|---|---|---|
| `GUID_PROP_READING` 상수 노출 | OK | windows-0.62.2 `.../UI/TextServices/mod.rs:157`, GUID `5463f7c0-8e31-11d2-bf46-00105a2799b5`. `use ...TextServices::*`(L10)로 in-scope. 리터럴 정의 불필요 |
| VARIANT 레이아웃 | OK | `VARIANT.Anonymous(VARIANT_0).Anonymous: ManuallyDrop<VARIANT_0_0>` → `.vt`, `.Anonymous(VARIANT_0_0_0).bstrVal: ManuallyDrop<BSTR>`. `VT_BSTR=VARENUM(8)` |
| VARIANT Drop 미구현 | OK | `impl Drop for VARIANT` grep 0건 — 자동 해제 없음, 명시 drop 필요 |
| `ITfProperty::SetValue(ec, prange: P1, *const VARIANT)` | OK | `range:&ITfRange` 전달 가능. `[in] const VARIANT*` → 값 복사, 소유권 미이전 (MS Learn) |
| VT_BSTR 지원 | OK | MS Learn: "Only VT_I4, VT_UNKNOWN, VT_BSTR and VT_EMPTY are supported" |
| `BSTR: From<&str>` / Drop=SysFreeString | OK | windows-strings-0.5.1 bstr.rs:77, :160 |
| **SetText 시 property discard** | 중요 | MS Learn Remarks: "Property values set with this method will be discarded when the text that the property value covers is modified." → 매 update SetText 후 READING **재부여 필수** |

## 2. 적대 검토에서 발견·수정한 Design JSON 결함 (둘 다 컴파일 실패였음)

D1·D2·D3가 제시한 `set_composition_reading` 코드는 **그대로면 빌드 FAIL**이었다. 실제 `cargo check`로 2건 적발·수정:

1. **E0502 borrow 충돌**: `let inner = &mut *var.Anonymous.Anonymous` 가 살아있는 채 `prop.SetValue(ec, range, &var)`(immutable borrow) 호출 → "cannot borrow `var` as immutable because it is also borrowed as mutable". D1 risk #2가 가능성만 언급했을 뿐 exact_edits는 미수정.
   - 수정: VARIANT 구성을 `{ ... }` 블록으로 감싸 `inner` mut borrow를 SetValue 전에 종료.
2. **ManuallyDrop union DerefMut 거부**: D2가 제안한 `ManuallyDrop::drop(&mut var.Anonymous.Anonymous.Anonymous.bstrVal)` 는 "not automatically applying DerefMut on ManuallyDrop union field"로 FAIL.
   - 수정: drop도 `set_composition_attribute`와 동일하게 `let inner = &mut *var.Anonymous.Anonymous; ManuallyDrop::drop(&mut inner.Anonymous.bstrVal)` 로 명시 deref.

D3가 제안한 cancel 경로 `clear_composition_reading`는 **불필요**(redundant): cancel은 `SetText(&[])`로 covered 텍스트를 비우므로 MS Learn 규칙상 READING이 자동 discard. 추가 안 함(회귀 표면적 최소화).

## 3. 적용된 변경 (검증 완료)

모두 `unim-tsf/src/composition.rs`:

1. **import**(L9): `{VARIANT, VT_I4}` → `{VARIANT, VT_BSTR, VT_I4}`
2. **`set_composition_reading` 헬퍼 신설**(set_composition_attribute 직후): VT_BSTR VARIANT 구성 → SetValue → BSTR 명시 drop. borrow/deref 수정 반영.
3. **`update_composition`(UpdateCompositionEditSession::DoEditSession)**: ATTRIBUTE 직후 `set_composition_reading(.., &self.text)` 추가. SetText discard 대응으로 매 갱신 재부여.
4. **`ReplaceSurroundingEditSession`(AutoTypeFix replay)**: 동일하게 `set_composition_reading(.., &self.preedit_text)` 추가.
5. **단일세션 전환**: `start_composition` phase2(update_composition 호출) 제거, `StartCompositionEditSession`에 `text:String`/`attr_atom:Option<u32>` 필드 재도입. `DoEditSession`을 StartComposition(empty)→SetText→select→ATTRIBUTE+READING 단일세션으로 통합. `attr_atom()`(&mut self)는 세션 구성 전에 호출(borrow 회피). 호출자 시그니처(`start_composition`/`update_composition`) 불변 → key_handler.rs:369/371 무수정.

## 4. 검증 기록

- `cargo check -p unim-tsf --target x86_64-pc-windows-msvc`: **Finished, 0 error / 0 warning**
- `cargo clippy -p unim-tsf`: 23 warning (전부 pre-existing; baseline 24보다 1 감소, 신규 함수는 clippy 무경고)
- `cargo test -p unim`: **639 passed + 19 passed (1 pre-existing ignored), 0 failed**
- 빌드 타깃: gnu 미설치 → msvc(stable-x86_64-pc-windows-msvc, native) 사용

## 5. 회귀·안전성 판단

- **메모장 등 정상 TSF 앱**: READING은 표준 TSF 속성으로 무시/정상처리, SetValue 실패는 비치명(dbg_log만). 핵심 입력 경로(SetText/select/EndComposition)는 동작 순서 보존. 코어 hangul 테스트 658건 그대로 통과 → 입력 로직 무변경.
- **거꾸로입력/backspace**: `move_caret_to_end`, EndComposition Some/None 분기 미변경. cancel의 `SetText(&[])` 보존 → 잔류 자모 커밋 버그 회귀 없음.
- **메모리 안전**: BSTR은 SetValue(값 복사) 후 정확히 1회 SysFreeString. VARIANT는 Drop 없어 double-free 불가.

## 6. 잔여 실측 게이트 (코드 외)

자체 판정 무리, 사용자 실측 필요:
- wezterm/Telegram(CUAS-unaware)에서 즉시-terminate 실제 해소 여부 — 가설 검증.
- READING 값=NFC 음절 문자열이 CUAS GCS_RESULTREADCLAUSE 절 경계 계산과 충돌 없는지(서로게이트 없음 가정).
- 장시간 입력 시 BSTR 누수/크래시 부재 확인(고빈도 SetValue).

## 7. 롤백

3덩어리 독립: (a)READING 헬퍼+호출 (b)단일세션 (c)replay READING.
단일세션이 정상앱 회귀를 일으키면 (5)만 git revert로 2-phase 복귀하되 (a)(c) READING 부여는 update_composition 세션에 유지 가능. → READING과 단일세션을 별도 커밋 권장.
