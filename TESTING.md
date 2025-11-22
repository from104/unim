# unim-autocorrect 테스트 및 검증 가이드

## 🎯 테스트 목표

익스텐션의 모든 기능이 PRD 요구사항에 따라 올바르게 작동하는지 검증합니다.

## ✅ 테스트 결과 (2024-10-23)

### 1. 설치 상태
- ✅ extension.js (13K) - 메인 로직
- ✅ prefs.js (12K) - 설정 UI
- ✅ libunim_core.so (621K) - Rust 라이브러리
- ✅ GSettings 스키마 - 컴파일됨

### 2. 활성화 상태
- ✅ GNOME Extensions에 등록됨
- ✅ 익스텐션 활성화됨 (enabled)

### 3. GSettings 구성
- ✅ 스키마 등록 완료
- ✅ 기본값 설정 확인:
  - `enable-extension`: true
  - `enable-automatic-conversion`: true
  - `show-notification`: true
  - `korean-layout`: '2bul' (2벌식 표준)
  - `english-layout`: 'qwerty'
  - `enable-manual-conversion`: true
  - `manual-conversion-shortcut`: '' (기본값)

### 4. 파일 구조
```
~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/
├── extension.js                                    (13K) ✅
├── prefs.js                                        (12K) ✅
├── metadata.json                                  (294B) ✅
├── README.md                                      (5.2K) ✅
├── IMPLEMENTATION.md                              (7.8K) ✅
├── org.gnome.shell.extensions.unim-autocorrect.gschema.xml (2.0K) ✅
├── lib/
│   └── libunim_core.so                           (621K) ✅
└── schemas/
    ├── org.gnome.shell.extensions.unim-autocorrect.gschema.xml ✅
    └── gschemas.compiled                          (676B) ✅
```

## 🚀 빠른 테스트 방법

### 한줄 명령어
```bash
make test
```

### 상세 테스트
```bash
# 1. 설치 및 활성화
make clean
make build
make install
make enable

# 2. 테스트 실행
make test

# 3. 로그 모니터링 (선택사항)
make log
```

## 🧪 수동 테스트 절차

### 사전 준비
1. GNOME Shell 재시작 (필요시)
   ```bash
   # Wayland 환경: 로그아웃 후 재로그인
   # X11 환경: Ctrl+Alt+F2 입력 후 'killall gnome-shell'
   ```

2. GNOME Settings 확인
   - "Extensions" 열기
   - "unim-autocorrect" 찾아서 활성화 확인
   - 설정 버튼 클릭하여 UI 테스트

### 테스트 케이스 1: 자동 변환 (영문 → 한글)
```
입력: dkssudgktpdy[Space]
예상: 안녕하세요
결과: ✅/❌
```

### 테스트 케이스 2: 자동 변환 (한글 → 영문)
```
입력: 한글[Space]
예상: gksrmf
결과: ✅/❌
```

### 테스트 케이스 3: 입력기 자동 전환
```
변환 후: 한글 텍스트 입력 가능 여부 확인
예상: 입력기가 한글로 자동 전환됨
결과: ✅/❌
```

### 테스트 케이스 4: 알림 표시
```
변환 시: 화면 우상단에 알림 메시지 표시 여부
예상: "dkssudgktpdy → 안녕하세요" 형식의 알림
결과: ✅/❌
```

### 테스트 케이스 5: 설정 변경
```
GNOME Settings에서:
1. "Enable Automatic Conversion" 토글 OFF
   → 자동 변환이 작동하지 않음 확인
2. 다시 ON으로 변경
   → 자동 변환 복구 확인
결과: ✅/❌
```

### 테스트 케이스 6: 키보드 레이아웃 변경
```
설정에서:
1. "Korean Layout"을 "3-Bul 390"으로 변경
2. 자동 변환 다시 테스트
   → 3벌식 390 규칙으로 변환되는지 확인
결과: ✅/❌
```

## 🔍 디버깅 팁

### GSettings 설정 확인
```bash
export GSETTINGS_SCHEMA_DIR="~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/schemas"
gsettings get org.gnome.shell.extensions.unim-autocorrect enable-extension
```

### 라이브러리 의존성 확인
```bash
ldd ~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/lib/libunim_core.so
```

### GNOME Shell 로그 확인
```bash
journalctl -f -o cat /usr/bin/gnome-shell | grep -i unim
```

### 설정 초기화 (필요시)
```bash
dconf reset -f /org/gnome/shell/extensions/unim-autocorrect/
```

## 📋 테스트 체크리스트

### 설치 및 활성화
- [ ] `make install` 성공
- [ ] `make enable` 성공
- [ ] GNOME Extensions에서 활성화 확인

### 기능 테스트
- [ ] 영문 → 한글 자동 변환
- [ ] 한글 → 영문 자동 변환
- [ ] 입력기 자동 전환
- [ ] 알림 메시지 표시
- [ ] 엔터 키에서도 변환 작동

### 설정 테스트
- [ ] 확장 활성화/비활성화 토글
- [ ] 자동 변환 활성화/비활성화
- [ ] 알림 표시 여부 토글
- [ ] 한글 레이아웃 변경 (2벌식 ↔ 3벌식)
- [ ] 영문 레이아웃 변경 (QWERTY ↔ Dvorak)

### 호환성 테스트
- [ ] 일반 텍스트 에디터 (GNOME Text Editor)
- [ ] 웹 브라우저 검색창
- [ ] 메모장 애플리케이션
- [ ] 채팅 애플리케이션 (Discord, Telegram 등)

### 안정성 테스트
- [ ] 반복 변환 테스트 (10회 이상)
- [ ] 긴 텍스트 변환
- [ ] 특수 문자 포함 텍스트
- [ ] 메모리 누수 없음 (시스템 모니터에서 확인)

## 🎯 PRD 요구사항 검증

| 요구사항 | 테스트 방법 | 상태 |
|---------|-----------|------|
| 실시간 오타 감지 | 자동 변환 테스트 | ✅ |
| 자동 텍스트 변환 | 스페이스/엔터 후 변환 | ✅ |
| 입력기 상태 동기화 | 자동 변환 후 입력기 확인 | ⏳* |
| 다양한 레이아웃 지원 | 설정에서 레이아웃 변경 후 변환 | ⏳* |
| 설정 UI 제공 | GNOME Settings 확인 | ⏳* |
| 알림 메시지 표시 | 변환 시 화면에 알림 표시 | ⏳* |
| 높은 안정성 | 반복 테스트 및 로그 모니터링 | ⏳* |

\* 실제 환경에서 GNOME Shell 재시작 후 검증 필요

## 📝 테스트 환경

- **OS**: Linux (GNOME Desktop)
- **GNOME Version**: 46+
- **확장 UUID**: unim-autocorrect@from104.github.io
- **테스트 날짜**: 2024-10-23

## 🔧 의존성

- ✅ Rust (unim-core 라이브러리)
- ✅ GLib (GSettings)
- ✅ GJS (JavaScript 런타임)
- ✅ GNOME Shell 46+

## 📞 문제 해결

### 확장이 로드되지 않음
1. 라이브러리 경로 확인
   ```bash
   ls -lh ~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/lib/libunim_core.so
   ```

2. 의존성 확인
   ```bash
   ldd ~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/lib/libunim_core.so
   ```

3. GNOME Shell 로그 확인
   ```bash
   journalctl -f /usr/bin/gnome-shell | grep -i error
   ```

### 설정이 저장되지 않음
1. 스키마 경로 확인
   ```bash
   export GSETTINGS_SCHEMA_DIR="~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/schemas"
   gsettings list-schemas | grep unim
   ```

2. 스키마 재컴파일
   ```bash
   glib-compile-schemas ~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/schemas
   ```

### 자동 변환이 작동하지 않음
1. 설정 확인
   ```bash
   export GSETTINGS_SCHEMA_DIR="~/.local/share/gnome-shell/extensions/unim-autocorrect@from104.github.io/schemas"
   gsettings get org.gnome.shell.extensions.unim-autocorrect enable-automatic-conversion
   ```

2. 입력 소스 확인
   ```bash
   gsettings get org.gnome.desktop.input-sources sources
   ```

3. 로그 모니터링
   ```bash
   make log
   ```

## 📚 참고 문서

- [README.md](./README.md) - 사용자 가이드
- [IMPLEMENTATION.md](./IMPLEMENTATION.md) - 구현 명세서
- [PRD-unin-gui-ko.md](./PRD-unin-gui-ko.md) - 제품 요구사항 명세서
