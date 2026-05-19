# UNIM 0.3.0 릴리즈 노트 (한국어)

**릴리즈 날짜**: 2026-05-19
**브랜치**: arch/popup-unify

> 한 줄 요약: 팝업 아키텍처를 popup-service 단일 SoT로 통합하고, GNOME extension에 popup_view를 내장하며, 안마태 + Moachigi v4 Atomic Window를 첫 모아치기 자판으로 출시했다.

<!-- TODO: screenshot popup-service GTK4 view -->
<!-- TODO: screenshot GNOME extension popup_view -->
<!-- TODO: screenshot settings dialog moachigi group -->

---

## 마이그레이션 안내

### unim-gui-qt → unim-gui-gtk + unim-popup-service

0.3.0에서 `unim-gui-qt` 패키지가 제거됐다. 기존 KDE Plasma 사용자는 다음 두 패키지로 전환한다.

```bash
# deb
sudo apt remove unim-gui-qt
sudo apt install unim-gui-gtk unim-popup-service

# rpm
sudo dnf remove unim-gui-qt
sudo dnf install unim-gui-gtk unim-popup-service
```

**설정 파일은 그대로 보존된다.** `~/.config/unim/config.yaml`과 `~/.config/unim/layouts/` 아래의 사용자 프로파일은 패키지 제거/설치와 무관하게 유지된다.

### v0 자판 프로필 사용자

`~/.config/unim/layouts/*.json` 파일에 `schema_version`, `metadata`, `combinations` 키가 없으면 v0으로 판정되어 로더가 거부한다. 파일 첫 줄에 `"schema_version": 1`을 추가하고 `combinations` 블록을 채운다. 빌트인 프로필은 이미 0.2.0에서 모두 v1으로 이관됐다.

---

## 신규 기능 (Added)

### 1. Popup 단일 SoT 아키텍처 — unim-popup-service

한자·특수문자·이모지 팝업의 렌더링 책임이 daemon에서 신규 사이드카 프로세스 `unim-popup-service`로 이관됐다.

- daemon의 `org.atit.unim.InputContext` 시그널 8종 → `org.atit.unim.Popup` 인터페이스로 forward
- D-Bus auto-activation(`org.atit.unim.PopupService.service`) — autostart .desktop 폐기
- 단일 view-model(`PopupRender` payload): 셀·헤더·푸터·탭·하이라이트가 모든 환경에서 동일

환경별 렌더러:

| 환경 | 렌더러 |
|------|--------|
| GNOME Wayland | Extension `popup_view.js` (St 위젯) |
| GNOME X11 / KDE / Xfce | `unim-popup-service` GTK4 윈도우 |
| Wayland WM (Sway/Hyprland) | `unim-popup-service` GTK4 (wayland-backend, `libgtk4-layer-shell` 필요) |

디버그:

```bash
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup
```

### 2. GNOME Shell extension popup_view 통합

GNOME Wayland에서 Mutter가 `wlr-layer-shell`·`zwp_input_popup_v2`를 미지원하므로, extension이 `popup_view.js`의 `PopupView` 클래스(St 위젯)로 한자·특수문자·이모지 팝업을 직접 렌더한다.

- popup-service와 동일한 CSS 토큰·클래스명(`.unim-hanja-popup`, `.grid-cell` 등) 공유
- `Meta.is_wayland_compositor()`가 true일 때만 활성화 — X11에서는 popup-service GTK4 popup 사용
- **외부 좌클릭 dismiss**: 팝업 바깥 좌클릭 시 팝업이 닫히고, 클릭 이벤트는 아래 창에 그대로 전달된다

### 3. 안마태 2003(안마태) + Moachigi v4 Atomic Window

UNIM 첫 번째 모아치기(chord 기반) 자판.

- **`ko_3bul_anmatae`** 빌트인: 초성 9·중성 15·종성 20 결합 규칙
- **Moachigi v4 — Atomic Window Principle**: chord 윈도우 만료 시점에 단일 결정. 버퍼 자모 1개 → 일반 처리, 2개 이상 → 영역별 permutation 탐색
- **`chord_window_ms`**: 범위 10–200ms, 기본 60ms (숙련자 기준). 입문자 권장값 100–150ms
- **`bidirectional_combine`**: 시간차가 있는 순차 입력에서도 역방향 결합 (ㅎ→ㄱ → ㅋ)
- GTK 설정창에서 슬라이더로 조정; `supports_moachigi=true` 자판 선택 시만 그룹 표시

### 4. AutoTypeFix 학습 blacklist 강화

retrigger 시점에 tentative 억제 항목 등록 + 즉시 억제. GUI "억제 단어" 페이지에서 Tentative/Confirmed/Inactive 3단계 관리.

### 5. 한자 마우스 페이지네이션 + 9×9 그리드

- ◀/▶ 버튼이 모든 프런트엔드(GNOME·GTK·Qt·XIM·Wayland)에 통일됨
- `total_pages == 1`이면 버튼 자동 숨김
- Period(`.`) 키로 compact 9칸 ↔ expanded 81칸 토글
- 즐겨찾기 해제(☆) 시 Catppuccin yellow `#f9e2af` 140ms cursor flash

---

## 변경 (Changed)

- **설정 다이얼로그 라이브 도움말 보강**: 26개 tooltip·15개 subtitle 재작성. what/when/why/권장값 4요소 템플릿 적용
- **`chord_window_ms` 슬라이더 범위**: 10–100ms → **10–200ms**, 기본값 50ms → 60ms
- **`emoji_popup.enabled` 설정 제거**: 한자 키 idle 트리거가 항상 켜짐 (조합 중 → 한자, idle → 이모지)

---

## 수정 (Fixed, best-effort)

- **XIM ON-THE-SPOT 잔존 회귀 회피책 적용**: `commit_then_preedit`에서 `commit()` 직전 `clear_preedit()` 강제. XTerm·WezTerm 등 OVER-THE-SPOT 환경에서 정상 복귀. 일부 ON-THE-SPOT(PREEDIT_CALLBACKS) 클라이언트에서는 잔존 — xim-0.5.0 crate 근본 원인 대기 중.

---

## 호환성 깨짐 (Breaking)

- **`HanjaCandidatesReordered` 시그널 페이로드 10-tuple로 변경** (기존 9-tuple). `was_bookmarked: bool` 필드 추가. 외부 구독자는 unpacking 코드를 갱신해야 한다.
- **자판 프로필 v0 스키마 거부**: v1 마커 없는 JSON은 `LoadError::UnsupportedSchema`로 거부. `"schema_version": 1` 추가 필요.
- **`unim-gui-qt` 제거**: `unim-gui-gtk` + `unim-popup-service`로 전환.

---

## 제거 (Removed)

- `unim-gui-qt` 패키지
- `emoji_popup.enabled` 설정 필드 (5지점 전체)
- Rust 상수 자모 조합 테이블 (`JUNG_COMBINATIONS` 등)
- `SchemaKind` enum + `detect()`
- `HangulComposer3BulMoachigi` 별도 컴포저
- `ko_3bul_qwerty` 빌트인 (연구 자료는 `docs/references/keymaps/ko_3bul_qwerty_v2.json`에 보존)

---

## 알려진 미해결 이슈

- **KDE Plasma 5.x Wayland**: `gtk4-layer-shell` 미지원으로 팝업 미표시. X11 세션 또는 GNOME으로 우회.
- **XIM ON-THE-SPOT(PREEDIT_CALLBACKS) preedit 누락**: best-effort 적용 후에도 일부 클라이언트에서 잔존. xim-0.5.0 crate 업스트림 fix 대기.

---

## 더 읽을 거리

- [사용자 매뉴얼](../../user-guide/README-ko.md)
- [트러블슈팅](../../troubleshooting/README-ko.md)
- [FAQ](../../faq/README-ko.md)
- [CHANGELOG](../../../../CHANGELOG-ko.md)
- [POPUP_SPEC.md](../../../dev/specs/POPUP_SPEC.md)
