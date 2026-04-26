# UNIM 0.2.0 릴리즈 노트 (한국어)

> 릴리즈 일자: 2026-04-26
> 코드네임: "Phase 8 cleanup + AutoTypeFix 완성"
> 0.1.0(2026-04-21) 이후 약 5일간의 집중 정리 및 신규 기능 통합.

---

## 한 줄 요약

자동 한↔영 오타 교정(AutoTypeFix)이 학습형 억제 사전과 사용자 사전을 갖추며 실용 단계에 도달했고, 한자 팝업이 9칸/81칸 토글과 즐겨찾기를 얻었으며, 자판이 v1 JSON 프로필 시스템으로 이관되어 사용자 자판 상속 + 옵션 규칙 토글이 가능해졌다.

---

## 신규 기능 (Added)

### 1. 자판 프로필 v1 (스펙 + 엔진 + 설정 + CLI + GUI)

내장 자판이 `src/keystroke/keymap/*.json` 자기 완결 v1 JSON으로 이관됐다. Rust const + 부분 JSON 혼합 경로를 단일 JSON으로 통합.

- **사용자 프로필**: `~/.config/unim/layouts/*.json`에 v1 JSON을 넣으면 데몬이 시작 시 스캔 + mtime 기반 핫 리로드.
- **inherits 체인 해석**: 자식 프로필이 `"inherits": "base_name"`을 선언하면 `ProfileRegistry`가 순환 탐지 + 레이어 병합으로 metadata/layout/rule_sets 해석.
- **rule_sets**: 프로필별 명명 옵션 서브룰. 예: `ko_3bul390`의 `sun_arae_batchim`. GUI SwitchRow 또는 CLI `set korean-active-rule-sets`로 토글.
- **설정 필드**(가산): `korean.custom_layout: Option<String>`, `korean.active_rule_sets: Vec<String>`. 5지점 싱크 (config.rs ↔ unim-cli ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog).
- **CLI**: `unim-cli config layout list / describe <name> / validate <file.json>` (exit code 0=pass, 1=warning, 2=error).
- **GUI**: 한국어 자판 ComboRow가 내장 + 사용자 프로필 모두 표시. 선택 시 rule_sets가 동적 SwitchRow로 재구성.
- **신규 내장 프로필**: `ko_3bul_qwerty`(쿼티형 세벌식, 26자리 알파벳 포화 — 14 초성/15 중성/19 종성). 내장 9 → 10.

### 2. AutoTypeFix 억제 사전 + 사용자 사전

- **억제 사전**(`~/.config/unim/typefix-blacklist.yaml`): Tentative/Confirmed/Inactive 3상태. 등록 시점이 「롤백 시점」에서 「재트리거 시점」으로 이동했고, 그 재트리거를 동시에 억제. GUI에서 [확정] 누르면 Tentative → Confirmed. `tentative_expiry_hours`(기본 1시간, 1~12)가 지나면 Inactive로 만료.
- **AutoTypeFix 설정** 신규 3키: `auto_typefix.rollback_detection`(bool, 기본 true), `auto_typefix.tentative_expiry_hours`(u16, 1~12), `auto_typefix.observation_timeout_secs`(u8, 5~15). 3지점 싱크.
- **설정 GUI 「교정 억제 단어」 페이지**: 3섹션(Tentative/Confirmed/Inactive) + 행별 [확정]/[비활성화]/[삭제]/[재활성화] 액션.
- **사용자 사전**(`~/.config/unim/userdict.yaml`): 텍스트 선택 후 단축키 → `RegisterUserDictFromSelection` DBus 메서드 → 영어 사전 항목 등록. GUI 「사용자 사전」 페이지에서 추가/제거/수정.

### 3. 한자 팝업 확장

- **9×9 = 81칸 확장 그리드**: 마침표(`.`) 키로 compact 9칸 ↔ expanded 81칸 토글. GTK Standalone, GTK IM, Qt IM, XIM 전 프론트엔드에 통일 적용 (GNOME Extension은 0.1.x부터 지원). ⊞/⊟ 아이콘으로 현재 모드 시각화.
- **즐겨찾기 (Hanja Bookmark)**: 후보 포커스 상태에서 Space 키로 ☆/★ 토글. `HanjaBookmarkChanged` DBus 시그널이 GTK/Qt/XIM/Wayland/GNOME의 모든 열린 팝업을 즉시 갱신.

### 4. 자동 영문 모드 전환 (Auto-English-Mode)

- **opt-in 기능** (`engine.auto_english.{enabled, trigger_keys}`, 기본 비활성).
- 한글 모드에서 trigger key(`Esc`, `/` 등) 입력 → preedit commit + 영문 모드로 영구 전환 + 트리거 키 통과.
- 사용자 정의 trigger 키: `ShiftSemicolon`(:), `ShiftSlash`(?) 같은 `Shift<Name>` 가상명 추가 가능.
- vim 명령 모드, CLI 슬래시 명령 사용자에게 유용.

### 5. 기타

- **Korean/English 자판 enum 제거**: 자판은 이제 평범한 문자열로 표현 (Phase 8/9). `KoreanLayout`/`EnglishLayout` enum 삭제, C-API setter/getter도 C 문자열로 통일.
- **`unim-config` orphan 크레이트 제거**: `unim-cli config` 서브커맨드로 통합.

---

## 변경 (Changed)

- **AutoTypeFix reverse 롤백 게이트 완화**: BS-AND-switch → **BS-OR-switch**. reverse 교정은 `clear_preedit=true`로 IM 모듈이 BS를 흡수하므로 engine_worker엔 도달하지 않아 AND 게이트가 구조적 불가였다. 모드 전환 단독으로 충분하게 변경. forward는 여전히 BS-AND-switch.
- **AutoTypeFix reverse 억제 키 수정**: `RecentCorrection.ascii`가 reverse는 `fix.corrected`(영단어), forward는 `fix.original`(ASCII 원문)을 저장. 이전엔 reverse가 빈 문자열로 등록되어 후속 매칭에 실패하던 버그 수정.
- **Blacklist 등록 시점 이동**: rollback-moment → retrigger-moment. 모드 오전환 같은 일회성 이벤트로는 등록되지 않도록 안전.
- **`KoreanLayout` enum 제거 (Phase 8)**: `korean.layout`은 String. 내장(`ko_2bulstd`/`ko_3bul390`/...) 또는 사용자 프로필 이름. 레거시 `custom_layout: Option<String>` 필드는 `layout`으로 병합. 기존 `config.yaml`(`layout: Dubeolsik`)과 `typefix-blacklist.yaml`은 serde compat 레이어로 자동 정규화.
- **`EnglishLayout` enum 제거 (Phase 9)**: 한국어와 대칭. `english.layout`은 String (내장: `qwerty`/`dvorak`/`colemak`/`colemak_dh`/`workman`). 레거시 YAML 자동 정규화.
- **트레이/팝업 즉시 동기화**: `unim-gui`가 `GlobalModeChanged` 시그널 수신 즉시 트레이 아이콘과 팝업을 동기화하도록 리팩터.

---

## 수정 (Fixed)

- **GTK3/4 IM의 preedit-end 누락 → ghostty 키 잠금**: `unim_emit_preedit` 헬퍼로 모든 commit/clear 경로에서 preedit-end가 누락되지 않게 보장.
- **XIM AutoTypeFix N+1 BS 방식 재구현**: Chrome preedit 잔존 외 안정 동작. (자세한 내용은 `unim-frontends/xim/SPEC.md` 참고)
- **dbus_ime.js의 `call_sync` 비표준 인자 수정**: `cancelHanja`/`cancelSpecialChar` 호출 시 `GLib.VariantType` 인자 수정.
- **자판 프로필 핫리로드 시 활성 자판 재초기화**: 디렉토리 mtime 감시로 자동 재스캔 + Composer 재구성.
- **Wayland popup_surface 정리 누락 수정**: FocusOut/Reset 시 popup_surface가 dangling 상태로 남던 케이스 정리.

---

## Breaking Changes (사용자 영향)

| 변경 | 영향 | 자동 마이그레이션 |
|------|-----|------------------|
| `KoreanLayout` enum → String | C/C++ 클라이언트만 영향 | YAML은 자동 정규화 |
| `EnglishLayout` enum → String | 동일 | 동일 |
| `unim-config` 크레이트 제거 | CLI 호출 경로 변경 | `unim-cli config` 서브커맨드로 동일 기능 |
| `custom_layout` 필드 통합 → `layout` | 직접 YAML을 편집한 경우 | serde compat 자동 처리 |

**일반 사용자 영향 없음.** GUI/CLI 사용자 누구도 손댈 게 없다.

---

## 마이그레이션 가이드

### 1. 패키지 업그레이드

```bash
sudo apt install ./unim_0.2.0_amd64.deb \
                 ./unim-common_0.2.0_amd64.deb \
                 ./unim-im-gtk_0.2.0_amd64.deb \
                 ./unim-im-qt_0.2.0_amd64.deb \
                 ./unim-gui-gtk_0.2.0_amd64.deb
```

### 2. 데몬 재시작

```bash
systemctl --user daemon-reload
systemctl --user restart unim-daemon
```

### 3. (선택) 새 기능 ON

```bash
# 자동 영문 모드 (vim 사용자 추천)
unim-cli config set engine.auto_english.enabled true

# AutoTypeFix Tentative 만료 시간 조정 (기본 1시간 → 6시간)
unim-cli config set auto_typefix.tentative_expiry_hours 6
```

### 4. 새 자판(`ko_3bul_qwerty`) 시도

```bash
unim-cli config set korean.layout ko_3bul_qwerty
```

또는 GTK 설정 GUI → 「일반」 → 「한국어 자판」.

---

## 알려진 이슈

| ID | 설명 | 영향 | 대안 |
|----|------|-----|------|
| KI-001 | 순수 Wayland(컴포지터: Sway/Hyprland)에서 팝업 좌표가 약간 어긋남 | UI 시각적 불일치 | 보고: [팝업 명세](../../specs/POPUP_SPEC.md) §8.4 |
| KI-002 | XIM Chrome에서 AutoTypeFix preedit이 일부 잔존 | 드물게 시각적 잔재 | Chrome 외 브라우저는 영향 없음 |
| KI-003 | 일부 Snap 앱이 `~/.profile`의 조건부 환경변수를 무시 | Snap 앱 한글 입력 실패 | `QT_IM_MODULE= GTK_IM_MODULE= snap run <앱>` |

---

## 컴포넌트별 변경 요약

| 컴포넌트 | 주요 변경 |
|----------|----------|
| Core (`src/`) | AutoTypeFix 억제 사전 + 자판 프로필 v1 + auto_english 훅 |
| C-API (`unim-capi/`) | KoreanLayout/EnglishLayout enum 제거 → C 문자열 |
| Daemon (`unim-daemon/`) | 자판 프로필 핫리로드, blacklist mtime 감시 |
| DBus (`unim-dbus/`) | `RegisterUserDictFromSelection`, `HanjaBookmarkChanged` 신규 |
| CLI (`unim-cli/`) | `config layout` 서브커맨드, `unim-config` 통합 |
| GTK GUI (`unim-gui-gtk/`) | 「교정 억제 단어」 페이지, 「사용자 사전」 페이지, dynamic rule_sets SwitchRow |
| Qt GUI (`unim-gui-qt/`) | GlobalModeChanged 즉시 동기화 |
| GTK3/4 IM | `unim_emit_preedit` 헬퍼, preedit-end 누락 수정 |
| Qt5/6 IM | 81칸 그리드, 즐겨찾기 시각화 |
| XIM | AutoTypeFix N+1 BS 방식 재구현 |
| Wayland | popup_surface 정리 누락 수정 |
| GNOME Extension | dbus_ime call_sync 인자 수정, 즐겨찾기 시그널 수신 |

---

## 기여자 / 감사

UNIM 프로젝트는 단일 메인테이너(서기현) 주도 + Claude Code 기반 자동화로 진행됐다. 0.2.0의 Phase 8/9 정리, AutoTypeFix 안정화, 자판 v1 이관에 사용된 하네스 구성은 [`AGENTS.md`](../../../AGENTS.md), [`.claude/agents/`](../../../.claude/agents/), [`.claude/skills/`](../../../.claude/skills/)에 기록.

---

## 다음 단계 (0.3.0 예고)

- 문맥 인식 기반 자동 한/영 전환 (지능화) — 로드맵 4단계.
- 엔진 v2 재설계 (모아치기, 옛한글, 복벌식 — 로드맵 6단계).
- macOS/Windows 어댑터 — 로드맵 5단계.

상세 로드맵: [`ROADMAP.md`](../../../ROADMAP.md).

---

## 참고 문서

- [사용자 매뉴얼](../../user-guide/README-ko.md)
- [트러블슈팅](../../troubleshooting/README-ko.md)
- [FAQ](../../faq/README-ko.md)
- [`CHANGELOG-ko.md`](../../../CHANGELOG-ko.md) — 전체 변경 내역
