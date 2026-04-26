---
name: engine-frontend-manager
description: UNIM 엔진/DBus/IM 모듈/입력 로직/설정 코어 작업 패턴. 한글 조합·한자·AutoTypeFix·팝업 상태머신·DBus 시그널·환경별 분기(GTK3/4·Qt5/6·XIM·Wayland·GNOME·Windows). LSP 우선 분석·검증 사다리(L1~L4)·5지점 동기화 의뢰·메모리 안전 규칙. "엔진 변경", "DBus", "IM 모듈", "한글/한자/AutoTypeFix", "config 추가" 트리거.
---

# Engine & Frontend Operating Pattern

## 변경 사다리

### L1 — 빠른 피드백
```
PATH=$HOME/.cargo/bin:$PATH
cargo build -p <crate>
cargo test -p <crate>
```

### L2 — 워크스페이스
```
cargo build --workspace --release  # warning 0
cargo test --workspace               # all pass
```

### L3 — make build (C/C++ 프런트엔드 포함)
```
make build
```

### L4 — 설치 + 샌드박스
```
make install   # sudo
make sandbox-{gtk3,gtk4,qt5,qt6,xim,indicator}
```

## 분석 도구
- **LSP 우선** (메모리: feedback_prefer_lsp): 심볼·참조·호출 관계
- **grep**: 문자열·주석 전용

## 5지점 동기화 (config 변경 시)
| 지점 | 파일 | 담당 |
|------|------|------|
| 코어 | `src/config.rs` | engine-frontend-manager |
| CLI | `unim-cli/src/main.rs` ConfigKey | engine-frontend-manager |
| DBus | `unim-dbus/src/service.rs` get/set_config | engine-frontend-manager |
| GUI | `unim-gui-{gtk,qt}/src/settings_dialog.rs` 등 | ui-manager |
| GNOME | `unim-gnome-extension/prefs.js` 또는 gschema | ui-manager |

`settings-sync-check` 에이전트 호출로 정합성 검증 (PM 협업).

## 환경 매트릭스 영향 체크
변경마다 다음 매트릭스 영향 확인:
- X11 / Wayland
- GTK3 / GTK4 / Qt5 / Qt6 / XIM / Wayland-native / GNOME / Windows

## 디버깅 (메모리: feedback_debug_methodology)
1. **단순한 것 먼저**: 파일명·경로·권한·gitignore
2. **로그 활성화**: `UNIM_DEVELOP=1 unim-daemon`
3. **systemd**: `journalctl --user -u unim -f`
4. **DBus introspect**: `busctl --user introspect org.unim.InputMethod /org/unim/InputMethod`
5. **proc 진단** (메모리 의심 시):
   ```
   grep -E 'VmRSS|VmData|Threads' /proc/$(pidof unim-daemon)/status
   ```

## 메모리·안전 규칙 (Zero Tolerance)
- 디버그 메시지는 `unim_log!()` (println/eprintln 금지)
- TypeFIX에 클립보드 백업/복원 금지 (메모리: feedback_no_clipboard_typefix)
- DBus call_sync 재진입 시 키 큐 패턴 (메모리: feedback_dbus_call_sync)
- POPUP_SPEC.md 명세 변경은 사용자 승인 (메모리: feedback_popup_spec_absolute)
- `unim_emit_preedit` 헬퍼로 preedit-end 누락 방지 (메모리: project_preedit_end_lock)

## 프런트엔드별 빌드 산출물 검증 (메모리: feedback_verify_install_target)
설치 시 대상 디렉토리의 실제 파일명 확인 후 진행:
- GTK3: `im-*.so`
- GTK4: `libim-*.so`

## 출력 양식
```markdown
## Engine & Frontend Manager Report — {ID}

### 변경 요약
- 파일: ...
- 영향 컴포넌트: ...
- 5지점 동기화: yes/no, 누락: ...

### 검증
- L<N>: PASS/FAIL
- warning: 0 / N개
- 환경 매트릭스: X11/Wayland/GTK3/4/Qt5/6/XIM/GNOME/Windows 영향

### 후속 협업 의뢰
- ui-manager: 위젯 추가
- doc-promo-manager: 명세 갱신
```
