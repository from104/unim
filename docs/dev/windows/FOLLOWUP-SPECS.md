# 후속 설계 사양서 — 범용 사용자사전/상용구 + classify_key 통합

> 상태: **설계 초안 (착수 대기)** · 작성일 2026-07-04 · 기준 버전 v0.3.54 (feat/windows-msi-redesign)
>
> 두 항목 모두 자동 구현을 보류한 이유:
> - (A)는 제품 정의(등록 대상·트리거 UX)가 사용자 승인 게이트를 요구한다.
> - (B)는 입력 핫패스(OnTestKeyDown/OnKeyDown)를 건드려 실기기 device-QA 없이는
>   무회귀를 보장할 수 없다.

---

## A. 범용 사용자 낱말/상용구 사전

### A-1. 목적과 범위

기존 `src/typefix_userdict.rs` 는 **AutoTypeFix 역방향(한→영) 교정 전용
영문 whitelist** 다 — ASCII 알파벳만 허용(`is_ascii_alphabetic` 가드), 매칭도
`contains_reverse(word_lower)` 단일 용도. "사용자 사전"이라는 이름과 달리
일반 IME 의 사용자 사전(낱말 등록·상용구·약어 확장)이 아니다.

본 사양은 이를 보완하는 **범용 사전**을 신설한다. 대상:

| 유형 | 예 | 동작 |
|------|----|------|
| 한글 낱말 | 고유명사 "긼벼루" | 조합 확정 시 그대로 통과 보증(ATF 오교정 억제 목록) |
| 한자 매핑 | "서울" → 徐蔚 | 한자 팝업 후보 목록에 사용자 항목 우선 노출 |
| 구절(상용구) | "메일서명" → 3줄 서명 | 트리거 키 입력 시 확장 삽입 |
| 약어 확장 | "ㄳ" → "감사합니다", "addr" → 주소 | 경계 문자 확정 시 자동 치환 또는 후보 제안 |

### A-2. 파일·스키마 (신규, typefix-userdict 와 분리)

파일: `~/.config/unim/user-dictionary.yaml` (Windows: `%APPDATA%\unim\`,
`crate::paths::config_dir()` 공용 — typefix-userdict 와 동일 해석).

**분리 이유**: typefix-userdict 는 "역방향 교정 억제" 의미(blacklist 의 반대)로
이미 배포됐고 스키마 version=1 이 ASCII 전용 계약을 갖는다. 의미가 다른
데이터를 한 파일에 합치면 CLI/GUI/핫리로드 3곳의 하위호환 분기가 늘어난다.
(기존 `typefix_blacklist` ↔ `typefix_userdict` 파일 분리 선례와 동일 원칙.)

```yaml
# user-dictionary.yaml (스키마 v1 제안)
version: 1
entries:
  - kind: word            # 한글 낱말 (ATF 오교정 억제)
    text: "긼벼루"
    note: "제품명"
    added_at: 1780000000
  - kind: hanja           # 사용자 한자 후보
    reading: "서울"
    hanja: "徐蔚"
    meaning: "인명"
    added_at: 1780000000
  - kind: snippet         # 상용구/약어 확장
    abbrev: "메일서명"     # 트리거 문자열 (한글/영문/혼합 허용)
    expansion: |
      홍길동 드림
      010-0000-0000
    trigger: manual        # manual | boundary (A-4 참조)
    added_at: 1780000000
```

Rust 표현(신규 `src/user_dictionary.rs`, 코어 `unim` 크레이트):

```rust
#[derive(Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum DictEntry {
    Word    { text: String, note: Option<String>, added_at: u64 },
    Hanja   { reading: String, hanja: String, meaning: Option<String>, added_at: u64 },
    Snippet { abbrev: String, expansion: String, trigger: SnippetTrigger, added_at: u64 },
}
```

- serde 내부태그 enum → 미래 kind 추가 시 unknown variant 를 **경고 후 보존**
  (파싱 실패로 파일 전체를 버리지 않도록 `serde(other)` 폴백 또는 2단 파싱).
- 로딩/저장/핫리로드(2초 스로틀 mtime 비교)/원자적 저장(`*.tmp` → rename)은
  `typefix_userdict.rs` 의 검증된 패턴을 그대로 복제한다.

### A-3. typefix-userdict 와의 관계·차이 (요약표)

| | typefix-userdict (기존) | user-dictionary (신규) |
|---|---|---|
| 파일 | `typefix-userdict.yaml` | `user-dictionary.yaml` |
| 의미 | 역방향 한→영 교정 **허용** whitelist | 낱말 보호·한자 후보·상용구 **확장** |
| 문자 | ASCII 알파벳만 | 유니코드 전체(한글 포함) |
| 참조 지점 | `auto_typefix::check_reverse` (UserDictGate) | ATF forward 억제 + 한자 팝업 + 커밋 파이프라인 |
| 마이그레이션 | 불필요(그대로 유지) | 없음(신규) |

`kind: word` 는 개념상 "ATF 순방향 오교정 억제"라 typefix-blacklist 와 겹쳐
보이나, blacklist 는 "교정하지 말 것"(영문 결과 차단)이고 word 는 "이 한글
낱말은 사전에 없어도 정상"(순방향·역방향 모두 억제 + 향후 조합 후보 가중치)
로 상위 개념이다. 구현 1단계에서는 word → 양방향 ATF 억제만 연결한다.

### A-4. 적용 시점 (핵심 설계 결정)

1. **word**: `auto_typefix::process_after_key` 의 check_forward/check_reverse
   진입 전에 커밋 예정 문자열이 사전에 있으면 교정을 스킵. 핫패스 비용은
   HashSet 조회 1회(사전 로드 시 인덱스 구축, 핫리로드 시 재구축).
2. **hanja**: 엔진 한자 후보 생성 시(`popup` 모듈) 사용자 항목을 후보 리스트
   선두에 병합. 팝업 렌더러(unim-popup-win)는 무수정 — 후보 데이터만 바뀜.
3. **snippet**:
   - `trigger: manual` — 확장 전용 키(기본 미지정, 설정 필요. 후보:
     Ctrl+Shift+E)를 누르면 캐럿 앞 문자열을 abbrev 로 역매칭해 치환.
     치환은 기존 `replace_surrounding` 경로(ATF 와 동일 인프라) 재사용 —
     Chrome 펌프-분할 불변식(INVARIANT-chrome-pump-split.md)·synth 폴백을
     그대로 상속하므로 신규 문서 조작 코드가 0 이다.
   - `trigger: boundary` — 경계 문자(Space/Enter/문장부호) 확정 시 자동 치환.
     **오발동 위험이 높아 기본 OFF**, 엔트리별 opt-in.
4. 다중문자 치환은 반드시 **commit 이후 텍스트**에만 적용한다(라이브 조합
   내부 치환 금지 — Word D1/word 모드 라이브 조합과 충돌하지 않도록).

### A-5. GUI — 결정 필요(사용자 게이트) 포함

현행: `unim-tsf-settings`(Slint) "사용자 사전" 탭 = typefix-userdict 전용
(word+note 2필드 리스트, 즉시 저장, 삭제 되돌리기 토스트).

**제안: 신규 탭 추가** ("낱말/상용구" 탭). 기존 탭은 "교정 예외(영문)"로
개칭. 이유: kind 3종은 필드 구성이 서로 달라(단어 1필드 vs 읽기+한자+뜻
3필드 vs 약어+본문+트리거 3필드) 한 리스트에 섞으면 편집 폼이 모달 분기
지옥이 된다. Slint StandardListView 재사용 + kind 콤보로 입력 폼 전환.

Linux 쪽(unim-settings GTK)도 동일 구조로 추가하되 별도 PR (검증 환경 분리).

### A-6. CLI

`unim-cli` 에 신규 서브커맨드 그룹 (기존 `config userdict` 와 병렬):

```
unim config dict list [--kind word|hanja|snippet]
unim config dict add-word <한글낱말> [--note ...]
unim config dict add-hanja <읽기> <한자> [--meaning ...]
unim config dict add-snippet <약어> <본문> [--trigger manual|boundary]
unim config dict remove <index|텍스트>
unim config dict clear [--kind ...]
```

### A-7. 동기화 지점 (settings-sync-check 6지점 대응)

| # | 지점 | 작업 |
|---|------|------|
| 1 | `src/config.rs` | 사전 자체는 별도 yaml 이므로 Config 필드는 **스위치만**: `engine.user_dictionary.enabled`(bool, 기본 true), `snippet_boundary_trigger`(bool, 기본 false), `snippet_manual_key`(Option<String>) |
| 2 | `unim-cli` | A-6 서브커맨드 + `config get/set` 신규 키 |
| 3 | locales (`unim-cli`/`unim-settings`/`unim-tsf-settings` ko·en) | 신규 키 전부 양언어 |
| 4 | `unim-dbus` | GetConfig/SetConfig 에 신규 키 노출 + 사전 파일 변경 시 ConfigChanged 시그널(Linux) |
| 5 | GTK UI (`unim-settings`) + Slint (`unim-tsf-settings`) | A-5 탭 |
| 6 | GNOME gschema (`unim-gnome-extension`) | 스위치 3키 미러링 |

Windows(TSF)는 DBus 미경유 — 핫리로드(mtime)가 반영 경로이므로 4번은
Linux 한정. Windows 는 `maybe_reload_config` 패턴에 사전 리로드를 편승.

### A-8. 결정 필요 사항 (사용자 승인 게이트)

1. **G1** boundary 트리거 기본값 — 제안 OFF. 자동 치환 오발동 허용도는
   사용자만 판단 가능.
2. **G2** manual 트리거 기본 키 — Ctrl+Shift+E 제안(Ctrl+Shift+Space 는 수동
   ATF 가 선점). 충돌 검토 필요.
3. **G3** GUI 신규 탭 vs 기존 탭 확장 — 신규 탭 제안(A-5 근거). UX 취향 게이트.
4. **G4** hanja 사용자 후보의 팝업 내 표기(★ 등 구분 마커 여부).
5. **G5** snippet 확장 최대 길이 제한(제안 2,000자 — synth 폴백 앱에서
   SendInput 배치 폭주 방지).

### A-9. 수용 기준 (A)

- [ ] `user-dictionary.yaml` 라운드트립 단위테스트(3 kind 전부) 통과, 손상
      파일 → 빈 사전 degrade(패닉 0).
- [ ] word 등록 시 해당 낱말이 ATF 순방향·역방향 모두에서 교정되지 않음
      (코어 테스트 + 메모장 실측).
- [ ] hanja 등록 시 한자 팝업 첫 페이지 선두에 후보 노출(중복 병합 검증).
- [ ] snippet manual 트리거가 메모장·Chrome·wezterm(synth 폴백) 3계열에서
      약어를 정확히 치환 — 펌프-분할 불변식 회귀 없음.
- [ ] boundary 트리거 OFF 상태에서 어떤 자동 치환도 발생하지 않음(무회귀).
- [ ] 6지점 동기화 완료 — settings-sync-check 체크리스트 PASS.
- [ ] `cargo test -p unim` all-pass, Windows 크레이트 zero-warning 빌드.

---

## B. 키 disposition `classify_key` 통합

### B-1. 문제 — 판정 로직 3중 포크

동일한 "이 키에서 조합을 확정하고 통과시킬 것인가" 판정이 3곳에 산재한다:

| 포크 | 위치 | 내용 |
|------|------|------|
| F1 | `text_service.rs::OnTestKeyDown` 인라인 (약 865–1045행) | key-repeat 억제 → bare-modifier 투과 → modifier-combo commit+pass → navkey commit+pass → numpad commit+pass → english-hold BS 소비 → `test_key_down` |
| F2 | `key_handler.rs::test_key_down` | 토글/팝업/한자키/한글 문자키/ATF 영문 소비 판정 |
| F3 | `key_handler.rs::handle_key_down` GAP2 블록 (약 427–457행) | **F1 의 navkey/numpad 게이트 복제** — OnTestKeyDown 미발화 앱(wmux/xterm.js) 전용 보강 |

여기에 synth echo 회계가 `synth_input::observe_test_key_down`(F1 앞단)과
`observe_key_down` Case A/B(OnKeyDown 앞단)로 다시 이원화되어 있다.

증상: 게이트 하나를 고칠 때마다 F1/F3 를 같이 고쳐야 하고(실제로 GAP2 는
wmux 결함의 사후 패치), F1 은 OnTestKeyDown 안에서 `commit_for_passthrough`
로 **문서를 변형**한다 — TSF 계약상 OnTestKeyDown 은 "먹을지 여부만
답하는" 비변형 콜백이어야 한다.

### B-2. 목표 설계 — 순수 함수 `classify_key`

신규 `unim-tsf/src/key_disposition.rs`:

```rust
/// 키 1개에 대한 처리 방침. 판정만 하고 어떤 부작용도 없다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyDisposition {
    /// IME 가 소비 (eaten=TRUE → OnKeyDown 에서 엔진 처리)
    Consume,
    /// 현재 조합(한글/english-hold)을 먼저 확정한 뒤 키를 앱으로 통과
    CommitThenPassthrough,
    /// 그대로 통과 (조합 불변)
    Passthrough,
}

/// 순수 판정 함수. 입력은 전부 값/스냅샷 — 락·COM·전역 접근 금지.
pub fn classify_key(input: &KeyInput, state: &KeySnapshot, cfg: &KeyPolicy) -> KeyDisposition;

pub struct KeyInput  { pub vk: u16, pub kc: KeyCode, pub modifiers: ModifierState, pub is_repeat: bool }
pub struct KeySnapshot {
    pub composing: bool,          // engine.is_composing()
    pub english_hold: bool,       // comp_mgr.english_hold_active()
    pub popup_active: bool,
    pub input_category: InputCategory,
    pub is_toggle: bool,          // engine.is_toggle_key(kc)
}
pub struct KeyPolicy { pub ignore_key_repeat: bool, pub atf_forward: bool /* … */ }
```

판정 순서(현행 F1 순서를 그대로 표로 옮김 — 의미 변경 0):

1. repeat 억제 조건 → `Consume`(no-op 소비)
2. bare-modifier(0x10/0x11/0x12 포함, 토글 제외) + 조합중 → `Passthrough`
3. modifier-combo + 조합중 → `CommitThenPassthrough`
4. navkey(`is_commit_passthrough_key`) + 조합중 → `CommitThenPassthrough`
5. numpad(`is_numpad_vk`) + 조합중 → `CommitThenPassthrough`
6. english-hold + Backspace → `Consume`
7. 그 외 → 기존 `test_key_down` 판정을 함수 내부로 흡수(`Consume`/`Passthrough`)

호출 지점 통합:

- **OnTestKeyDown**: `classify_key` 1회 → `Consume`→TRUE,
  `CommitThenPassthrough`→(과도기: 현행처럼 commit 후) FALSE, `Passthrough`→FALSE.
- **OnKeyDown(handle_key_down)**: 동일 `classify_key` 호출로 F3(GAP2) 대체.
  OnTestKeyDown 미발화 앱에서는 여기가 첫 판정 지점이 되므로
  `CommitThenPassthrough` → `confirm_english_hold`/`commit_for_passthrough`
  후 eaten=false. **F1 과 F3 이 같은 표를 읽으므로 게이트 불일치가 구조적으로
  불가능해진다.**
- synth echo 회계(observe_test_key_down/observe_key_down Case A/B)는
  classify_key **앞단**에 그대로 둔다(합성 echo 는 사용자 키가 아니므로
  분류 대상이 아님 — 회계 로직 무수정).

### B-3. commit 시점 이동 — OnTestKeyDown 비변형 복원 (2단계)

**현행 제약(검증된 반례)**: `key_handler.rs:167` / `text_service.rs:982` 주석
— "commit 을 OnKeyDown 에서 한 뒤 FALSE 반환하면 CUAS 가 이미 test 단계에서
claim 된 키를 앱으로 흘려보내지 않는다"(Enter/화살표 먹통의 원인이었음).
즉 **navkey 는 OnTestKeyDown 에서 FALSE 를 반환해야 하고, 그 경우 OnKeyDown
자체가 호출되지 않으므로** 단순 이동은 불가능하다.

따라서 이동은 다음 방식으로만 성립한다:

- **B 안(권장, 본 사양)**: OnTestKeyDown 은 `CommitThenPassthrough` 판정 시
  **판정만 기록**(pending_commit 슬롯, 1회성 take)하고 FALSE 반환. 실제
  commit 은 같은 스레드에서 **즉시 이어지는 `WM_UNIM_COMMIT` post** 로
  wndproc 에서 수행(R6b `WM_UNIM_TAIL`·b1 `WM_UNIM_FLUSH2` 와 동일한 기존
  검증 패턴). 키는 FALSE 로 앱에 전달되고, commit 편집은 별도 메시지로
  문서에 들어간다.
  - **순서 리스크**: 앱이 키(WM_KEYDOWN)를 wndproc 메시지보다 먼저 처리하면
    "개행 → 그 뒤 한글 확정" 역전 발생 가능. TSF STA 큐 특성상
    PostMessage 가 키 전달보다 먼저 소비될 것으로 예상되나 **앱별 실측
    없이는 단정 불가** — 이것이 device-QA 필수 사유다.
- **A 안(보수, 폴백)**: commit 을 OnTestKeyDown 에 유지하되
  `commit_for_passthrough` 호출을 `apply_disposition()` 어댑터 한 곳으로
  격리. 비변형 복원은 포기하지만 3중 포크 제거 효과는 동일. **B 안 QA
  실패 시 이 상태로 릴리스한다** (1단계 산출물 = A 안, 2단계 = B 안 시도).

단계 분할:

| 단계 | 내용 | 행동 변화 |
|------|------|-----------|
| P1 | `classify_key` 신설 + F1/F3 호출 치환 + 유닛테스트(표 기반 전수) | **0 (순수 리팩터)** |
| P2 | `WM_UNIM_COMMIT` 지연 commit, config 게이트 `deferred_commit`(기본 OFF) 뒤에 구현 | OFF 시 0 |
| P3 | device-QA 통과 앱 계열에서 기본 ON 전환 검토 | QA 결과에 종속 |

### B-4. device-QA 절차 (무회귀 게이트)

P1 후와 P2(게이트 ON) 후 각각 전체 매트릭스 1회전:

| 앱 계열 | 대표 | 필수 시나리오 |
|---------|------|----------------|
| 정식 TSF | 메모장, VS Code | 조합중 Enter/Tab/Esc/방향/Home/End/PgUp·Dn → 확정+앱동작, 넘패드 숫자, Ctrl+J/Shift+Del 콤보, Shift 단독(세벌식 시프트자모 보존) |
| Blink | Chrome 주소창·textarea | 위 전부 + 펌프-분할 회귀(빠른 연타 자동교정), 첫 글자 누락 |
| CUAS/IMM32 브리지 | 카톡, 한컴, Sticky Notes | 조합중 Enter 먹통 재발 여부(핵심 반례 지점), 자동교정 |
| OnTestKeyDown 미발화 | wmux(xterm.js) | 음절모드 synth 자동교정(꼬리 종성 유실 회귀), navkey 경계확정(GAP2 동등성) |
| 터미널 | wezterm | 오버레이 preedit + navkey |
| Word | winword | word 모드 라이브 조합 + navkey/콤보 (D1 게이트 회귀) |

판정: 시나리오 전부 v0.3.54 와 동일 동작 = PASS. 하나라도 FAIL 이면 P2 는
게이트 OFF 유지(P1 만 반영), 원인을 본 문서에 추기.

### B-5. 리스크 목록

| # | 리스크 | 심각도 | 완화 |
|---|--------|--------|------|
| R1 | CUAS 키 미전달 재발(B-3 반례) | 높음 | config 게이트 기본 OFF + A 안 폴백 |
| R2 | WM_UNIM_COMMIT vs 키 전달 순서 역전 | 높음 | 매트릭스 실측, 역전 앱 발견 시 즉시-commit 예외 목록(cuas_windows 학습 재사용) |
| R3 | classify_key 흡수 과정의 순서 미묘 변화(토글 vs bare-modifier 우선순위 등) | 중간 | P1 을 "표 그대로 이식 + 표 기반 유닛테스트 전수(현행 F1 순서 = 기대값)"로 고정 |
| R4 | english_hold 스냅샷 시점 차(락 순서 engine→composition 준수) | 중간 | KeySnapshot 을 기존 락 취득 지점에서 1회 구성, classify_key 내부 락 금지 규약 |
| R5 | wmux Case B 회계와의 상호작용 | 낮음 | synth 회계는 classify 앞단 유지(무수정) — 접점 없음 |

### B-6. 수용 기준 (B)

- [ ] `classify_key` 유닛테스트: F1 현행 게이트 순서를 표로 옮긴 전수 케이스
      (repeat/bare-mod/combo/navkey/numpad/eng-hold-BS/토글/팝업/한자/문자키,
      제네릭 VK 0x10-0x12 포함) all-pass.
- [ ] F1 인라인 게이트와 F3(GAP2) 블록이 삭제되고 호출이 classify_key
      1곳으로 수렴 — `text_service.rs` OnTestKeyDown 에 disposition match 외
      게이트 분기 잔존 0.
- [ ] P1 상태에서 B-4 매트릭스 PASS (동작 변화 0 확인).
- [ ] `deferred_commit` OFF 시 바이트 수준 동일 경로(로그 diff 로 확인).
- [ ] ON 시 CUAS 계열 Enter/화살표 정상 + 확정 순서 역전 0 — 실패 시 OFF
      릴리스 결정 기록.
- [ ] Windows 크레이트 zero-warning, `cargo test -p unim` all-pass.

---

## 착수 순서 제안

1. **B-P1** (순수 리팩터, 회귀면 최소) → device-QA 1회전.
2. **A** 스키마+코어+CLI (사용자 게이트 G1–G5 승인 후) → GUI → 동기화.
3. **B-P2/P3** (deferred commit) — A 와 독립, QA 리소스 확보 시.
