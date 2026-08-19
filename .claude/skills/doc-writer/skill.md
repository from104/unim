---
name: doc-writer
description: UNIM 사용자/엔드유저 문서 + GUI 라이브 도움말 작성. README/사용자 매뉴얼/트러블슈팅/FAQ를 한국어/영어 양쪽으로 작성하고, GTK/Qt 설정 GUI의 모든 위젯에 툴팁·subtitle·설명을 추가. 과도하게 친절한 톤. "사용자 문서", "릴리즈 노트", "라이브 도움말", "툴팁 추가", "트러블슈팅", "FAQ 작성" 요청 시 반드시 트리거.
---

# Doc Writer — 과도하게 친절한 문서화

## 톤 가이드
- **과도한 친절**: "이 옵션은 X 합니다" 수준 금지. "X 한다는 건 Y 환경에서 Z 효과. 보통은 ON으로 두세요" 수준 권장.
- **예시 풍부**: 모든 추상 설명에 구체 예시 1개 이상.
- **링크 풍부**: 관련 옵션끼리 상호 참조.
- **약어 풀이**: IME, IM 모듈, DBus, XIM, TSF 등 첫 등장 시 풀이.
- **실행 가능 코드만**: 의사코드 금지.

## 산출물 6종

### 1. 사용자 매뉴얼
`docs/user/user-guide/{README.md, README-ko.md}`
- 무엇/왜/누가
- 5분 빠른 시작 (Ubuntu/Arch/Debian)
- 환경별 설정 (X11/Wayland × GNOME/KDE/Sway)
- 일상 사용 (한/영 토글, 한자, 특수문자, 이모지, AutoTypeFix)
- 설정 GUI 투어 (스크린샷 placeholder `<!-- screenshot: ... -->`)
- 키 매핑 치트시트 (표)
- CLI 사용법 전 서브커맨드

### 2. 라이브 도움말 (위젯별)
GTK 설정 다이얼로그 — 각 위젯에:
```rust
let row = adw::SwitchRow::builder()
    .title(t!("settings_autotypefix_title"))
    .subtitle(t!("settings_autotypefix_subtitle"))  // ← 라이브 도움말
    .build();
row.set_tooltip_text(Some(&t!("settings_autotypefix_tooltip")));
```
페이지·그룹에도 1-2줄 안내. Qt는 `ToolTip { text: qsTr(...) }`. GNOME prefs는 `Adw.ActionRow.subtitle`.

### 3. 트러블슈팅
`docs/user/troubleshooting/{README.md, README-ko.md}`
증상별 진단 트리:
| 증상 | 1차 진단 | 2차 명령 |
|------|---------|---------|
| 한글 안 뜸 | IM 모듈 등록 확인 | `gsettings get ...`, `gnome-extensions list` |
| 한자 popup 안 뜸 | popup_mode 설정 | `unim-cli config show popup_mode` |
| 설정 저장 안 됨 | 권한/경로 | `ls -la ~/.config/unim/` |
| GNOME ext 안 보임 | 설치 확인 | `gnome-extensions enable unim-gnome@from104.github.io` |
| 키 잠김 | 로그 | `journalctl --user -u unim -f` |

### 4. FAQ
`docs/user/faq/{README.md, README-ko.md}`
- 다른 IME(ibus-hangul, fcitx, kime, nimf)와 차이
- 동시 설치 가능?
- 어떤 환경이 가장 안정?
- AutoTypeFix 동작 원리
- 한자 9칸 vs 81칸
- 설정 백업/복원

### 5. 릴리즈 노트
`CHANGELOG.md` / `CHANGELOG-ko.md` — 별도 릴리즈 노트 문서는 두지 않는다.
GitHub 릴리스 본문은 `scripts/release-body.sh` 가 이 두 파일에서 뽑는다.
- 사용자 가시 변경 highlight (`### 수정됨` / `### 추가됨` / `### 변경됨`)
- Breaking changes (없으면 명시)
- 마이그레이션 안내는 해당 항목의 하위 불릿으로
- 남은 제약은 `### 알려진 문제` 절에 — 릴리스 본문 맨 위로 올라간다
- 알려진 이슈

### 6. 루트 README 정리
`README.md` 갱신:
- 0.2.0 배지
- 1줄 요약 + 스크린샷
- 5단계 빠른 시작
- 위 docs 링크

## 라이브 도움말 추출 절차
1. `unim-gui-gtk/src/settings_dialog.rs`에서 모든 위젯 목록화
2. 각 위젯의 i18n 키 명명 규칙(`settings_<group>_<name>_<role>`) 적용
3. title / subtitle / tooltip 3종 키 등록
4. ko.yml/en.yml에 텍스트 추가 (i18n-applier와 협업)
5. `make build` warning 0 유지

## 이중언어 작성 패턴
- 한국어 우선 작성 (사용자 모국어, 의미 정확도 ↑)
- 영어 짝은 동일 구조·동일 깊이로 번역
- 코드블록·명령은 양쪽 동일 (번역 안 함)
- 파일명 컨벤션: 영어 `README.md`, 한국어 `README-ko.md`

## 검증
- 깨진 링크 없는지: `grep -rn '](\\..*\\.md)' docs/`
- 한/영 짝 누락 없는지
- 코드블록 명령 실행 가능: 적어도 `--help` 정도는 직접 실행 검증
- 매뉴얼 따라하면 실제로 동작하는지 (선행 시나리오 일치)

## 출력 보고서
`_workspace/release/03_doc_report.md`:
- 신규/갱신 문서 목록 (단어 수)
- 라이브 도움말 추가된 위젯 수 / 전체 위젯 수
- 깨진 링크/누락 짝
- 사용자 판단 필요 영역
