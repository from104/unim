# CUAS-unaware 앱(터미널·레거시) 대응: client-side preedit 전환 설계

## 배경 — 기존 폴백이 깨진 이유 (실측 확정)
wezterm 실측 로그(`%TEMP%\unim-tsf.log`, 0.3.1 DLL)에서 `기현` 입력이 `ㄱ기깋기혀현현`으로 누적됨.

근본 원인 = **폴백의 삭제 메커니즘이 터미널에서 무력**:
- 폴백(`composition_unsupported` 분기)은 매 키마다 `replace_surrounding(del=N, commit+preedit)` 호출.
- `ReplaceSurroundingEditSession`(composition.rs:501)은 `ITfRange::ShiftStart(-N)`로 커서 앞 범위를 늘려 `SetText(&[])`로 삭제.
- wezterm은 **이미 확정·출력된 텍스트로 범위를 뒤로 확장하는 것을 거부** → `shifted=0`(513행에서 `let _ =`로 무시) → 삭제 0건.
- 삽입(`SetText`)은 정상 → **삽입만 누적**.
- A(display attribute) 가설은 완전 기각(`SetValue/GetValue` 실패 0건). B(host 즉시 종료)는 확정(`OnCompositionTerminated: IMMEDIATE … by_time=true`).

## 새 접근 (사용자 지시) — 리눅스 IME식 client-side preedit 강제
터미널·레거시 앱은 on-the-spot preedit를 지원하지 않으므로, fcitx/ibus가 쓰는 방식대로
**조합 중 글자를 UNIM 자체 오버레이 창에 표시하고, 확정된 글자만 앱에 삽입**한다.

### 핵심 통찰
엔진의 **commit 텍스트는 append-only**(절대 되돌릴 필요 없음). 변하는 건 preedit(조합 중 음절)뿐.
→ 앱에는 `insert_text`(SetText, wezterm 정상 동작)로 commit만 추가. **삭제 영원히 불필요.**
→ preedit는 우리 창에만. 앱 버퍼 미접촉.

### 동작 (composition_unsupported 분기 재작성)
```
commit  = commit_changed ? take_commit() : ""
preedit = engine.preedit_str()

if !commit.is_empty()  { comp_mgr.insert_text(ctx, tid, &commit); }   // 앱에 영구 추가, 삭제 없음
if preedit.is_empty()  { preedit_win.hide(); }
else                   { preedit_win.show(&preedit, caret_pos(ctx,tid)); }
```
검증: 트레이스 `기현 ` → 앱엔 `기`+`현 `만 삽입(정확), 조합 음절은 오버레이.

## 구현 항목
1. **새 파일 `preedit_window.rs`** — `PopupWindow`(popup_window.rs) 모델 복제한 경량 오버레이.
   - `WS_EX_NOACTIVATE | WS_EX_TOPMOST`(포커스 안 뺏음 → composition terminate 회피).
   - 1줄 preedit 렌더: 텍스트 + 밑줄(미확정 표시), Malgun Gothic, 다크/라이트 추종.
   - API: `create() / show(&str, Option<(i32,i32)>) / hide() / is_active()`.
   - hanja 팝업과 **별도 창**(동시 표시 가능 — preedit "한" + 후보창).
2. **위치**: `get_composition_screen_pos`(key_handler.rs:170, selection 기반)를 폴백에서도 호출.
   실패 시 마지막 위치 또는 GetCaretPos fallback.
3. **`text_service.rs`**: `PreeditWindow` 보유, `handle_key_down`에 전달. focus 상실/앱 전환/`composition_unsupported` 해제 시 `hide`.
4. **`key_handler.rs`**: `composition_unsupported` 분기를 위 동작으로 교체. 기존 del+재삽입(`replace_surrounding` 폴백 호출)·`fallback_pending` 제거. 정상 경로(메모장)·AutoTypeFix(정상 앱)는 불변.
5. **정리**: 폴백 전용 `fallback_pending` 카운터, 관련 로그 제거/대체.

## 영향·리스크
- 정상 앱(메모장 등 TSF-aware): **무변경**(composition_unsupported=false 경로 그대로).
- AutoTypeFix: 정상 앱에선 그대로. **터미널에선 ATF 삭제도 동일 한계** → 폴백 모드 ATF는 후속 과제(우선 기본 한글 입력 정상화).
- 런타임 검증 불가(개발자 측) → 빌드 후 사용자 wezterm/Telegram 직접 확인 필수.
- 캐럿 위치: 일부 터미널은 GetTextExt 부정확 가능 → 위치 어긋나면 후속 보정.

## 검증 시나리오 (빌드 후 사용자)
- wezterm: `기현 ` → 앱에 정확히 `기현 `, 조합 중 오버레이에 음절 표시.
- Telegram: 동일.
- 메모장(대조): 기존처럼 inline composition 유지(회귀 없음).
- 로그: 폴백 분기에서 `SetText` 삭제 호출 사라짐, `insert_text`(commit)만.
