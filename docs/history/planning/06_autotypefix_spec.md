# AutoTypeFix 정식 명세 — v1

> 작성일: 2026-04-08
> 목적: 자기야가 의도한 동작을 명확하게 정의하고, 현재 구현과의 괴리를 진단

---

## 1. 트리거 조건

### 순방향 — 영→한 (영어 모드 → 한글 교정)

**입력 컨디션:**
- 현재 입력 모드: English
- 키스트로크 버퍼에 알파벳/숫자/특수문자(세벌식 자모용) 키가 시간 윈도우(`time_window_ms`) 안에 N개 이상 쌓임

**검사 로직:**
1. 키스트로크 → ASCII 문자열 (QWERTY/Dvorak 등 영문 레이아웃 기준)
2. ASCII가 영어 사전에 있으면 진짜 영어 → **스킵** (단, 알파벳만 있는 경우)
3. `eng_to_kor(ASCII)` 시뮬 → 한글 결과
4. **완성 음절 수 ≥ `kor_syllable_threshold`** (기본 2) → 트리거

**완성 음절의 정의:**
- 완성형 음절(U+AC00~U+D7A3): 1음절
- 독립 자모(ㄱ, ㅏ 등): 0음절

### 역방향 — 한→영 (한글 모드 → 영어 교정)

**입력 컨디션:**
- 현재 입력 모드: Korean
- 키스트로크 버퍼에 알파벳 키 N개 이상

**검사 로직:**
1. 키스트로크 → ASCII 문자열 (물리 키 = QWERTY 기준)
2. **ASCII 길이 ≥ `eng_word_min_length`** (기본 5)
3. ASCII가 영어 사전에 있음 → 트리거

---

## 2. 트리거 시 화면 조작

### 순방향 예시 (세벌식 + QWERTY)

```
사용자 입력: kfxld (5키, 영어 모드)
화면 변화:
  k 입력 → 화면 "k"
  f 입력 → 화면 "kf"
  x 입력 → 화면 "kfx"
  l 입력 → 화면 "kfxl"
  d 입력 → 트리거! (eng_to_kor("kfxld") = "각지" 2음절)
교정 후:
  - d의 commit 억제 (화면에 안 나감)
  - 화면에 "kfxl" 4글자 있음
  - 백스페이스 4번
  - "각" commit (1글자)
  - "지" preedit (조합 상태)
  - 모드 한글로 전환
  - 엔진 상태: 마지막 음절 "지"의 조합 상태 (ㅈ+ㅣ)
최종 화면: "각지" (지는 preedit)
```

### 역방향 예시 (세벌식 + QWERTY)

```
사용자 입력: speed (5키, 한글 모드)
화면 변화:
  s → 화면 "ㅣ" (preedit)
  p → 화면 "ㅣ펴" or "ㅣㅍ" (commit + preedit)
  e → 화면 "ㅣ펴" 정도
  e → 화면 "ㅣ펴ㅕ"
  d → 트리거! (사전 매칭 "speed", 5자)
교정 후:
  - d의 commit/preedit 억제
  - 화면 글자 수: eng_to_kor("speed") = "ㅣ펴ㅕㅣ" = 4글자
  - 백스페이스 4번
  - "speed" commit (5글자)
  - preedit 없음
  - 모드 영어로 전환
최종 화면: "speed"
```

**핵심 차이:**
- 순방향: `delete_chars` = 키 수 - 1 (trigger 키 commit 억제분)
- 역방향: `delete_chars` = `eng_to_kor(ascii).chars().count()` (한글:키 비율 ≠ 1:1)

---

## 3. 모드 전환

- 트리거 → 백스페이스 → commit → preedit → **모드 전환**
- 모드 전환은 데몬의 `GlobalModeChanged` 시그널로 인디케이터/UI에 즉시 반영
- Global 모드면 다른 컨텍스트도 동기화

---

## 4. 엣지 케이스

| 케이스 | 동작 |
|--------|------|
| 트리거 후 즉시 다른 키 | 새 모드의 새 입력으로 처리 (preedit 상태 활용) |
| Ctrl+Z (트리거 직후) | 원본 keystroke 복원 |
| 비문자 키 (Enter/Tab/Backspace) | 버퍼 초기화 |
| 모드 전환 키 | 버퍼 초기화 |
| 포커스 변경 | 버퍼 초기화 |

---

## 5. 프론트엔드 일관성

모든 프론트엔드는 동일한 결과를 만들어야 함:

| 프론트엔드 | 백스페이스 | commit | preedit |
|-----------|-----------|--------|---------|
| GNOME ext | `vkbd.backspaceMultiple(N)` | `commitText` | `updatePreedit` |
| GTK3/4 | `gtk_im_context_delete_surrounding(-N, N)` | `g_signal_emit "commit"` | `unim_dbus_set_preedit_cache` + `preedit-changed` |
| Qt5/6 | `QInputMethodEvent.setCommitString(text, -N, N)` (한 번에) | 동일 | 별도 처리 |
| XIM | `XSendEvent(BackSpace)` x N | `commit_string` | XIM preedit |
| Wayland | (보류) | (보류) | (보류) |

---

## 6. 현재 구현 vs 명세 — 괴리 진단

### 🔴 치명적 버그

#### 1. 백스페이스 카운트 손실 (모든 프론트엔드)

**증상**: `vkbd.backspaceMultiple(3)` 호출했는데 wezterm에서 1번만 실행됨

**원인 가설**:
- (a) 가상 키보드 백스페이스 이벤트가 너무 빨라서 앱이 못 따라잡음
- (b) GTK `delete_surrounding(-3, 3)`이 surrounding text 미지원 앱에서 일부만 처리
- (c) 백스페이스 사이에 지연 없음

**해결**:
- 각 백스페이스 사이에 짧은 지연 (5~10ms) 추가
- 또는 백스페이스 한 번에 N글자 처리하는 API 사용 (있다면)

#### 2. preedit 캐시 ↔ 엔진 상태 불일치

**증상**: GTK에서 preedit "기" 표시는 됐지만, 다음 키 입력 시 commit 안 되고 사라짐

**원인**:
- 데몬 엔진은 replay로 "기"의 조합 상태(ㅈ+ㅣ)를 만들었지만
- 프론트엔드의 preedit 캐시(`unim_dbus_set_preedit_cache`)는 단순 문자열만 저장
- 다음 키 입력 시 데몬이 새 키만 처리 → "기" commit 누락

**해결**:
- 프론트엔드 preedit은 표시 전용으로만 사용
- 다음 키 처리 시 데몬이 자동으로 "기"를 commit하도록 엔진 동작 보장
- 또는 preedit-changed 시그널과 함께 commit 강제

### 🟡 잠재적 버그

#### 3. wezterm은 어떤 프론트엔드?

wezterm이 어떤 IME 프론트엔드를 쓰는지 확인 필요:
- Wayland native? → unim-wayland (보류 상태)
- XWayland? → XIM
- GTK?

이게 첫 번째 진단 단계.

#### 4. 역방향에서 commit_text가 잘못 계산될 수 있음

`eng_to_kor("speed")` 결과가 항상 4글자라고 가정 — 실제로는 세벌식 따라 다를 수 있음.

#### 5. 시그널 타이밍

ProcessKeyEvent 동기 응답과 AutoTypefixApply 비동기 시그널 간 순서 보장 안 됨.

---

## 7. 다음 액션 (우선순위)

1. **wezterm이 사용하는 프론트엔드 확인** (XIM? Wayland? GTK?)
2. **백스페이스 타이밍 디버깅** — 각 백스페이스 사이 지연 추가
3. **preedit ↔ 엔진 상태 동기화** — 다음 키에서 "기" commit 보장
4. **역방향 백스페이스 카운트 검증** — 로그에 실제 화면 글자 수 출력
5. **재구현 또는 패치** — 진단 결과에 따라

---

## 8. 검증 시나리오

각 시나리오에서 명세대로 동작해야 함:

### A. 영→한 (순방향)
- [ ] 영어 모드에서 `kfxld` → "각지" (지는 preedit)
- [ ] 영어 모드에서 `ntkd` → "서기" (기는 preedit)
- [ ] preedit 상태에서 추가 키 → 정상 한글 조합
- [ ] preedit 상태에서 Space → "기 " 정상 commit

### B. 한→영 (역방향)
- [ ] 한글 모드에서 `speed` 키 입력 → "speed" (백스페이스 4번 + commit 5글자)
- [ ] 트리거 후 Space → 정상 공백 입력
- [ ] 앞 단어 공백 보존
- [ ] 모드가 영어로 전환됨

### C. 다중 프론트엔드
- [ ] GNOME ext (Wayland)
- [ ] GTK4 (gnome-text-editor)
- [ ] GTK3 (구형 앱)
- [ ] Qt6 (KDE 앱)
- [ ] XIM (xterm/wezterm?)
