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
- **취소**: Escape 또는 미등록 키 입력 시 닫기 + 원래 문자 유지
- **포커스 이동 시 자동 닫기**

### 4.2 특수문자 팝업
- **위치**: 커서(caret) 바로 아래
- **화면 경계 처리**: 동일
- **레이아웃**: 9×9 그리드, 열 우선 채움
- **선택**: top_row 키(q~o)로 열 점프, 숫자 1-9로 행 선택
- **페이지**: Tab/Shift+Tab, PgUp/PgDn으로 이동
- **취소/자동 닫기**: 한자 팝업과 동일

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
