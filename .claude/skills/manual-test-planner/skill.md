---
name: manual-test-planner
description: UNIM 0.2.0 수동·자동 테스트 시나리오 설계. 13개 컴포넌트(daemon/cli/dbus/gui-gtk/gui-qt/im-gtk3/im-gtk4/im-qt5/im-qt6/im-xim/im-wayland/windows-tsf/gnome-extension)별 골든패스+엣지케이스+회귀 시나리오를 사용자가 따라할 수 있는 체크리스트로 도출. "테스트 계획", "수동 테스트", "릴리즈 점검 시나리오", "기능 검증 가이드" 요청 시 반드시 트리거.
---

# Manual Test Planner — 시나리오 설계 패턴

## 시나리오 카테고리 (6종)
1. **골든패스**: 가장 일반적인 사용자 플로우 (한국어 입력, 한자 변환, 영어 토글)
2. **AutoTypeFix**: 한영 오타 교정 정/역방향, 환경별
3. **팝업**: Standalone/Embedded 모드, 한자 grid 토글, 북마크, 이모지/특수문자
4. **설정 GUI**: GTK/Qt 모든 위젯 탐방, CLI config 모든 키, GNOME prefs
5. **회귀**: 기존 해결 버그 재발 방지 (preedit-end, DBus 재진입, N+1 BS XIM 등)
6. **환경 매트릭스**: X11/Wayland × GNOME/KDE 4조합

## 작성 양식 (체크리스트)

각 시나리오는 다음 구조:
```markdown
### [컴포넌트] 시나리오 제목 — 예상 시간 (1분/5분/10분+)
**선행조건**: <환경/패키지/설정>
**절차**:
1. 명령 또는 키 시퀀스 (구체적으로)
2. ...
**기대 결과**: <확인할 것>
**실패 시 진단**: <로그 위치, 디버깅 명령>
- [ ] PASS / FAIL 체크박스
```

## 컴포넌트별 핵심 검증 포인트

| 컴포넌트 | 핵심 검증 |
|----------|----------|
| unim-daemon | systemd start/restart, 비정상 종료 후 자동 재시작, journalctl 깔끔함 |
| unim-cli | 모든 서브커맨드(--help, convert, config show/set/path/reset), locale 자동 감지 |
| unim-dbus | unim-test-dbus all-pass, busctl introspect 정상 |
| unim-gui-gtk | 시작/종료, 다크/라이트 자동, 모든 SwitchRow/SpinRow/ComboRow 변경 즉시 저장 |
| unim-gui-qt | QML 페이지 이동, 동일 설정 변경 즉시 저장 |
| im-gtk3/4 | gedit/gtk*-demo에서 한국어 입력, preedit-end 누락 없음 |
| im-qt5/6 | qtwidgets demo, popup 위치, 한자 popup |
| im-xim | xterm/Emacs/터미널 입력, AutoTypeFix N+1 BS |
| im-wayland | weston-text-input-demo, focus 변경 |
| gnome-extension | GNOME Shell 트레이, prefs 5개 옵션, 한자 popup Push 모드 |
| windows-tsf | 크로스컴파일 check, Windows 설치 후 메모장 입력 (옵션) |

## 자동화 가능성 매핑

| 시나리오 카테고리 | 자동화 비율 | 자동화 도구 |
|------------------|-------------|-------------|
| 골든패스 | 30% | unim-test-dbus, cargo test |
| AutoTypeFix | 60% | unim-typefix-engine cargo test |
| 팝업 | 20% | DBus signal 검증만 |
| 설정 GUI | 5% | i18n 키 검증만 |
| 회귀 | 70% | 기존 테스트 케이스 |
| 환경 매트릭스 | 0% | 수동만 |

## 출력
- `docs/release/0.2.0/TEST_CHECKLIST.md` — 사용자용 체크리스트 (한국어)
- `docs/release/0.2.0/TEST_CHECKLIST-en.md` (영어 짝)
- `docs/release/0.2.0/TEST_AUTOMATION.md` — 자동 커버리지 매핑
- 트러블슈팅 초안: 사용자 README(`docs/user/troubleshooting/README{,-ko}.md`)의 "## 0.2.0 릴리스 특이 진단" 섹션에 흡수 (doc-writer가 확장)

## 작성 원칙
- 모든 명령은 실행 가능한 형태 (의사코드 금지)
- 키 시퀀스는 `[Shift]+[Space]` 같이 명시
- 사용자가 따라하다 막히면 진단 명령으로 즉시 복구할 수 있도록
- 시간 예측은 보수적 (5분 ≈ 평소 3분 작업)
