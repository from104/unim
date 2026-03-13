# 한글 입력기(IME) 동작 명세

UNIM의 모든 프론트엔드(GTK3, GTK4, Qt5, Qt6, XIM, Wayland, GNOME Extension)가 준수해야 하는 한글 입력 동작 규격.

---

## 1. 조합(Composition) 기본 동작

### 1.1 조합 중 텍스트 표시 (Preedit)
- 한글 조합 중인 글자는 **preedit**(조합 문자열)로 표시
- preedit은 커서 위치에 인라인으로 표시 (앱이 지원하는 경우)
- 앱이 인라인 preedit을 지원하지 않으면 **오버레이 팝업**으로 표시

### 1.2 조합 확정 (Commit)
- 조합이 완료되면 확정된 텍스트를 앱에 **commit**
- commit 후 preedit은 클리어

---

## 2. 포커스 동작

### 2.1 포커스 획득 (Focus In)
- 새로운 텍스트 필드에 포커스 획득 시 입력 컨텍스트를 활성화
- DBus `FocusIn(windowId)` 호출

### 2.2 포커스 상실 (Focus Out)
- **조합 중이면 즉시 commit** (조합 중이던 글자를 확정)
- preedit 클리어
- **팝업(한자/특수문자)이 열려있으면 닫기** + 해당 모드 취소
- DBus `FocusOut()` 호출 → 반환된 commit 텍스트를 앱에 전달

### 2.3 클릭으로 커서 이동
- 같은 텍스트 필드 내에서 다른 위치를 클릭하면 포커스 이동과 동일하게 처리
- 조합 중이면 commit 후 커서 이동

---

## 3. 키 분류별 동작

### 3.1 문자 키 (한글/영문)
- 한글 모드: 한글 조합 로직에 따라 preedit 업데이트 또는 commit
- 영문 모드: 그대로 앱에 전달 (바이패스)

### 3.2 수정자 키 (Modifier)
- `Shift`, `Ctrl`, `Alt`, `Super`, `Meta`, `Hyper`, `CapsLock`, `NumLock`, `ScrollLock`
- **단독 입력 시 무시** (소비하지 않음)
- 조합 상태에 영향 없음

### 3.3 Ctrl/Alt/Super 조합
- `Ctrl+C`, `Alt+F4`, `Super+L` 등
- **조합 중이면 commit 후 바이패스**
- IME가 소비하지 않음 → 시스템/앱 단축키로 전달

### 3.4 네비게이션 키
- `←` `→` `↑` `↓`, `Home`, `End`, `Page Up`, `Page Down`, `Insert`, `Delete`
- **조합 중이면 commit 후 바이패스**
- 키 자체는 앱에 전달되어 커서 이동 등 원래 동작 수행

### 3.5 Enter / KP Enter
- **조합 중이면 commit 후 바이패스**
- Enter 키 자체는 앱에 전달 (줄바꿈)
- 이중 커밋 방지: processKey를 거치지 않고 직접 flush

### 3.6 Escape
- **조합 중이면 commit 후 바이패스**
- 팝업이 열려있으면 팝업 닫기 + 모드 취소

### 3.7 Tab / Shift+Tab
- **조합 중이면 commit 후 바이패스**
- 앱에 전달 (포커스 이동)

### 3.8 한영전환 키 (Toggle)
- `Hangul`, `Shift+Space` 등 (설정 가능)
- 한글↔영문 모드 전환
- **조합 중이면 commit 후 전환**

### 3.9 한자키 (Hanja)
- `F9`, `Hangul_Hanja` 등 (설정 가능)
- 조합 중이거나 직전 커밋된 문자에 대해 한자 후보 팝업 표시
- 한자 후보가 없으면 특수문자 후보 폴백

### 3.10 BackSpace
- 조합 중이면 조합 문자의 마지막 자모 삭제
- 조합 중이 아니면 앱에 전달 (일반 백스페이스)

---

## 4. 팝업 동작

### 4.1 한자 팝업
- **위치**: 커서(caret) 바로 아래
- **화면 경계 처리**: 오른쪽/아래 넘침 시 왼쪽/위로 조정
- **선택**: 숫자 1-9 직접 선택, ↑↓ 네비게이션, Enter 확정
- **페이지**: ← → PgUp PgDn Space로 이동
- **선택 시 커밋 플로우**: SelectHanja → CancelHanja(엔진 preedit 리셋) → clearPreedit → commit
- **취소**: Escape 또는 미등록 키 입력 시 닫기 + 원래 문자 유지
- **포커스 이동 시 자동 닫기**

### 4.2 특수문자 팝업
- **위치**: 커서(caret) 바로 아래
- **화면 경계 처리**: 동일
- **레이아웃**: 9×9 그리드, 열 우선 채움
- **선택**: top_row 키(q~o)로 열 점프, 숫자 1-9로 행 선택
- **페이지**: Tab/Shift+Tab, PgUp/PgDn으로 이동
- **선택 시 커밋 플로우**: SelectSpecialChar → CancelSpecialChar → clearPreedit → commit
- **취소/자동 닫기**: 한자 팝업과 동일

### 4.3 팝업에서 미처리 키 동작
- 팝업이 처리하지 않는 키(일반 문자, 등록 안 된 키)가 입력되면:
  1. 팝업 닫기 + 해당 모드 취소 (CancelHanja / CancelSpecialChar)
  2. **나머지 IME 키 처리 로직으로 fall-through** (네비게이션 키 → commit+바이패스, 문자 키 → ProcessKey)
  3. 즉시 return하지 않음 — GTK3 immodule.c의 "미지원 키 → 닫기 → fall-through" 패턴을 따름

---

## 5. 텍스트 전달 경로

### 5.1 Wayland (GNOME Extension)
```
키 입력 → Mutter → vfunc_filter_key_event (ClutterInputMethod)
    → consumed=true  → commit()/set_preedit_text() → text-input-v3 → 앱
    → consumed=false  → wl_keyboard.key() → 앱
```

### 5.2 GTK3/GTK4 (IM Module)
```
키 입력 → GtkIMContext.filter_keypress()
    → DBus ProcessKeyEvent → commit/preedit 시그널 → 앱
```

### 5.3 Qt5/Qt6 (IM Module)
```
키 입력 → QInputMethod::filterEvent()
    → DBus ProcessKeyEvent → commitString/preeditString → 앱
```

### 5.4 XIM
```
키 입력 → XIM 프로토콜 → forward_event
    → DBus ProcessKeyEvent → XIM commit/preedit → 앱
```

---

## 6. 이중 처리 방지

### 6.1 vfunc + captured-event 중복 (GNOME Extension)
- Backend에 커스텀 IM이 등록되면 `captured-event` 핸들러에서 `EVENT_PROPAGATE` 반환
- vfunc이 우선 처리하므로 captured-event에서 재처리하지 않음

### 6.2 Enter/네비게이션 키 이중 커밋 방지
- `processKey`를 거치지 않고 직접 `_flushCompose()` 호출
- flush 후 `return false`로 키를 앱에 전달

---

## 7. 프론트엔드 구현 체크리스트

새 프론트엔드 추가 시 검증 항목:

- [ ] 한글 조합/확정 동작
- [ ] preedit 인라인 표시
- [ ] 포커스 인/아웃 시 커밋
- [ ] 네비게이션 키 커밋+바이패스
- [ ] Enter 커밋+바이패스 (이중 커밋 없음)
- [ ] Ctrl/Alt 조합 바이패스
- [ ] 한영전환 동작
- [ ] 한자/특수문자 팝업 표시/선택/취소
- [ ] 팝업 커서 위치 배치 + 경계 조정
- [ ] 포커스 이동 시 팝업 자동 닫기
- [ ] BackSpace 자모 삭제
- [ ] 팝업 키 처리: PopupState 위임 (C-API 또는 직접 사용)
- [ ] PopupNavigate 시그널 수신 → 팝업 UI 업데이트

---

## 8. 프런트엔드 공통 입력 처리 패턴

모든 프런트엔드는 언어/프레임워크가 다르지만 동일한 입력 처리 시퀀스를 따라야 한다.
아래 패턴은 프런트엔드 간 동작 일관성의 기준이며, 새 프런트엔드 추가 시 반드시 이 순서를 구현해야 한다.

### 8.1 ProcessKeyEvent 결과 처리

```
1. DBus 호출: ProcessKeyEvent(keyval, keycode, state) → (consumed, preedit, commit)
2. commit 처리 (먼저):
   - commit이 비어있지 않으면 → 앱에 commit 전달
   - 방법: GTK commit_string / Qt commitString / XIM server.commit / Wayland virtual_keyboard / GNOME commitText
3. preedit 처리 (commit 후):
   - preedit이 비어있으면 → preedit 클리어 (빈 문자열로 설정)
   - preedit이 있으면 → preedit 표시 업데이트
   - 방법: GTK set_preedit_string / Qt setPreeditString / XIM PreeditDraw / Wayland set_preedit / GNOME setPreeditText
4. consumed 반환:
   - true → 키 소비 (앱에 전달하지 않음)
   - false → 키 통과 (앱에 전달)
```

**주의 (XIM)**: commit 전에 `clear_preedit()`를 호출하면 안 됨. PreeditDone이 먼저 전송되어 일부 클라이언트에서 세션이 닫힘.

**순서가 중요한 이유**: commit → preedit 순서를 지키지 않으면 조합 중 문자가 이중 커밋되거나 누락됨.

### 8.2 포커스 획득 (Focus In) 시퀀스

```
1. 윈도우 ID 생성 (플랫폼별):
   - GTK: "gtk3-win-0x{hwnd}" / "gtk4-win-0x{hwnd}"
   - Qt:  "qt5-win-0x{hwnd}" / "qt6-win-0x{hwnd}"
   - XIM: "xim-win-0x{hwnd}"
   - Wayland: "wayland-{app_id}"
   - GNOME: "gnome-extension"
2. DBus FocusIn(windowId) 호출
3. 내부 상태 초기화:
   - preedit_cache = ""
   - is_composing = false
4. 이전 컨텍스트의 팝업이 남아있으면 닫기
5. 이전 preedit 복원 금지 — 매 포커스마다 새 컨텍스트
```

### 8.3 포커스 상실 (Focus Out) 시퀀스

```
1. 조합 중 확인: preedit_cache가 비어있지 않으면
   → 현재 preedit을 앱에 commit
2. 열린 팝업 닫기:
   → 팝업 윈도우 hide/destroy
   → DBus CancelHanja 또는 CancelSpecialChar 호출 (팝업 컨텍스트가 있는 경우)
3. DBus FocusOut() 호출
   → 반환된 commit 텍스트가 있으면 앱에 전달
4. preedit 표시 클리어
5. 내부 상태 리셋:
   - preedit_cache = ""
   - is_composing = false
```

**순서가 중요**: commit → 팝업 닫기 → DBus 호출 → 표시 클리어 → 상태 리셋

### 8.4 수정자 키 필터링

```
■ 단독 수정자 키 (즉시 바이패스, ProcessKeyEvent에 보내지 않음):
  Shift_L/R, Control_L/R, Alt_L/R, Super_L/R,
  Meta_L/R, Hyper_L/R, Caps_Lock, Num_Lock, Scroll_Lock

■ Ctrl/Alt/Super 조합 (시스템 단축키):
  1. 조합 중이면 → preedit commit (flush)
  2. consumed=false 반환 → 앱/시스템에 키 전달
  3. ProcessKeyEvent에 보내지 않음 (시스템 단축키이므로)

■ Shift 조합 (문자 키):
  → 일반 키와 동일하게 ProcessKeyEvent에 전달 (대문자/특수문자 입력)
```

### 8.5 consumed=false 키 전달 방식

| 프런트엔드 | 방식 |
|-----------|------|
| GTK3/4 | `filter_keypress()` 에서 `FALSE` 반환 → GTK가 앱에 전달 |
| Qt5/6 | `filterEvent()` 에서 `false` 반환 → Qt가 앱에 전달 |
| XIM | `server.forward_key()` 호출 → XIM 프로토콜로 클라이언트에 KeyPress 합성 |
| Wayland | `virtual_keyboard.key()` 호출 → compositor가 앱에 전달 |
| GNOME | `EVENT_PROPAGATE` 반환 → Clutter/Mutter가 앱에 전달 |

### 8.6 팝업 키 처리 위임 (PopupState 통합)

팝업이 활성화된 상태에서의 키 처리는 `PopupState`(Rust `src/popup/`)에 위임:

```
■ Rust 프런트엔드 (XIM, Wayland):
  → PopupState를 직접 사용
  → keysym → PopupKey 변환 → PopupState::handle_key() → PopupKeyResult

■ C/C++ 프런트엔드 (GTK, Qt):
  → unim-capi FFI 경유
  → unim_popup_key_from_gdk/qt(keyval) → unim_popup_handle_key() → CPopupKeyResult

■ GNOME Shell Extension:
  → 팝업 활성 시 키를 데몬 ProcessKeyEvent로 전달
  → 데몬이 PopupState로 처리 후 DBus 시그널로 결과 통지:
    - PopupNavigate(page, totalPages, selected, rows, cols, selRow, selCol) → UI 업데이트
    - HidePopup → 팝업 닫기
    - commit 텍스트 → 선택된 문자 커밋

■ PopupKeyResult 처리 (공통):
  - Select(index) → 해당 문자 commit + 팝업 닫기
  - Cancel → 원래 문자 유지 + 팝업 닫기
  - Updated → 상태(page/sel_row/sel_col 등) 동기화 + UI 재렌더링
  - Consumed → 키 소비, 변경 없음
  - NotHandled → 팝업 닫기 + 키를 일반 입력으로 fall-through
```

### 8.7 DBus 인터페이스 요약

프런트엔드 → 데몬:

| 메서드 | 파라미터 | 반환 | 용도 |
|-------|---------|------|------|
| `CreateInputContext` | (client_name, window_id) | object_path | 컨텍스트 생성 |
| `ProcessKeyEvent` | (keyval, keycode, state) | (consumed, preedit, commit) | 키 처리 |
| `FocusIn` | (window_id) | - | 포커스 획득 |
| `FocusOut` | - | commit_text | 포커스 상실 |
| `Reset` | - | - | 상태 리셋 |

데몬 → 프런트엔드 (시그널):

| 시그널 | 파라미터 | 용도 |
|-------|---------|------|
| `ShowHanjaPopup` | (target, candidates, cursor_rect) | 한자 팝업 표시 |
| `ShowSpecialPopup` | (target, characters, top_row, cursor_rect) | 특수문자 팝업 표시 |
| `HidePopup` | - | 팝업 닫기 |
| `PopupNavigate` | (page, totalPages, selected, rows, cols, selRow, selCol) | 팝업 상태 업데이트 |
| `GlobalModeChanged` | (is_korean) | 한/영 모드 변경 알림 |
