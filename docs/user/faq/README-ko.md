# UNIM FAQ (한국어)

> UNIM 0.3.0에 대해 사람들이 정말 자주 묻는 질문 모음.
> 답에는 항상 「왜 그렇게 동작하는지」를 한 줄 이상 곁들여, 단순 사실 전달이 아니라 다음 결정에 도움이 되도록 했다.

---

## Q1. 다른 한글 IME(ibus-hangul, fcitx-hangul, kime, nimf)와 무엇이 다른가?

| 항목 | UNIM | ibus-hangul | fcitx-hangul | kime | nimf |
|------|------|-------------|--------------|------|------|
| 코어 언어 | Rust | C | C | Rust | C |
| 통신 방식 | DBus 데몬 + IM 모듈 | IBus 데몬 | Fcitx 데몬 | 임베디드 | 데몬 |
| GTK3/4 지원 | ✅ 네이티브 IM 모듈 | ✅ | ✅ | ✅ | ✅ |
| Qt5/6 지원 | ✅ 네이티브 플러그인 | ✅ | ✅ | ✅ | ✅ |
| XIM | ✅ | ✅ | ✅ | ✅ | ✅ |
| Wayland (input-method-v2) | ✅ | ✅ (IBus) | ✅ | △ | ✅ |
| GNOME Shell 직접 통합 | ✅ 자체 확장 | ✅ (IBus) | △ | ✗ | ✗ |
| 자동 한↔영 오타 교정 | ✅ AutoTypeFix(forward+reverse+학습) | ✗ | ✗ | ✗ | ✗ |
| 한자 9칸/81칸 그리드 | ✅ 통일 | △ | △ | ✗ | △ |
| 한자 즐겨찾기 | ✅ DBus 시그널로 전체 동기화 | ✗ | ✗ | ✗ | ✗ |
| 사용자 자판 v1 JSON | ✅ inherits + rule_sets | ✗ | △ | △ | ✗ |
| 라이선스 | (프로젝트 라이선스 참고) | LGPL/GPL | GPL | GPLv3 | LGPL |

**한 줄 요약**: UNIM은 "Rust 코어 한 개를 모든 환경에 그대로 꽂는다"는 설계 + AutoTypeFix와 학습형 억제 사전 같은 사용자 경험 기능을 다른 IME가 안 가진 영역으로 차별화한다.

---

## Q2. 시스템 IME와 동시에 설치해도 되나?

**가능하긴 하지만 권장하지 않는다.** 한 데스크톱에 두 IME가 살아있으면 키 이벤트가 어디로 갈지 OS와 툴킷이 헷갈린다.

- **GNOME**: IBus와 UNIM 동시 사용 시 키 이벤트 유실 빈발 → IBus 제거 필수.
  ```bash
  sudo apt remove ibus
  ```
- **KDE**: fcitx5는 자체 데몬을 띄우므로 충돌. 환경변수에서 한쪽만 활성화하라.
- **테스트 환경**: VM/컨테이너로 분리해 비교하는 건 좋다.

> 결론: "둘 중 하나"가 정답. UNIM 도입 전 기존 IME를 깔끔히 제거하라.

---

## Q3. 어떤 환경이 가장 안정적인가?

UNIM 0.2.0 시점의 안정성 등급:

| 환경 | 등급 | 비고 |
|------|------|------|
| Ubuntu 24.04 + GNOME(Wayland) + GNOME Extension | 🟢 A | 권장. 메인 개발/테스트 환경 |
| Ubuntu 24.04 + GNOME(X11) | 🟢 A | 환경변수 + Standalone 팝업 |
| KDE Plasma 6 (Wayland) | 🟢 B+ | input-method-v2 잘 동작 |
| KDE Plasma 6 (X11) | 🟢 B+ | XIM/Qt IM 둘 다 양호 |
| Sway (Wayland) | 🟡 B | 팝업 위치 약간 미흡, [팝업 명세](../../dev/specs/POPUP_SPEC.md) §8.4 참고 |
| Hyprland (Wayland) | 🟡 B | 동상 |
| XFCE/MATE (X11) | 🟢 B+ | 전통적 환경, 잘 동작 |
| Wayland 단독(컴포지터별) | 🟡 B/C | 컴포지터의 IM 프로토콜 지원도에 의존 |

**A 등급 권장 = "처음 깔아 보는 사람용"**. 익숙해지면 다른 환경도 충분히 사용 가능.

---

## Q4. AutoTypeFix는 정확히 어떻게 동작?

2단계로 보면 된다.

### 단계 1 — 관측

엔진은 사용자 키 입력을 모드별로 동시에 두 개의 가상 트랙(한글 트랙, 영어 트랙)으로 시뮬레이션한다. 예: 영문 모드에서 `gksrmf`이 들어오면 한글 트랙은 동시에 `한글`을 만든다.

### 단계 2 — 트리거

단어 경계(스페이스/구두점/Enter)가 들어왔을 때, 현재 모드와 다른 트랙이 「의미 있는 단어」를 만들었으면 교정 제안.

- forward: 영문 모드인데 한글 트랙이 글자 만듦 → `gksrmf` → `한글`
- reverse: 한글 모드인데 영어 트랙이 단어 만듦 → `ㅈㅐㅍㅁ` → `wave`

### 학습 (억제 사전)

사용자가 교정을 「실수」로 받아들이고 BS+모드전환으로 되돌리면 → Pending 표시. 같은 단어가 또 트리거되면 「그 시도를 억제하면서 동시에 Tentative로 등록」. GUI에서 [확정] 누르면 영구.

> 이 모델의 핵심은 **등록을 롤백 시점이 아니라 재시도 시점에 한다**는 것. 모드를 잘못 둔 일회성 오타로는 학습되지 않게 막는 안전장치.

---

## Q5-1. 즐겨찾기를 끄면 popup이 다른 페이지로 휙 넘어가는데 정상인가?

**정상 동작이다.** 즐겨찾기(★)는 한자를 1페이지 상단으로 promote하고, 해제(☆)는 그 한자를 원래 사전순 위치로 demote한다. 사전순 위치가 현재 페이지가 아니면 popup이 그 페이지로 점프한다.

이 점프 자체를 사용자가 놓치지 않게, 도착한 cursor 셀이 **140ms 동안 노란색(`#f9e2af`)으로 짧게 깜박인다(flash)**. "내가 별을 끄니 이 한자가 여기로 돌아갔구나"를 시각으로 잡아 주는 신호다.

- 등록(★ ON)은 cursor가 1페이지 상단으로 따라가는 게 자연스럽고 눈에 띄어서 flash가 없다.
- 해제(☆ OFF)는 어디로 갈지 예측이 어려워서 flash가 있다.

자세한 동작은 [사용자 매뉴얼 §4.2](../user-guide/README-ko.md#42-한자-변환-hanja)와 [팝업 명세 §3.6/§3.7](../../dev/specs/POPUP_SPEC.md) 참고.

---

## Q5. 한자 9칸 vs 81칸 차이는?

| 항목 | 9칸 (compact) | 81칸 (expanded) |
|------|----------------|------------------|
| 화면 점유 | 작음 | 큼 |
| 시야 후보 수 | 9개 | 81개 (9×9) |
| 적합 상황 | 자주 쓰는 후보가 상위에 있음 | 드문 한자, 동음이의어가 많음 |
| 토글 키 | — | `.` (마침표) |
| 표시 | ⊟ 아이콘 | ⊞ 아이콘 |
| 키 바인딩 | 1~9 직선택, 화살표 | 1~9 + 행 점프, 화살표 |

> 한자가 9개를 넘기면 9칸은 PageDown 대신 화살표로 다음 페이지를 보여 주지만, 81칸은 한 번에 9페이지를 펼쳐 시각적으로 바로 비교 가능.

---

## Q6. 설정 파일 위치, 백업과 복원은?

### 위치

```
~/.config/unim/
├── config.yaml              # 일반 설정 (소스 오브 트루스)
├── typefix-blacklist.yaml   # 학습된 억제 사전
├── userdict.yaml            # reverse 사용자 사전 (0.2.0 신규)
└── layouts/                 # 사용자 정의 v1 자판
    └── my_3bul_variant.json
```

### 백업

```bash
tar -czf ~/unim-backup-$(date +%F).tar.gz -C ~/.config unim
```

### 복원

```bash
# 데몬을 멈추고 복원하면 안전
systemctl --user stop unim-daemon
tar -xzf ~/unim-backup-2026-04-26.tar.gz -C ~/.config
systemctl --user start unim-daemon
```

> 데몬은 `typefix-blacklist.yaml`과 `userdict.yaml`을 mtime 감시로 자동 리로드하므로 데몬을 멈추지 않고도 복원할 수 있다. 다만 `config.yaml`은 일부 키가 데몬 시작 시 캐시되므로 재시작이 안전.

---

## Q7. 자판은 어떤 게 있고, 새로 추가할 수 있나?

### 내장 한국어 자판

`ko_2bulstd`(두벌식 표준), `ko_3bul390`(세벌식 390), `ko_3bul391`(세벌식 391), `ko_3bul_noshift`(세벌식 순아래), `ko_3bul_anmatae`(안마태, 모아치기).

> 참고: 쿼티형 세벌식(`ko_3bul_qwerty`)은 빌트인이 아닌 연구 자료로만 보존된다.
> `docs/references/keymaps/ko_3bul_qwerty_v2.json`을 `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사하면 사용자 프로필로 활성화된다.

### 영어

`qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`.

### 사용자 정의

`~/.config/unim/layouts/<name>.json`에 v1 스키마 JSON을 넣으면 데몬이 자동 스캔. `inherits: "ko_3bul390"` 같이 상속해 일부만 덮어쓸 수도 있음.

```bash
# 검증
unim-cli config layout validate ~/.config/unim/layouts/my.json
# 활성화
unim-cli config set korean.layout my
```

스키마 상세는 [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](../../archive/plans/LAYOUT_PROFILE_V1.md).

> rule_sets로 같은 자판에 옵션 토글을 붙일 수도 있다. 예: `ko_3bul390`의 `sun_arae_batchim`(순아래받침). 설정 GUI에서 SwitchRow가 동적으로 나타난다.

---

## Q8. UNIM의 메모리 사용량은 얼마나 되나?

정상 운영 시 `unim-daemon` RSS는 30~80 MB 수준. UNIM 0.2.0이 사용하는 안정화 장치:

- `#[global_allocator] tikv_jemallocator::Jemalloc` — glibc ptmalloc arena 폭주 차단.
- `Environment=MALLOC_ARENA_MAX=2` (systemd) — C 라이브러리 경로 이중 차단.
- 60초 주기 `libc::malloc_trim(0)` — OS에 메모리 반환 강제.

> 과거 RSS가 2 GB까지 부풀던 사건(0.1.x) 이후 회귀 금지로 못 박았다. RSS 500 MB+ 관측 시 [트러블슈팅 §14](../troubleshooting/README-ko.md#14-데몬이-메모리-너무-많이-먹음-rss-500mb)로.

---

## Q9. UNIM이 비밀번호 입력을 가로채는가?

**아니다.** 비밀번호 필드는 `content_purpose`로 식별되어 자동으로 영문 강제 전환된다. AutoTypeFix(순방향·역방향)·한자 변환·특수문자 팝업 모두 비활성화된다. 이미 쌓인 키 관측 버퍼·되돌리기 기록도 함께 지워져, `dkssud` 같은 비밀번호가 한글로 자동 교정돼 값이 깨지지 않는다. 입력값은 데몬 메모리에도 남기지 않는다.

> 단, 자동 검출은 앱이 `content_purpose=password`를 정확히 보고할 때만 동작한다. 보고하지 않는 환경 — **XIM 레거시 앱, Windows IMM32 폴백, content-purpose 를 보내지 않는 일부 Wayland 컴포지터·웹폼** — 에서는 자동 감지가 안 될 수 있으니, 그런 곳에서는 직접 한/영 키로 영문 모드를 확인하길 권장한다. (정상 감지 환경: GTK3/4·Qt5/6·GNOME 확장·Windows TSF)

---

## Q9-1. 비밀번호 칸에서는 왜 자동 오타 교정이 안 되나?

**의도된 동작이다.** 비밀번호·PIN 칸에서는 자동 오타 교정을 일부러 끈다(Q9 참고). 켜 두면 `dkssud`처럼 친 비밀번호가 단어 경계에서 한글로 바뀌어 로그인이 깨지기 때문이다. 칸을 벗어나면 즉시 원래대로 돌아오고, 수동으로 켜고 꺼 둔 토글 상태도 그대로 유지된다.

> 반대로 **비밀번호가 아닌 일반 칸인데도 교정이 안 되는** 경우는 다른 원인이다 → [트러블슈팅](../troubleshooting/README-ko.md) §8. 위에 적은 미감지 환경(XIM·IMM32·일부 Wayland)에서는 비밀번호 칸이 일반 칸으로 취급돼 오히려 교정이 발동할 수 있는데, 이 한계도 트러블슈팅 §8-1 에 정리돼 있다.

---

## Q9-2. `install.sh`(한 줄 설치)는 안전한가요?

`curl -fsSL .../install.sh | bash` 는 편리하지만 "정체불명 스크립트를 통째로 실행"하는 것이므로 걱정될 수 있다. UNIM 설치 스크립트에는 다음 4가지 안전장치가 있다.

1. **SHA256 체크섬 검증** — 릴리스에 포함된 `SHA256SUMS` 를 매니페스트로 삼아 내려받은 모든 `.deb` 를 검증한다. 하나라도 불일치하면 **설치 단계로 넘어가지 않고 중단**한다(부분 설치 없음).
2. **임시 디렉토리 격리** — 모든 다운로드는 `mktemp` 로 만든 임시 디렉토리에 저장되고, 성공/실패/중단 어느 경우든 `trap` 으로 자동 삭제된다. 시스템에 남는 파일이 없다.
3. **apt 트랜잭션** — 설치는 `dpkg -i` 가 아니라 `apt install` 로 하므로, 외부 런타임 의존성까지 apt 의 원자적 트랜잭션으로 해결된다. 실패 시 부분 설치가 남지 않는다.
4. **스크립트 전문 공개** — 스크립트는 [main 브랜치](https://github.com/from104/unim/blob/main/install.sh)에 그대로 공개돼 있다. 실행 전에 `curl ... -o install.sh` 로 받아 직접 읽어볼 수 있다.

> 한계: `SHA256SUMS` 가 `.deb` 와 **같은 출처(GitHub Releases)**에 있으므로, 전송 무결성은 보장되지만 출처 인증은 GitHub 의 TLS 신뢰에 의존한다. GPG/minisign 서명은 향후 과제다. 최소 권한을 원하면 방법 2(수동 다운로드)로 각 파일을 직접 검증해 설치하면 된다.

---

## Q10. 0.1.x에서 0.2.0으로 업그레이드 시 주의?

대부분 자동. 단 두 군데 자동 정규화가 있다.

- `config.yaml`의 `korean.layout`이 enum 형식(`Dubeolsik`)이면 → `ko_2bulstd` 같은 문자열로 자동 변환.
- `english.layout`이 enum이면 → `qwerty` 등 문자열로 자동 변환.
- `typefix-blacklist.yaml`의 옛 키도 serde compat 레이어로 자동 정규화.

C-API: `UnimEnglishLayout`/`UnimKoreanLayout` enum 제거 → C 문자열 setter/getter로 변경. C/C++ 클라이언트 사용자만 영향.

자세한 마이그레이션은 [릴리즈 노트](../release-notes/0.2.0/RELEASE_NOTES-ko.md).

---

## Q11. UNIM은 macOS/Windows에서도 되나?

**현재는 리눅스 전용.** 로드맵 5단계에 크로스 플랫폼이 적혀 있지만 미착수. Rust 코어와 C-API가 분리돼 있어 이론적으론 각 플랫폼의 IME 인터페이스(macOS의 IMKit, Windows의 TSF)에 어댑터를 붙이면 된다. 자원자가 있다면 환영.

---

## Q12. 빌드에 cargo 1.95가 왜 필요?

`Cargo.lock` 파일이 v4 포맷이고, 이 포맷은 cargo 1.83+이 안전하게 다룰 수 있다 (1.95에서 안정 사용 검증). 리눅스 배포판에 따라 `/usr/bin/cargo`가 1.75 같은 구 버전인 경우가 있어 다음과 같이 명시 권장:

```bash
rustup update stable
which cargo                  # ~/.cargo/bin/cargo 가 잡혀야 함
cargo --version              # 1.95.0 이상
```

소스 빌드 실패 메시지에 `lock file version 4 requires '-Znext-lockfile-bump'`가 보이면 100% 이 문제다.

---

## Q13. 기여하고 싶다 — 어디부터?

1. [`CONTRIBUTING.md`](../../../CONTRIBUTING.md) — 브랜치/PR 워크플로.
2. [`AGENTS.md`](../../dev/architecture/AGENTS.md) — 아키텍처와 컴포넌트 맵.
3. [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md) — 동작 명세.
4. 컴포넌트 별 `SPEC.md` (각 크레이트 안).
5. 빌드 검증: `make build` 경고 0개 + `cargo test --workspace` 전체 통과.
6. 커밋 메시지는 영어, 문서는 한국어 (프로젝트 컨벤션).

`good-first-issue` 라벨 이슈가 출발점으로 가장 좋다.

---

## Q14. 「universal」이라는 이름은 왜?

**Universal Next-generation Input Method**의 약자. "한글" 입력기지만 한국어/영어를 자유롭게 오가는 양방향 + 모든 툴킷에 한 코어를 그대로 쓴다는 의미. 장기적으로는 macOS/Windows까지 확장(로드맵 5단계).

---

---

## Q15. 안마태(안마태)와 Moachigi(모아치기)가 뭐가 다른가?

**안마태**는 구체적인 자판 배열 이름이다. 2003년에 완성된 세벌식 계열 자판으로, 초·중·종성이 키보드 영역별로 고정 배치된다. UNIM에서는 `ko_3bul_anmatae` 프로필로 내장됐다.

**Moachigi**(모아치기, "모아서 치기")는 입력 방식이다. 여러 자모를 동시에 또는 짧은 시간 안에 연달아 누르면 하나의 음절로 묶어서 처리한다. 일반 두벌식·세벌식이 한 키씩 순서대로 처리하는 것과 달리, 모아치기는 chord 윈도우(기본 60ms) 안에 들어온 키를 한꺼번에 처리한다.

즉, 안마태 자판으로 모아치기를 하는 것이 UNIM 0.3.0의 첫 모아치기 지원이다. `supports_moachigi=true` 자판에서만 모아치기 설정 그룹이 GTK 설정창에 나타난다.

---

## Q16. popup-service가 안 뜨는 증상은 어떻게 진단하나?

`unim-popup-service`가 설치됐는지 먼저 확인한다.

```bash
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup
```

응답이 없으면 패키지 미설치 또는 D-Bus 서비스 파일 누락이다. 자세한 진단은 [트러블슈팅 §16](../troubleshooting/README-ko.md#16-popup-service-디버깅-030).

---

## Q17. deb에서 rpm으로 또는 그 반대로 마이그레이션할 때 설정이 유지되나?

예. 사용자 설정과 레이아웃은 `~/.config/unim/` 아래에 저장되며, 패키지 형식과 무관하다.

- `~/.config/unim/config.yaml` — 주 설정 파일
- `~/.config/unim/layouts/*.json` — 사용자 자판 프로필
- `~/.config/unim/typefix-blacklist.yaml` — AutoTypeFix 억제 사전
- `~/.config/unim/userdict.yaml` — 사용자 사전

패키지를 제거하고 다른 형식으로 재설치해도 위 파일들은 건드리지 않는다. 단, `unim-gui-qt` 패키지는 0.3.0에서 제거됐으니 `unim-gui-gtk`와 `unim-popup-service`로 대체한다.

---

## Q19. 자판을 눈으로 보거나 편집·연습할 도구가 있나?

두 개의 GTK4 보조 도구가 함께 설치된다.

- **`unim-keymap-studio` (자판 스튜디오)**: 한국어/영문 자판을 시각적으로 보고 편집한다.
  헤더의 3단 드롭다운(언어 > 출처 > 자판)으로 대상을 고르고, 4개 탭(기본 / 자판 / 조합 / 확장)으로
  내용을 본다. 「조합」·「확장」 탭은 한글 자판일 때만 나타난다. 헤더 우측에는 [도움말]·[설정]·[메뉴].
  - **빌트인 자판**은 읽기 전용이라 「다른 이름으로 저장」만 가능하고, **사용자 자판**은
    「저장」 + 「다른 이름으로 저장」 둘 다 된다.
  - 사용자 자판은 `~/.config/unim/layouts/` 에 JSON으로 저장된다(Q7의 사용자 정의 자판과 동일 위치).
- **`unim-typing-practice` (타자 연습)**: 현재 활성 자판으로 타자 연습을 한다. WPM/CPM,
  정확도, 오타 히트맵을 측정해 어떤 키에서 실수가 많은지 보여 준다.

두 도구는 동일한 5행 키보드 위젯을 공유하므로 보이는 자판 모양이 일관된다. 단축키는
[사용자 매뉴얼 §5.6](../user-guide/README-ko.md#56-자판-도구-keymap-studio--typing-practice) 참고.

---

## Q18. chord_window_ms 적정값은 얼마인가?

범위는 **10–200ms**, 기본값은 **60ms**다. 60ms는 숙련자 기준으로 설계됐다.

| 프로필 | 권장 범위 | 설명 |
|--------|----------|------|
| 입문자 | 100–150ms | 키를 동시에 누르는 타이밍이 불안정할 때 넉넉하게 |
| 일반 | 60–100ms | 대부분의 사용자에게 편안한 범위 |
| 숙련자 | 10–60ms | 오입력 최소화, 반응 속도 우선 |

`chord_window_ms`를 0으로 설정하면 모아치기가 완전히 꺼진다. 설정창의 슬라이더로 조정하거나 `unim-cli config set chord-window-ms 80` 명령을 쓴다.

## Q20. 키를 오래 누르면 글자가 여러 번 입력돼요.

손 떨림 등으로 키를 오래 누르고 있으면 운영체제가 같은 키를 빠르게 반복 입력한다(자동 반복). UNIM에는 이 반복을 데몬이 무시하는 **조합키 자동반복 억제(접근성)** 옵션이 있다. 억제 대상은 **한/영 토글 키와 한글 모드의 문자 키**이며, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둔다. 기본값은 꺼짐이라 켜기 전에는 동작이 바뀌지 않는다.

켜는 법 — 설정 앱의 **접근성 → 조합키 자동반복 억제** 스위치, 또는 CLI:

```bash
unim-cli config set ignore-key-repeat true
```

**폴백의 한계**: Wayland·Qt5/6·GNOME 확장은 반복 여부를 정확히 가려낸다. 반면 GTK3/4·XIM·ibus 호환 경로는 80ms 시간창으로 근사 판정하므로 (1) 첫 반복 1회는 통과될 수 있고, (2) 시스템의 키 반복 간격을 80ms보다 길게 설정했다면 걸러지지 않을 수 있다. 어느 경우든 오판 시 항상 "덜 막는" 쪽으로 안전하게 동작한다. GNOME 확장 사용자는 재로그인 후 적용된다.

---

## 더 읽을 거리

- [사용자 매뉴얼](../user-guide/README-ko.md)
- [트러블슈팅](../troubleshooting/README-ko.md)
- [릴리즈 노트 0.3.0](../release-notes/0.3.0/README.md)
- [릴리즈 노트 0.2.0](../release-notes/0.2.0/RELEASE_NOTES-ko.md)
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md)
- [`docs/dev/specs/POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)
