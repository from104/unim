# Auto TypeFix 기술 조사 결과

> 조사일: 2026-04-05
> 조사자: researcher agent
> 대상: UNIM Korean IME — 자동 실시간 한영 오타 수정 기능

---

## 1. 현재 UNIM TypeFix 구현 분석

### 1.1 기존 코드 (`src/typefix.rs`)

현재 UNIM은 **수동 TypeFix** (Super+K 단축키)를 지원한다:

- `eng_to_kor(text, korean_layout, english_layout)` — 영문 키스트로크 → 한글 변환 (예: "gksrmf" → "한글")
- `kor_to_eng(text, korean_layout, english_layout)` — 한글 → 영문 키스트로크 변환 (예: "한글" → "gksrmf")
- `is_english_keystrokes(text, keyboard_map)` — 텍스트가 한글 키보드 매핑에 속하는 영문 문자로만 구성되었는지 판별
- `is_korean_text(text)` — 한글 음절/자모 범위 문자로만 구성되었는지 판별

**동작 흐름:**
1. GNOME Extension에서 `_doTypeFix(direction)` 호출
2. DBus `GlobalTypeFix` 요청 → `engine_worker`가 마지막 포커스된 컨텍스트의 `typefix_convert(direction)` 실행
3. `surrounding_text`에서 선택 영역(cursor != anchor) 추출 → 변환 → `delete_surrounding_text` + `commit_text`로 교체

**핵심 인프라 (이미 존재):**
- `InputEngine`에 `surrounding_text`, `surrounding_cursor`, `surrounding_anchor` 필드 있음
- `set_surrounding_text()` / `surrounding_text()` getter 있음
- GNOME Extension의 `unim_input_method.js`에서 `vfunc_set_surrounding(text, cursor, anchor)` 구현됨
- `delete_surrounding()` + `commit()` 파이프라인이 GNOME 환경에서 작동 중

### 1.2 자동 TypeFix에 활용 가능한 기반

| 기반 요소 | 상태 | 비고 |
|-----------|------|------|
| 한↔영 변환 함수 | ✅ 완성 | `eng_to_kor`, `kor_to_eng` |
| 언어 판별 함수 | ✅ 완성 | `is_english_keystrokes`, `is_korean_text` |
| Surrounding text 수신 | ✅ 동작 | GNOME Extension에서 DBus로 전달 |
| delete_surrounding + commit | ✅ 동작 | 수동 TypeFix에서 검증됨 |
| 키스트로크 히스토리 | ❌ 없음 | 자동 감지에 필요 |
| 사전/n-gram 데이터 | ❌ 없음 | 언어 판별 정확도 향상에 필요 |

---

## 2. 실시간 오타 감지 알고리즘

### 2.1 문제 정의

두 가지 시나리오를 감지해야 한다:

| 시나리오 | 현재 모드 | 사용자 의도 | 입력 결과 | 올바른 결과 |
|----------|-----------|-------------|-----------|-------------|
| A | 한글 | 영문 타이핑 | ㅗ디ㅣㅐ | hello |
| B | 영문 | 한글 타이핑 | gksrmf | 한글 |

### 2.2 접근법 비교

#### (1) 규칙 기반 (Rule-based) — **권장 1순위**

**원리:** 키스트로크 매핑의 결정론적 특성을 활용. 두벌식 QWERTY에서 영문→한글 매핑은 1:1이므로, 변환 결과가 "유효한 한글 단어"인지 판별하면 된다.

**시나리오 A (한글 모드에서 영문 의도) 감지:**
1. 한글 자모 조합 과정에서 키스트로크 원본(영문 문자)을 버퍼에 누적
2. 단어 경계(공백/구두점) 도달 시, 누적된 영문 문자열이 유효한 영단어인지 확인
3. 유효하면 → 한글 조합 결과를 영문으로 교체

**시나리오 B (영문 모드에서 한글 의도) 감지:**
1. 영문 문자 누적 버퍼 유지
2. 단어 경계 도달 시, `eng_to_kor()` 변환 결과가 유효한 한글 단어인지 확인
3. 유효하면 → 영문을 한글로 교체

**장점:**
- 구현 단순, 예측 가능
- 지연 시간 최소 (사전 lookup만 필요)
- 기존 `typefix.rs` 함수 재사용 가능

**단점:**
- 사전에 없는 단어(신조어, 고유명사) 미감지
- 영어/한글 모두 유효한 경우 모호성 (드묾)

#### (2) 통계 기반 (N-gram) — **권장 2순위 (보완용)**

**원리:** 문자/음절 bigram/trigram 빈도로 "자연스러운" 텍스트인지 판별.

- 한글 음절 bigram: "한글"(ㅎ→ㅏ→ㄴ, ㄱ→ㅡ→ㄹ)은 자연스러운 한글 bigram
- "ㅗ디ㅣㅐ"는 한글 음절 bigram 빈도가 극도로 낮음 → 오타 의심

**구현:**
- 한글 음절 bigram 빈도 테이블 (~11,172 × 11,172, 실제 사용 조합은 수천 개)
- 영문 character bigram 빈도 테이블 (26 × 26)
- 입력 문자열의 평균 bigram 빈도가 임계값 이하 → 오타 판정

**장점:**
- 사전 불필요, 미등록어도 감지 가능
- 고정 크기 테이블로 메모리 예측 가능

**단점:**
- 짧은 문자열(2-3자)에서 정확도 저하
- 임계값 튜닝 필요
- 오탐(false positive) 관리 필요

#### (3) 하이브리드 접근 — **최종 권장**

```
입력 완성(단어 경계) →
  1차: 규칙 기반 판별 (변환 결과가 사전에 있는가?)
    → 확실하면 교정
    → 불확실하면:
  2차: n-gram 빈도 비교 (원본 vs 변환본, 어느 쪽이 더 자연스러운가?)
    → 차이가 임계값 이상이면 교정
    → 아니면 무시
```

### 2.3 감지 시점

| 시점 | 장점 | 단점 | 적합성 |
|------|------|------|--------|
| 매 키 입력 | 즉각 반응 | 높은 오탐률, 조합 중 판단 불가 | ❌ |
| 단어 완성 시 (공백/구두점) | 완성된 단어로 판별 가능 | 약간의 지연 | ✅ **권장** |
| N자 이후 | 절충안 | 단어 중간 교정 시 UX 혼란 | △ 보조적 |
| 문장 완성 시 (Enter) | 가장 정확 | 너무 늦음 | ❌ |

**권장: 단어 경계(공백, 구두점, Enter) 도달 시 직전 단어를 검사.**

이유:
- 한글 조합이 완료된 상태에서만 정확한 판별 가능
- 사용자가 교정을 인지하기 전에 자연스럽게 교체 가능
- 기존 `delete_surrounding_text` + `commit_text` 패턴으로 구현 가능

---

## 3. 기존 IME/키보드 자동 교정 사례

### 3.1 날개셋 한글 입력기

- **접근:** "모아주기" 기능 — 세벌식에서 타자 순서가 틀린 경우 자동 정정
- **한영 자동 전환:** "복벌식" — 두벌식과 세벌식을 수동 전환 없이 자동 인식
- **원리:** 입력 오토마타 수준에서 키 시퀀스를 재해석
- **UNIM 적용:** 세벌식 지원 시 참고 가능하나, 한영 오타 감지와는 다른 문제
- **출처:** [날개셋 한글 입력기 — 나무위키](https://namu.wiki/w/%EB%82%A0%EA%B0%9C%EC%85%8B%20%ED%95%9C%EA%B8%80%20%EC%9E%85%EB%A0%A5%EA%B8%B0)

### 3.2 ibus-hangul / fcitx5-hangul

- **접근:** 한영 자동 전환 기능 없음. 수동 전환만 지원.
- **오타 교정:** 미지원
- **UNIM 적용:** 차별화 포인트. UNIM이 이 기능을 구현하면 Linux IME 최초.
- **출처:** [Arch Wiki — Korean Localization](https://wiki.archlinux.org/title/Localization/Korean)

### 3.3 macOS 한글 입력기

- **접근:** 기본 한글 입력기에 자동 한영 전환 없음
- **서드파티:** LinguaX, InputSwitcher 등이 앱별 자동 전환 제공 (앱 컨텍스트 기반, 타이핑 내용 기반 아님)
- **UNIM 적용:** 앱별 전환은 UNIM의 PerApp 모드와 유사. 타이핑 내용 기반 감지는 별개 문제.
- **출처:** [LinguaX](https://linguax.app/blog/introducing-linguax-macos-input-method-switching), [Apple Korean Input Method Guide](https://support.apple.com/guide/korean-input-method/welcome/mac)

### 3.4 Samsung / Google 한글 키보드 (Android)

- **접근:** 자동 언어 감지 기능 있음 (Samsung Keyboard). 입력 패턴으로 현재 언어 자동 감지.
- **원리:** 구체적 알고리즘 비공개. 추정: 문자 빈도 + 사전 매칭 조합.
- **한계:** 물리 키보드와 터치 키보드는 입력 특성이 다름.
- **UNIM 적용:** 컨셉은 동일하나 구현 세부사항은 참고 불가 (비공개).
- **출처:** [Samsung Keyboard Multi-language](https://eu.community.samsung.com/t5/community-newsroom/multi-language-keyboard/ba-p/1464411)

### 3.5 영타로 (Android 앱)

- **접근:** 한영 키보드 입력 오류를 자동으로 감지하여 실시간 수정
- **원리:** 구체적 알고리즘 비공개. 키보드 대체 앱으로 전체 입력 파이프라인 제어.
- **UNIM 적용:** 동일 UX 목표. IME 레벨에서 구현하므로 더 자연스러운 통합 가능.
- **출처:** [Google Play — 영타로](https://play.google.com/store/apps/details?id=com.dadak.typing_convertor&hl=en_US)

### 3.6 검색 엔진 오타 교정 (네이버, 구글)

- **접근:** SymSpell + n-gram + 사전 조합
- **원리:**
  - SymSpell: Symmetric Delete 알고리즘으로 edit distance 기반 후보 생성 (100만 배 빠름)
  - 한영 변환: 알파벳↔자음모음 매칭 후 사전 조회
  - 빈도 기반 우선순위: 단어 빈도 사전으로 교정 후보 순위 결정
- **UNIM 적용:** SymSpell의 Rust 구현 (`symspell_rs`) 존재. 사전 기반 판별에 직접 활용 가능.
- **출처:** [SymSpell GitHub](https://github.com/wolfgarbe/SymSpell), [SymSpell Rust](https://github.com/wolfgarbe/symspell_rs), [한글 맞춤법 교정 with SymSpell](https://seekstorm.com/blog/korean-spelling-correction-with-symspell/), [검색 시스템 — 오타 교정](https://medium.com/lbox-team/%EA%B2%80%EC%83%89-%EC%8B%9C%EC%8A%A4%ED%85%9C-%ED%86%BA%EC%95%84%EB%B3%B4%EA%B8%B0-1-%EA%B2%80%EC%83%89%EC%96%B4-%EC%9E%90%EB%8F%99%EC%99%84%EC%84%B1%EA%B3%BC-%EC%98%A4%ED%83%80-%EA%B5%90%EC%A0%95-%EA%B8%B0%EB%8A%A5-bf93fffa5485)

---

## 4. Wayland/GNOME 환경 기술적 제약

### 4.1 Surrounding Text 실시간 접근

#### Wayland text-input-v3 프로토콜

- `set_surrounding_text(text, cursor, anchor)` — 클라이언트 → 컴포지터 방향. 앱이 surrounding text를 IME에 전달.
- `delete_surrounding_text(before_length, after_length)` — 컴포지터 → 클라이언트 방향. IME가 surrounding text 삭제 요청.
- **중요:** 값은 double-buffered이며 `done` 이벤트에서 적용/초기화됨.
- **적용 순서:** (1) preedit 제거 → (2) surrounding 삭제 → (3) commit 삽입 → (4) 새 surrounding 계산 → (5) 새 preedit 삽입
- **인덱싱:** UTF-8 바이트 단위. 코드 포인트 중간 바이트를 가리키면 안 됨.
- **출처:** [Wayland text-input-v3 Protocol](https://wayland.app/protocols/text-input-unstable-v3)

#### GNOME Shell Clutter.InputMethod

- `clutter_input_method_commit(text)` — 텍스트 커밋
- `clutter_input_method_delete_surrounding(offset, len)` — surrounding text 삭제
- `clutter_input_method_request_surrounding()` — surrounding text 요청
- `set_surrounding` vfunc — 앱에서 surrounding text 전달받는 콜백
- **출처:** [Clutter.InputMethod API](https://mutter.gnome.org/clutter/class.InputMethod.html)

#### UNIM 현재 상태

UNIM의 `unim_input_method.js`에서 이미 구현됨:
- `vfunc_set_surrounding(text, cursor, anchor)` — surrounding text 수신
- `this.delete_surrounding(offset, charCount)` — surrounding text 삭제
- DBus를 통해 Rust Core의 `InputEngine.set_surrounding_text()`에 전달

**결론: GNOME+Wayland 환경에서 surrounding text 파이프라인은 이미 완성되어 있다.**

### 4.2 delete_surrounding + commit_text 교체 방식의 한계

| 제약 | 설명 | 심각도 |
|------|------|--------|
| 비동기 특성 | delete와 commit이 별도 이벤트로 전달되어 중간에 깜빡임 가능 | 중 |
| 앱 지원 편차 | 모든 GTK/Qt 앱이 surrounding text를 정확히 보고하지 않음 | 중 |
| 글자 수 제한 | surrounding text가 잘려서 전달될 수 있음 (앱 재량) | 하 |
| Electron 앱 | Chromium 기반 앱은 text-input-v3 지원이 불완전할 수 있음 | 중 |
| 순수 Wayland (비GNOME) | GNOME Shell 없이는 Clutter.InputMethod 사용 불가 | 고 (범위 외) |

### 4.3 Wayland 프론트엔드 상태

UNIM의 Wayland 프론트엔드(`unim-frontends/wayland/`)에서는:
- `surrounding text 정보 (현재 미사용)` — 코드 주석으로 확인
- 향후 Wayland 네이티브 환경에서도 자동 TypeFix를 지원하려면 이 부분 활성화 필요

---

## 5. 성능 제약

### 5.1 IME 키 처리 레이턴시

- 일반 키보드 입력 레이턴시: 18~30ms (스캔+디바운스)
- IME 추가 처리 허용 범위: **< 5~10ms** (사용자가 체감하지 못하는 수준)
- UNIM 스킬 명세 기준: **< 10ms**
- **출처:** [Dan Luu — Keyboard Latency](https://danluu.com/keyboard-latency/), [Computer Latency 1977-2017](https://danluu.com/input-lag/)

### 5.2 자동 감지의 추가 비용

| 연산 | 예상 소요 시간 | 빈도 |
|------|---------------|------|
| 키스트로크 버퍼 누적 | < 0.01ms | 매 키 |
| 사전 lookup (HashMap) | < 0.1ms | 단어 경계 시 |
| n-gram 빈도 계산 | < 0.5ms | 단어 경계 시 |
| eng_to_kor / kor_to_eng 변환 | < 0.1ms | 단어 경계 시 |
| delete_surrounding + commit | ~1ms (DBus 왕복) | 교정 시에만 |

**총 추가 레이턴시 (단어 경계 시): < 1ms — 10ms 제한 내에서 충분히 여유 있음.**

### 5.3 사전 기반 접근 시 메모리/속도 트레이드오프

| 사전 종류 | 항목 수 | 메모리 | 구축 난이도 |
|-----------|---------|--------|-------------|
| 영어 기본 사전 | ~50,000 단어 | ~2MB (HashMap) | 낮음 (공개 데이터) |
| 한글 기본 사전 | ~100,000 단어 | ~5MB (HashMap) | 중 (국립국어원 데이터) |
| 영어 bigram | 676 쌍 | < 1KB | 낮음 |
| 한글 음절 bigram | ~5,000 빈용 쌍 | ~40KB | 중 |
| SymSpell 전처리 인덱스 | ~200,000 | ~20MB | 중 (symspell_rs) |

**권장:** 경량 사전(영어 5만 + 한글 10만) + bigram 테이블로 시작. SymSpell은 2차 최적화에서 고려.

---

## 6. 권장 구현 전략

### 6.1 Phase 1: 최소 기능 (MVP)

```
[사용자 키 입력] → [기존 한글 조합 처리] → [키스트로크 버퍼에 원본 영문 키도 누적]
                                                    ↓ (공백/구두점 입력 시)
                                           [직전 단어 추출]
                                                    ↓
                                    [현재 모드에 따라 변환 시도]
                                    한글 모드: 영문 원본 → 영어 사전 검색
                                    영문 모드: eng_to_kor() → 한글 사전 검색
                                                    ↓
                                    [매칭되면 delete_surrounding + commit으로 교체]
                                    [모드 자동 전환 (선택적)]
```

### 6.2 필요한 새 컴포넌트

1. **키스트로크 히스토리 버퍼** (`src/input_engine.rs`)
   - 현재 단어의 원본 키스트로크(영문) 누적
   - 단어 경계에서 초기화

2. **언어 판별기** (`src/typefix.rs` 확장 또는 `src/auto_typefix.rs` 신규)
   - 사전 기반 판별: `is_valid_english_word()`, `is_valid_korean_word()`
   - n-gram 기반 자연스러움 점수: `naturalness_score()`

3. **경량 사전 데이터** (`data/` 또는 빌드 시 임베드)
   - 영어: 고빈도 50,000 단어 (MIT 라이선스 가용)
   - 한글: 국립국어원 기본 어휘 (~100,000)

4. **자동 교정 트리거** (`src/input_engine.rs`)
   - 단어 경계 감지 로직
   - 교정 실행 + `delete_surrounding_text` 시그널 발생

5. **설정 항목** (`src/config.rs`)
   - `auto_typefix_enabled: bool`
   - `auto_typefix_sensitivity: u8` (감도 조절)

### 6.3 건드리지 않아도 되는 부분

- `typefix.rs`의 기존 변환 함수 — 그대로 재사용
- GNOME Extension의 surrounding text 파이프라인 — 이미 동작 중
- DBus 서비스/클라이언트의 `delete_surrounding_text` — 이미 구현됨
- 수동 TypeFix(Super+K) — 공존 유지

---

## 7. 리스크 및 미해결 사항

| 리스크 | 설명 | 완화 방안 |
|--------|------|-----------|
| 오탐 (False Positive) | 정상 입력을 오타로 판정하여 원치 않는 교정 | 높은 신뢰도 임계값 + 되돌리기(Ctrl+Z) 지원 |
| 사전 커버리지 | 신조어/고유명사 미등록 | 사용자 사전 추가 기능, n-gram 보완 |
| UX 혼란 | 자동 교정이 사용자 의도와 다름 | 교정 직후 시각적 표시 + 되돌리기 |
| 한글 조합 중 간섭 | 자모 조합 도중에 교정하면 상태 깨짐 | 반드시 조합 완료(commit) 후에만 교정 |
| 라이선스 | 사전 데이터의 라이선스 | MIT/CC 라이선스 사전 사용 |
| Electron 앱 호환 | surrounding text 미지원 앱 | graceful fallback (교정 안 함) |

---

## 8. 참고 자료 종합

### 프로토콜/API
- [Wayland text-input-v3 Protocol](https://wayland.app/protocols/text-input-unstable-v3)
- [Clutter.InputMethod API](https://mutter.gnome.org/clutter/class.InputMethod.html)
- [Clutter.InputFocus API](https://gnome.pages.gitlab.gnome.org/mutter/clutter/class.InputFocus.html)

### 알고리즘/라이브러리
- [SymSpell — GitHub](https://github.com/wolfgarbe/SymSpell) — 100만 배 빠른 스펠링 교정
- [symspell_rs — Rust 구현](https://github.com/wolfgarbe/symspell_rs)
- [한글 맞춤법 교정 with SymSpell](https://seekstorm.com/blog/korean-spelling-correction-with-symspell/)
- [KR100509917B1 — 어절 n-gram 띄어쓰기/철자 교정 특허](https://patents.google.com/patent/KR100509917B1/ko)

### 기존 IME
- [날개셋 한글 입력기 — 나무위키](https://namu.wiki/w/%EB%82%A0%EA%B0%9C%EC%85%8B%20%ED%95%9C%EA%B8%80%20%EC%9E%85%EB%A0%A5%EA%B8%B0)
- [Arch Wiki — Korean Localization](https://wiki.archlinux.org/title/Localization/Korean)
- [LinguaX — macOS Auto Switch](https://linguax.app/blog/introducing-linguax-macos-input-method-switching)

### 성능
- [Dan Luu — Keyboard Latency](https://danluu.com/keyboard-latency/)
- [Dan Luu — Computer Latency 1977-2017](https://danluu.com/input-lag/)

### 검색/NLP
- [검색 시스템 — 오타 교정 기능 (LBOX)](https://medium.com/lbox-team/%EA%B2%80%EC%83%89-%EC%8B%9C%EC%8A%A4%ED%85%9C-%ED%86%BA%EC%95%84%EB%B3%B4%EA%B8%B0-1-%EA%B2%80%EC%83%89%EC%96%B4-%EC%9E%90%EB%8F%99%EC%99%84%EC%84%B1%EA%B3%BC-%EC%98%A4%ED%83%80-%EA%B5%90%EC%A0%95-%EA%B8%B0%EB%8A%A5-bf93fffa5485)
- [영타로 — Google Play](https://play.google.com/store/apps/details?id=com.dadak.typing_convertor&hl=en_US)
