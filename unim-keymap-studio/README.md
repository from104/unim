# UNIM 키맵 스튜디오 (unim-keymap-studio)

UNIM 입력기의 한·영 키맵을 보고 편집하는 GTK4 도구. 빌트인 자판(두벌식·세벌식·QWERTY·Dvorak·Colemak·Workman 등)을 출발점으로 키 배열·자모 조합·규칙 세트·키 메타데이터를 편집하고, 사용자 자판을 `~/.config/unim/layouts/`에 저장한다. 저장 후에는 UNIM 데몬이 mtime 변화를 감지해 자동 재로드한다.

## 주요 기능

- **헤더 3단 드롭다운** — 언어(한글/영문) > 출처(빌트인/사용자) > 자판명 순으로 선택. 출처가 한눈에 보여 빌트인과 사용자 자판을 혼동하지 않는다.
- **4개 편집 탭** — 기본 / 자판 / 조합 / 확장. "조합"·"확장"은 한글 자판일 때만 노출되어, 한 번에 한 가지만 다루도록 구성.
- **빌트인 보호 정책** — 빌트인 자판은 자유롭게 편집하되 "저장"은 비활성, "다른 이름으로 저장"만 가능. 사용자 자판은 "저장"·"다른 이름으로 저장" 모두 가능.
- **클릭 편집 키보드** — typing-practice와 동일한 5행 stagger 키보드. 키 셀을 클릭하면 위쪽(Shift)/아래쪽(평문) 한 글자씩 편집. 한글 자판이면 초·중·종성 드롭다운 헬퍼 제공(종성은 첫가끝 자모 표시).
- **자모 조합 편집** — 같은 스코프끼리 1+1=1 조합(예: ㄱ+ㄱ→ㄲ)을 드롭다운으로 추가·편집·삭제.
- **규칙 세트 & 키 메타데이터** — rule_sets 활성 토글, 세트별/전역 key_meta(vowel_combine_head, context_alt 9 변이) 편집.
- **미저장 변경 가드** — 다른 자판으로 이동하거나 새로 만들 때 미저장 변경이 있으면 확인 다이얼로그로 보호.

## 빌드와 실행

```bash
# 워크스페이스 루트(unim/)에서:
cargo run -p unim-keymap-studio           # 디버그
cargo run --release -p unim-keymap-studio # 릴리스

# 또는 빌드 후 직접 실행:
cargo build --release -p unim-keymap-studio
./target/release/unim-keymap-studio
```

### 사전 요구

- GTK 4.10 이상 (FileDialog 사용)
- libadwaita 1.4 이상
- Rust 1.95+

## 사용법

1. 헤더 좌측 드롭다운에서 언어 > 출처 > 자판을 선택
2. 탭을 옮겨 가며 편집:
   - **기본** — 언어·자판 유형·식별자·표시 이름(ko/en)·상속·메타데이터·태그, 한글이면 모아치기(v3) 지원 토글
   - **자판** — 키 셀 클릭 → 편집 다이얼로그(무조건 한 글자, 한글이면 초·중·종성 드롭다운)
   - **조합** — 초/중/종성 자모 조합 CRUD
   - **확장** — 규칙 세트와 키 메타데이터
3. 사용자 자판이면 `Ctrl+S`로 저장, 빌트인이거나 새 이름이 필요하면 `Ctrl+Shift+S`로 "다른 이름으로 저장"

### 단축키

| 키 | 동작 |
|---|---|
| `F1` | 사용법·단축키 도움말 |
| `Ctrl + N` | 새 자판 만들기 |
| `Ctrl + D` | 현재 자판 복제 |
| `Ctrl + S` | 저장 (빌트인은 비활성) |
| `Ctrl + Shift + S` | 다른 이름으로 저장 |
| `Ctrl + E` | JSON 내보내기 |
| `Ctrl + I` | JSON 가져오기 |
| `Ctrl + 1 / 2 / 3 / 4` | 기본 / 자판 / 조합 / 확장 탭 |

### 헤더 메뉴 (☰)

새 자판 만들기 / 현재 자판 복제 / JSON 내보내기 / JSON 가져오기 / 사용자 자판 폴더 열기 / 원본으로 복원.

## 저장 규칙

- 저장 경로는 `~/.config/unim/layouts/<name>.json` 단일 경로
- 식별자는 영숫자·`_`·`-`만 허용
- 빌트인 이름과 충돌 불가(덮어쓰기 금지). 사용자 폴더는 현재 편집 중인 자판과 같은 이름일 때만 덮어쓰기 허용
- 영문 자판으로 전환하면 한글 전용 필드(combinations·rule_sets·key_meta·moachigi)는 자동으로 비워진다

## 아키텍처

```
state/
  app_state.rs       AppState — registry·editor·is_builtin·toast·refresh/language 콜백
  editor_state.rs    EditorState — 복사 후 편집(copy-on-edit), 메타·조합·rule_set·key_meta·save/save_as
header/
  profile_dropdown.rs  3단 드롭다운 (MenuButton + Popover + 중첩 ExpanderRow)
  menu_actions.rs      ☰ 메뉴 액션 6종 + gio::Menu 모델
tabs/
  tab_basic.rs       언어·메타·모아치기
  tab_keymap.rs      5행 키보드 + 키 편집 위임
  tab_combos.rs      cho/jung/jong 조합 CRUD
  tab_extended.rs    rule_sets + key_meta
dialogs/
  key_edit / combo_edit / rule_set_edit / key_meta_edit
  new_profile / duplicate_profile / save_as / import_export / help
widgets/studio_keyboard.rs  클릭 가능한 5행 stagger 키보드
helpers/
  jamo_catalog.rs    초(19)/중(21)/종(27) 자모 카탈로그 — 종성은 첫가끝 자모
  name_validator.rs  이름 충돌·정규식 검증
```

- `editor_state`는 GTK 의존이 없어 단위 테스트 가능
- 자모 조합 규약: 초·중성은 호환 자모(`ㄱ`/`ㅗ`), 종성은 첫가끝 자모(`ᆨ`)
- 저장 후 별도 알림 불필요 — UNIM 데몬의 `ProfileRegistry::reload_if_changed`가 mtime 변화를 감지해 자동 재로드

## 라이선스

UNIM 워크스페이스의 라이선스를 따른다 — 루트의 [LICENSE](../LICENSE) 참조.

## 관련 문서

- [UNIM 메인 README](../README.md) — 입력기 본체
- [UNIM 타자 연습](../unim-typing-practice/README.md) — 같은 시각 언어를 쓰는 자매 도구
