# UNIM 타자 연습 (unim-typing-practice)

UNIM 입력기와 함께 동작하는 GTK4 한·영 타자 연습 도구. 실제 사용자의 자판(두벌식·세벌식·Workman·Dvorak 등) 그대로 측정하고, 한글 음절을 자모 키스트로크 단위로 분해해 WPM/CPM/오타 분포를 추적한다.

## 주요 기능

- **자판 자동 인식** — UNIM 데몬에서 현재 활성 한·영 키맵을 읽어 동일 layout으로 연습. 설정에서 자판을 바꾸면 2초 내 키보드 그림·라벨·세션이 자동 갱신.
- **단어 단위 greedy 매칭** — 한 단어가 어긋나도 다음 단어 진행에 영향 없음. 누락·치환 글자는 색상으로 회고 표시.
- **분당 타수(CPM)** — 한글 음절은 자모 키스트로크 수(예: '정' = ㅈ+ㅓ+ㅇ = 3타)로 환산하여 한국 표준 측정 방식과 일치. WPM = CPM ÷ 5.
- **오타 히트맵** — 음절을 키스트로크로 분해한 셀별 오타 빈도를 5단계 색으로 시각화. Practice 탭과 동일 디자인의 키보드 위젯에 표시.
- **사용자 정의 지문** — 파일/클립보드에서 가져와 `$XDG_CONFIG_HOME/unim-typing-practice/corpora/*.txt`에 저장. 2000 byte 상한. 드롭다운 호버 시 이름 변경·삭제 아이콘.
- **자동 진행** — Space/Enter/Tab으로 강제 이동, 단어 수만 채우면 typos에도 자동 진행.

## 빌드와 실행

```bash
# 워크스페이스 루트(unim/)에서:
cargo run -p unim-typing-practice           # 디버그
cargo run --release -p unim-typing-practice # 릴리스

# 또는 빌드 후 직접 실행:
cargo build --release -p unim-typing-practice
./target/release/unim-typing-practice
```

### 사전 요구

- GTK 4.10 이상 (FileDialog 사용)
- libadwaita 1.4 이상
- UNIM 데몬(`unim-daemon`) 실행 중 — 미실행 시 3-step 안내 다이얼로그만 띄우고 종료
- Rust 1.95+

## 사용법

1. UNIM 트레이에서 입력기를 활성화한 뒤 본 앱 실행
2. 헤더 좌측 드롭다운에서 지문 선택 → 입력 필드 자동 포커스
3. 한 줄씩 따라 입력. 각 줄 끝에서 Space/Enter/Tab으로 다음 줄, 모든 줄 완료 시 결과 화면 자동 전환
4. 결과 화면에서 WPM/CPM/정확도/오타율과 키별 히트맵 확인 → "다시 시작" 또는 코퍼스 변경 후 재진행

### 단축키

| 키 | 동작 |
|---|---|
| `F1` | 사용법·단축키 도움말 |
| `Ctrl + R` | 처음부터 다시 시작 |
| `Ctrl + Shift + C` | 결과 클립보드 복사 |
| `Ctrl + 1` | 연습 화면 |
| `Ctrl + 2` | 결과 화면 |
| `Ctrl + O` | 지문 파일 가져오기 |
| `Ctrl + Shift + V` | 클립보드에서 지문 가져오기 |

### 사용자 정의 지문

- **메뉴 → "지문 파일 가져오기…"** 또는 `Ctrl+O` — `.txt` 선택, 줄바꿈(`\r\n`/`\r`/`\n`) 자동 정규화 후 저장
- **메뉴 → "클립보드에서 가져오기"** 또는 `Ctrl+Shift+V` — 클립보드 텍스트로 즉석 corpus 생성 (`clipboard_YYYYMMDD_HHMMSS`)
- **드롭다운 호버** — 사용자 지문 행 우측에 ✏ (이름 변경) / 🗑 (삭제) 아이콘 노출. 빌트인 지문은 보호되어 아이콘 없음
- **2000 byte 상한** — 초과 시 토스트로 거부. 이름은 영숫자·한글·`_`·`-`만 허용 (공백은 `_`로 치환)

### 입력 필드 정책

- **붙여넣기 차단** — `Ctrl+V`, 우클릭 → 붙여넣기, 중간 클릭(primary selection) 모두 차단. 직접 타이핑한 키만 측정에 반영
- **IME 합성 그대로 측정** — UNIM이 OS 수준에서 한글 조합·preedit·백스페이스를 처리하므로 본 앱은 통계만 담당

## 아키텍처

```
practice_engine.rs   순수 통계 코어 — greedy 매칭, 음절→키스트로크 분해, WPM/CPM
keyboard_view.rs     5행 stagger 키보드 위젯 (한·영 라벨 동적, 히트맵 5단계)
practice_page.rs     GTK4 UI — Practice/Result 4×10 그리드, slot 패턴 자판 reload
active_layout.rs     UNIM DBus로 활성 한·영 자판 읽기 (config fallback)
corpus.rs            빌트인 + 사용자 정의 지문 (저장/로드/이름변경/삭제)
app.rs               메인 윈도우 + 통합 CSS + 데몬 비활성 안내
```

- `practice_engine`은 GTK 의존이 없어 단위 테스트 가능 (28 tests)
- 한글 음절 → ASCII 키스트로크 변환은 `unim::typefix::kor_to_eng` 재사용
- 자판 변경 감지는 2초 polling (`korean.layout` / `english.layout` DBus key 비교)

## 라이선스

UNIM 워크스페이스의 라이선스를 따른다 — 루트의 [LICENSE](../LICENSE) 참조.

## 관련 문서

- [DESIGN.md](DESIGN.md) — UI/UX 시안과 토큰
- [docs/design-brief.md](docs/design-brief.md) — 디자인 명세
- [UNIM 메인 README](../README.md) — 입력기 본체
