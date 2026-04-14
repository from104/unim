# GTK UI 시각 QA 레퍼런스

## 샌드박스 실행

```bash
make sandbox-gtk4
```

Xephyr 창이 뜨면서 격리된 X 디스플레이에서 GTK4 앱을 실행. 시스템 전체에 영향 주지 않고 UI 확인 가능.

## 설정 다이얼로그 단독 실행

```bash
# 개발 빌드
target/debug/unim-gui-gtk --settings

# 설치본
unim-gui-gtk --settings
```

## 시각 체크리스트 (Phase 3 완료 시)

### 레이아웃
- [ ] 창 크기 520x640 이상에서 스크롤 불필요
- [ ] 3개 페이지 탭이 상단에 모두 표시 (GNOME 세션 시)
- [ ] 비-GNOME 세션에서 "GNOME Shell" 페이지 숨김
- [ ] PreferencesGroup 간 여백 균일

### 위젯 동작
- [ ] SpinRow "임계 음절 수": 2↔6 범위, step 1
- [ ] SpinRow "임계 글자 수": 3↔8 범위, step 1
- [ ] SpinRow "트리거 윈도우": 0.5↔5.0, step 0.5, suffix "초"
- [ ] SwitchRow 변경 즉시 `~/.config/unim/config.yaml`에 반영
- [ ] 값 변경 시 "저장됨 ✓" Toast 2초 표시
- [ ] ComboRow 드롭다운 선택 → 즉시 반영
- [ ] EntryRow 쉼표 구분 입력 → 공백 trim + 빈 토큰 제거

### 테마
- [ ] 시스템 라이트 모드 → 다이얼로그 라이트
- [ ] 시스템 다크 모드 → 다이얼로그 다크
- [ ] 테마 전환 시 다이얼로그 재시작 없이 즉시 반영 (GNOME Settings에서 테마 변경)

### DBus 전파
- [ ] GTK에서 한글 자판 변경 → 다른 터미널에서 `busctl monitor`로 ConfigChanged 수신
- [ ] GNOME extension indicator 모드 표시 즉시 변경

## 스크린샷 권장 영역

보고서(phase3_gtk_designer.md)에 아래 4장 첨부(설명이나 ASCII도 가능):
1. "일반" 페이지 전체
2. "오타 교정" 페이지 전체 (순방향/역방향 그룹)
3. "GNOME Shell" 페이지
4. 라이트/다크 테마 비교

## 흔한 시각 버그

| 증상 | 원인 | 수정 |
|------|------|------|
| ActionRow 높이가 비정상적으로 큼 | title/subtitle 외에 child widget을 `add_suffix` 대신 `add_prefix`+spacing | 표준 패턴 복귀 |
| SpinRow가 adw 0.7에 없음 | SpinRow는 libadwaita 1.4+. adw crate 0.7이 1.2 바인딩이면 ActionRow + SpinButton 조합으로 대체 | Cargo.toml 버전 확인 후 결정 |
| ForceDark 잔존으로 라이트 테마에서 검게 나옴 | gtk_ui.rs의 전역 style_manager 설정 | 다이얼로그 로컬 style_manager 또는 전역 제거 |
| Toast가 사라지지 않음 | timeout 0 또는 default | `Toast::set_timeout(2)` 명시 |
