# UNIM FAQ (한국어)

> UNIM 0.4.1에 대해 사람들이 정말 자주 묻는 질문 모음.
> 답에는 항상 「왜 그렇게 동작하는지」를 한 줄 이상 곁들여, 단순 사실 전달이 아니라 다음 결정에 도움이 되도록 했다.

---

<!-- @platform:linux -->
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
<!-- @endplatform -->

## Q2. 시스템 IME와 동시에 설치해도 되나?

<!-- @platform:linux -->
**🐧 리눅스**

**가능하긴 하지만 권장하지 않는다.** 한 데스크톱에 두 IME가 살아있으면 키 이벤트가 어디로 갈지 OS와 툴킷이 헷갈린다.

- **GNOME**: IBus와 UNIM 동시 사용 시 키 이벤트 유실 빈발 → IBus 제거 필수.
  ```bash
  sudo apt remove ibus
  ```
- **KDE**: fcitx5는 자체 데몬을 띄우므로 충돌. 환경변수에서 한쪽만 활성화하라.
- **테스트 환경**: VM/컨테이너로 분리해 비교하는 건 좋다.

> 결론: "둘 중 하나"가 정답. UNIM 도입 전 기존 IME를 깔끔히 제거하라.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

**공존해도 된다.** Windows는 입력기(TSF — Text Services Framework, 윈도우의 입력기 규격)를 **여러 개 등록해 두고 골라 쓰는** 구조라, 리눅스처럼 "하나만 남기고 지워야" 하는 상황이 아니다. UNIM MSI도 기존 입력기를 지우지 않고 목록에 **추가**만 한다.

- 등록된 입력기 목록: **설정 → 시간 및 언어 → 언어 및 지역 → 한국어 → 키보드**.
- 어느 것이 먼저 뜰지는 **기본 입력기** 지정이 정한다. 작업표시줄 입력 표시를 **오른쪽 클릭 → `기본 입력기로 설정`** 으로 UNIM을 기본으로 둘 수 있다.
- 다만 **문제를 진단할 때는 다른 한글 IME(날개셋·새나루 등)를 잠시 비활성화**하고 UNIM 단독으로 재현해 보는 편이 원인을 가리기 쉽다.
<!-- @endplatform -->

---

## Q3. 어떤 환경이 가장 안정적인가?

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

Windows 지원은 v0.4.0에서 들어갔다. 리눅스처럼 환경별 등급을 매길 만큼 여러 머신의 실측이 쌓이지는 않았다. 현재 확인된 지원 범위는 이렇다.

| 항목 | 내용 |
|------|------|
| 운영체제 | Windows 10 / 11 (64비트) |
| 입력 방식 | TSF (Text Services Framework) |
| 64비트 앱 | `unim_tsf.dll` — 메모장·Edge·Chrome·워드 등 |
| 32비트 앱 | 별도의 32비트 TIP `unim_tsf32.dll` — 카카오톡·한컴 등 |
| 콘솔·IMM32 계열 앱 | WezTerm·텔레그램 등에서 한글 조합 지원 |

- **앱 조합의 폭은 리눅스보다 좁게 검증됐다.** 개발자 상용 환경에서 매일 쓰이지만, 드문 앱에서는 문서와 실제가 다를 수 있다.
- 처음 설치했다면 **메모장에서 먼저** 한/영 전환·한자 팝업이 되는지 확인한 뒤 다른 앱으로 넓혀 가는 편이 원인 격리에 좋다.
- 이상 동작을 만나면 [GitHub Issues](https://github.com/from104/unim/issues)에 **앱 이름 · Windows 버전(winver) · 32/64비트 여부**를 함께 적어 알려 주면 도움이 된다.
<!-- @endplatform -->

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

<!-- @platform:linux -->
**🐧 리눅스**

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
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

### 위치

```
%APPDATA%\unim\
├── config.yaml                # 일반 설정 (소스 오브 트루스)
├── typefix-blacklist.yaml     # 학습된 억제 사전 (억제 단어 탭)
├── typefix-userdict.yaml      # 역방향 사용자 사전 (사용자 사전 탭)
└── layouts\                   # 사용자 정의 v1 자판
    └── my_3bul_variant.json
```

탐색기 주소창에 `%APPDATA%\unim` 을 그대로 붙여 넣으면 열린다. (보통 `C:\Users\<사용자>\AppData\Roaming\unim`)

### 백업

```powershell
Compress-Archive -Path "$env:APPDATA\unim" -DestinationPath "$env:USERPROFILE\Desktop\unim-backup.zip"
```

### 복원

```powershell
Expand-Archive -Path "$env:USERPROFILE\Desktop\unim-backup.zip" -DestinationPath "$env:APPDATA" -Force
```

> 리눅스와 달리 **멈추고 시작할 데몬이 없다.** 입력 처리는 각 앱 프로세스에 로드된 TSF 모듈이 하고, 설정은 이 폴더의 파일을 함께 읽는 방식이다. 그래서 파일을 직접 고치면 **약 2초 뒤 자동으로 반영**되고, 설정 창에서 바꾼 값은 즉시 적용된다. 반영이 안 되는 것 같으면 다른 창을 한 번 클릭했다가 돌아오면 된다.
>
> 제거(Q27)해도 이 폴더는 지워지지 않으므로, 재설치하면 설정이 그대로 이어진다.
<!-- @endplatform -->

---

## Q7. 자판은 어떤 게 있고, 새로 추가할 수 있나?

### 내장 한국어 자판

`ko_2bulstd`(두벌식 표준), `ko_3bul390`(세벌식 390), `ko_3bul391`(세벌식 391), `ko_3bul_noshift`(세벌식 순아래), `ko_3bul_anmatae`(안마태, 모아치기).

> 참고: 쿼티형 세벌식(`ko_3bul_qwerty`)은 빌트인이 아닌 연구 자료로만 보존된다.
> `docs/references/keymaps/ko_3bul_qwerty_v2.json`을 `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사하면 사용자 프로필로 활성화된다.

### 영어

`qwerty`, `dvorak`, `colemak`, `colemak_dh`, `workman`.

### 사용자 정의

<!-- @platform:linux -->
**🐧 리눅스**

`~/.config/unim/layouts/<name>.json`에 v1 스키마 JSON을 넣으면 데몬이 자동 스캔. `inherits: "ko_3bul390"` 같이 상속해 일부만 덮어쓸 수도 있음.

```bash
# 검증
unim-cli config layout validate ~/.config/unim/layouts/my.json
# 활성화
unim-cli config set korean-layout my
```
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

`%APPDATA%\unim\layouts\<name>.json` 에 v1 스키마 JSON을 넣으면 엔진이 자동 스캔한다. 스키마·`inherits`·`rule_sets`는 리눅스와 완전히 동일한 파일 형식이라, 리눅스에서 쓰던 자판 JSON을 그대로 복사해 와도 된다.

```powershell
# 폴더가 없으면 만든다
New-Item -ItemType Directory -Force -Path "$env:APPDATA\unim\layouts"
# 탐색기로 열어 JSON 을 넣는다
explorer "$env:APPDATA\unim\layouts"
```

- 명령줄 도구 `unim-cli` 는 **Windows 설치본에 들어 있지 않다.** 그래서 위의 `validate` / `set` 같은 CLI 검증·활성화 명령은 Windows에서 쓸 수 없다.
- 자판 선택은 **설정 창 → 일반 탭 → 자판** 에서 한다.
- 사용자 자판은 Windows 쪽에서 아직 손이 덜 탄 영역이다. 자판이 목록에 안 보이면 JSON 문법 오류일 가능성이 높으니, 리눅스 쪽에서 먼저 `unim-cli config layout validate` 로 검증한 파일을 가져오는 편이 확실하다.
<!-- @endplatform -->

스키마 상세는 [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](../../archive/plans/LAYOUT_PROFILE_V1.md).

> rule_sets로 같은 자판에 옵션 토글을 붙일 수도 있다. 예: `ko_3bul390`의 `sun_arae_batchim`(순아래받침). 설정 GUI에서 SwitchRow가 동적으로 나타난다.

---

<!-- @platform:linux -->
## Q8. UNIM의 메모리 사용량은 얼마나 되나?

정상 운영 시 `unim-daemon` RSS는 30~80 MB 수준. UNIM 0.2.0이 사용하는 안정화 장치:

- `#[global_allocator] tikv_jemallocator::Jemalloc` — glibc ptmalloc arena 폭주 차단.
- `Environment=MALLOC_ARENA_MAX=2` (systemd) — C 라이브러리 경로 이중 차단.
- 60초 주기 `libc::malloc_trim(0)` — OS에 메모리 반환 강제.

> 과거 RSS가 2 GB까지 부풀던 사건(0.1.x) 이후 회귀 금지로 못 박았다. RSS 500 MB+ 관측 시 [트러블슈팅 §14](../troubleshooting/README-ko.md#14-데몬이-메모리-너무-많이-먹음-rss-500mb)로.

---
<!-- @endplatform -->

## Q9. UNIM이 비밀번호 입력을 가로채는가?

**아니다.** 비밀번호 필드는 `content_purpose`로 식별되어 자동으로 영문 강제 전환된다. AutoTypeFix(순방향·역방향)·한자 변환·특수문자 팝업 모두 비활성화된다. 이미 쌓인 키 관측 버퍼·되돌리기 기록도 함께 지워져, `dkssud` 같은 비밀번호가 한글로 자동 교정돼 값이 깨지지 않는다. 입력값은 데몬 메모리에도 남기지 않는다.

> 단, 자동 검출은 앱이 `content_purpose=password`(리눅스) 또는 `InputScope`(Windows)를 정확히 보고할 때만 동작한다. 보고하지 않는 환경 — **XIM 레거시 앱, content-purpose 를 보내지 않는 일부 Wayland 컴포지터·웹폼** — 에서는 자동 감지가 안 될 수 있으니, 그런 곳에서는 직접 한/영 키로 영문 모드를 확인하길 권장한다. (정상 감지 환경: GTK3/4·Qt5/6·GNOME 확장·Windows TSF — 64비트·32비트 앱 모두 `unim_tsf32.dll` TSF TIP 이 동일한 `InputScope` 방식으로 감지한다. "Windows IMM32 앱은 ES_PASSWORD 표준 컨트롤만 최선노력 감지"라는 서술을 봤다면 실제 배포되지 않는 IMM32 폴백 이야기이니 무시해도 된다 — Q11 참고)

---

## Q9-1. 비밀번호 칸에서는 왜 자동 오타 교정이 안 되나?

**의도된 동작이다.** 비밀번호·PIN 칸에서는 자동 오타 교정을 일부러 끈다(Q9 참고). 켜 두면 `dkssud`처럼 친 비밀번호가 단어 경계에서 한글로 바뀌어 로그인이 깨지기 때문이다. 칸을 벗어나면 즉시 원래대로 돌아오고, 수동으로 켜고 꺼 둔 토글 상태도 그대로 유지된다.

> 반대로 **비밀번호가 아닌 일반 칸인데도 교정이 안 되는** 경우는 다른 원인이다 → [트러블슈팅](../troubleshooting/README-ko.md) §8. 위에 적은 미감지 환경(XIM·일부 Wayland)에서는 비밀번호 칸이 일반 칸으로 취급돼 오히려 교정이 발동할 수 있는데, 이 한계도 트러블슈팅 §8-1 에 정리돼 있다.

---

<!-- @platform:linux -->
## Q9-2. `install.sh`(한 줄 설치)는 안전한가요?

`curl -fsSL .../install.sh | bash` 는 편리하지만 "정체불명 스크립트를 통째로 실행"하는 것이므로 걱정될 수 있다. UNIM 설치 스크립트에는 다음 4가지 안전장치가 있다.

1. **SHA256 체크섬 검증** — 릴리스에 포함된 `SHA256SUMS` 를 매니페스트로 삼아 내려받은 모든 `.deb` 를 검증한다. 하나라도 불일치하면 **설치 단계로 넘어가지 않고 중단**한다(부분 설치 없음).
2. **임시 디렉토리 격리** — 모든 다운로드는 `mktemp` 로 만든 임시 디렉토리에 저장되고, 성공/실패/중단 어느 경우든 `trap` 으로 자동 삭제된다. 시스템에 남는 파일이 없다.
3. **apt 트랜잭션** — 설치는 `dpkg -i` 가 아니라 `apt install` 로 하므로, 외부 런타임 의존성까지 apt 의 원자적 트랜잭션으로 해결된다. 실패 시 부분 설치가 남지 않는다.
4. **스크립트 전문 공개** — 스크립트는 [main 브랜치](https://github.com/from104/unim/blob/main/install.sh)에 그대로 공개돼 있다. 실행 전에 `curl ... -o install.sh` 로 받아 직접 읽어볼 수 있다.

> 한계: `SHA256SUMS` 가 `.deb` 와 **같은 출처(GitHub Releases)**에 있으므로, 전송 무결성은 보장되지만 출처 인증은 GitHub 의 TLS 신뢰에 의존한다. GPG/minisign 서명은 향후 과제다. 최소 권한을 원하면 방법 2(수동 다운로드)로 각 파일을 직접 검증해 설치하면 된다.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q10. 0.1.x에서 0.2.0으로 업그레이드 시 주의?

대부분 자동. 단 두 군데 자동 정규화가 있다.

- `config.yaml`의 `korean.layout`이 enum 형식(`Dubeolsik`)이면 → `ko_2bulstd` 같은 문자열로 자동 변환.
- `english.layout`이 enum이면 → `qwerty` 등 문자열로 자동 변환.
- `typefix-blacklist.yaml`의 옛 키도 serde compat 레이어로 자동 정규화.

C-API: `UnimEnglishLayout`/`UnimKoreanLayout` enum 제거 → C 문자열 setter/getter로 변경. C/C++ 클라이언트 사용자만 영향.

자세한 마이그레이션은 [변경 이력 0.2.0](../../../CHANGELOG-ko.md#020-2026-04-26).

---
<!-- @endplatform -->

## Q11. UNIM은 macOS/Windows에서도 되나?

**Windows는 된다. macOS는 아직 안 된다.**

v0.4.0부터 Windows 10/11(64비트)을 TSF(Text Services Framework) 기반 `unim-tsf`로 지원한다.

```powershell
irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
```

로 MSI를 내려받아 설치한다([사용자 매뉴얼 §2.1 설치](../user-guide/README-ko.md#21-설치) 참고). 32비트 앱은 64비트 TSF 대신 별도의 32비트 TSF TIP(`unim_tsf32.dll`)이 처리한다. 예전에 검토됐던 IMM32 폴백(`unim-imm32` 크레이트)은 실제 배포 MSI에는 포함되지 않는 진단·연구용 소스로만 남아 있다 — "IMM32 폴백"을 배포 기능으로 안내하는 문서를 봤다면 오래된 것이다.

Windows 쪽은 개발자 상용 환경에서 매일 쓰이며 다듬어지고 있지만, 리눅스만큼 여러 머신·앱 조합을 거치지는 못했다. 문제를 만나면 [GitHub Issues](https://github.com/from104/unim/issues)로 앱 이름과 Windows 버전(`winver`)을 함께 보고해 주길 권한다.

macOS는 여전히 미착수다(로드맵 5단계). Rust 코어와 C-API가 분리돼 있어 이론적으론 macOS의 IMKit에 어댑터를 붙이면 되지만, 착수한 사람이 아직 없다. 자원자가 있다면 환영.

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

즉, 안마태 자판으로 모아치기를 하는 것이 UNIM 0.3.0의 첫 모아치기 지원이다. `supports_moachigi=true` 자판에서만 모아치기 설정 그룹이 설정 앱(`unim-settings`)에 나타난다.

---

<!-- @platform:linux -->
## Q16. popup-service가 안 뜨는 증상은 어떻게 진단하나?

`unim-popup-service`가 설치됐는지 먼저 확인한다.

```bash
busctl --user introspect org.atit.unim.PopupService /org/atit/unim/popup
```

응답이 없으면 패키지 미설치 또는 D-Bus 서비스 파일 누락이다. 자세한 진단은 [트러블슈팅 §16](../troubleshooting/README-ko.md#16-popup-service-디버깅-030).

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q17. deb에서 rpm으로 또는 그 반대로 마이그레이션할 때 설정이 유지되나?

예. 사용자 설정과 레이아웃은 `~/.config/unim/` 아래에 저장되며, 패키지 형식과 무관하다.

- `~/.config/unim/config.yaml` — 주 설정 파일
- `~/.config/unim/layouts/*.json` — 사용자 자판 프로필
- `~/.config/unim/typefix-blacklist.yaml` — AutoTypeFix 억제 사전
- `~/.config/unim/typefix-userdict.yaml` — 사용자 사전

패키지를 제거하고 다른 형식으로 재설치해도 위 파일들은 건드리지 않는다. 단, `unim-gui-qt` 패키지는 0.3.0에서 제거됐다 — 트레이 아이콘·설정창·팝업 렌더러는 지금 `unim-desktop`(인디케이터+레거시 설정창+`unim-popup-service` 묶음) 과 `unim-settings`(Slint 설정 앱) 두 패키지가 나눠 담당한다. 현재 배포되는 11개 패키지 전체 목록은 `debian/control` 또는 `dpkg -l 'unim*'` 로 확인.

---
<!-- @endplatform -->

<!-- @platform:linux -->
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
<!-- @endplatform -->

## Q18. chord_window_ms 적정값은 얼마인가?

범위는 **10–200ms**, 기본값은 **60ms**다. 60ms는 숙련자 기준으로 설계됐다.

| 프로필 | 권장 범위 | 설명 |
|--------|----------|------|
| 입문자 | 100–150ms | 키를 동시에 누르는 타이밍이 불안정할 때 넉넉하게 |
| 일반 | 60–100ms | 대부분의 사용자에게 편안한 범위 |
| 숙련자 | 10–60ms | 오입력 최소화, 반응 속도 우선 |

`chord_window_ms`를 0으로 설정하면 모아치기가 완전히 꺼진다.

<!-- @platform:linux -->
**🐧 리눅스** — 설정창의 슬라이더로 조정하거나 `unim-cli config set korean-chord-window-ms 80` 명령을 쓴다.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows** — **설정 창 → 일반 탭 → 모아치기** 의 슬라이더로 조정한다. 명령줄 도구 `unim-cli` 는 Windows 설치본에 없으므로 CLI로는 못 바꾸고, 대신 `%APPDATA%\unim\config.yaml` 을 직접 고쳐도 된다(약 2초 뒤 반영).
<!-- @endplatform -->

## Q20. 키를 오래 누르면 글자가 여러 번 입력돼요.

손 떨림 등으로 키를 오래 누르고 있으면 운영체제가 같은 키를 빠르게 반복 입력한다(자동 반복). UNIM에는 이 반복을 데몬이 무시하는 **조합키 자동반복 억제(접근성)** 옵션이 있다. 억제 대상은 **한/영 토글 키와 한글 모드의 문자 키**이며, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둔다. 기본값은 꺼짐이라 켜기 전에는 동작이 바뀌지 않는다.

<!-- @platform:linux -->
**🐧 리눅스**

켜는 법 — 설정 앱의 **접근성 → 조합키 자동반복 억제** 스위치, 또는 CLI:

```bash
unim-cli config set ignore-key-repeat true
```

**폴백의 한계**: Wayland·Qt5/6·GNOME 확장은 반복 여부를 정확히 가려낸다. 반면 GTK3/4·XIM·ibus 호환 경로는 80ms 시간창으로 근사 판정하므로 (1) 첫 반복 1회는 통과될 수 있고, (2) 시스템의 키 반복 간격을 80ms보다 길게 설정했다면 걸러지지 않을 수 있다. 어느 경우든 오판 시 항상 "덜 막는" 쪽으로 안전하게 동작한다. GNOME 확장 사용자는 재로그인 후 적용된다.
<!-- @endplatform -->

<!-- @platform:windows -->
**🪟 Windows**

켜는 법 — **설정 창**에서 조합키 자동반복 억제를 켠다. 설정 창을 열기 어렵다면 `%APPDATA%\unim\config.yaml` 을 직접 고쳐도 된다(약 2초 뒤 반영).

```yaml
engine:
  ignore_key_repeat: true
```

동작 방식은 **리눅스와 동일**하다 — 한/영 토글 키와 한글 모드의 문자 키만 대상이고, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둔다. 기본값은 꺼짐이다.

> 억제를 켜도 키를 오래 눌렀을 때 반복이 남는다면, Windows 자체의 키 반복 설정(**제어판 → 키보드 → 반복 지연/반복 속도**)을 함께 늦춰 보는 편이 확실하다. 두 설정은 서로 독립적이라 같이 써도 충돌하지 않는다.
<!-- @endplatform -->

---

<!-- @platform:linux -->
## Q21. Chrome/Chromium의 비밀번호 필드에서 한/영 차단이 안 될 때는?

**Chrome은 입력기를 보고하지 않으므로 UNIM이 자동 감지할 수 없습니다.**

### 원인별 진단

#### 1. Wayland Chrome (네이티브, `--enable-wayland-ime` 미활성화)

기본 상태에서는 Wayland 입력 메서드 프로토콜(`input-method-v2`)을 사용하지 않습니다. 플래그를 명시적으로 켜야 합니다.

**해결 방법:**

- **옵션 1 — 명령줄 플래그**
  ```bash
  google-chrome --enable-wayland-ime
  chromium --enable-wayland-ime
  ```

- **옵션 2 — 플래그 파일** (`~/.config/chrome-flags.conf`)
  ```
  --enable-wayland-ime
  ```

- **옵션 3 — .desktop 항목 (모든 사용자, 패키지 재설치해도 유지)**
  ```bash
  # /usr/share/applications/google-chrome.desktop 또는 ~/.local/share/applications/google-chrome.desktop
  # Exec= 라인을 찾아 끝에 --enable-wayland-ime 추가
  Exec=/opt/google/chrome/google-chrome --enable-wayland-ime %U
  ```

플래그를 켠 후 Chrome을 재시작하면 UNIM이 비밀번호 필드를 감지합니다.

#### 2. X11 Chrome / Chromium

Chromium 엔진은 **X11에서도 입력 필드 종류를 DBus 입력기에 보고하지 않습니다.** 이는 Chromium의 설계 선택이므로 UNIM 쪽에서 해결할 수 없습니다. 직접 한/영 토글(예: 한영 키)으로 영문 모드를 확인하시기 바랍니다.

> 대안: Firefox는 입력기에 필드 정보를 보고하므로 필드 감지가 정상 동작합니다.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q22. XIM 환경에서 비밀번호 필드 자동 감지가 안 될 때는?

**XIM 프로토콜 자체에 입력 필드 종류를 전달할 방법이 없습니다.**

XIM(X Input Method)은 1994년 설계된 레거시 프로토콜로, 비밀번호 같은 필드의 의미를 전달할 기능이 없습니다. 대신 다음 중 선택하시기 바랍니다:

1. **직접 영문 모드 확인** — 비밀번호 칸에 진입하기 전에 한/영 키를 눌러 영문 모드인지 확인.
2. **앱을 GTK/Qt 기반으로 전환** — XIM만 쓰는 레거시 앱을 최신 GTK/Qt 앱으로 바꾸기 (예: gvim → vim-gtk / nvim-qt).
3. **다른 입력 방식 시도** — 커맨드라인이라면 ibus 호환 경로도 함께 켜서 시도.

---
<!-- @endplatform -->

<!-- @platform:linux -->
## Q23. UNIM을 제거했더니 한/영 전환이 아예 안 된다 — 다른 IME로 되돌리려면?

**UNIM 제거는 시스템 입력기 지정을 자동으로 원래대로 되돌리지 않는다.** Q2 에서 안내한 대로 UNIM 을 설치하며 `sudo apt remove ibus` 로 IBus 를 지운 경우, `im-config -n unim` (또는 GNOME+Wayland 확장 활성화)으로 지정해 둔 "현재 입력기 = unim" 설정이 UNIM 패키지를 제거해도 그대로 남는다. 그 상태에서 재로그인하면 `run_im unim` 만 남고 정작 UNIM 은 없으므로 **어떤 IME 도 뜨지 않아 한/영 전환 자체가 안 되는 상태**가 된다.

### 제거 전에 할 일 (권장)

UNIM 을 제거하기 **전에** 다른 IME 로 먼저 되돌려 둔다.

```bash
# 예: ibus-hangul 로 되돌아가려면 먼저 설치
sudo apt install ibus ibus-hangul

# 입력기 지정을 되돌린다
im-config -n ibus
# 또는 사용 가능한 프로파일 중 자동 선택
im-config -n auto

# 그 다음에 UNIM 제거 (전 패키지 일괄 — 셸 glob 확장 사용)
sudo apt remove 'unim*'
```

### 이미 제거해서 한글 입력이 전혀 안 되는 경우

1. `im-config -n auto` 를 실행해 설치된 IME 중 하나로 자동 재지정한다. 아무 IME 도 안 깔려 있으면 `sudo apt install ibus ibus-hangul` 로 최소 하나를 설치한 뒤 다시 실행한다.
2. `~/.xinputrc` 를 직접 확인해 `run_im unim` 이 남아 있다면 지우거나 다른 IME 이름으로 바꾼다(GNOME+Wayland 세션에서는 이 파일이 아예 안 쓰일 수도 있다 — [사용자 매뉴얼 §2.2](../user-guide/README-ko.md#22-환경-변수-gnome-확장을-안-쓰는-모든-데스크톱) 참고).
3. 로그아웃 후 다시 로그인.

> 이 제거·롤백 경로는 UNIM 패키지 스크립트가 아니라 데비안/우분투의 `im-config` 프레임워크가 관리하는 영역이라, UNIM 쪽에서 자동으로 원복해 주지 않는다. 위 수동 절차가 현재의 유일한 복구 방법이다.
<!-- @endplatform -->

<!-- @platform:windows -->
## Q24. 설치했는데 입력기 목록에 UNIM이 안 보입니다.

**🪟 Windows** — **설치 직후에 안 보이는 건 정상이다.** TSF(Text Services Framework, 윈도우의 입력기 규격) 입력기는 OS가 세션을 시작할 때 읽어 들이므로, **재부팅하거나 로그오프 후 다시 로그인**해야 목록에 나타난다.

재로그인해도 안 보이면 위에서부터 순서대로 확인한다.

1. **한국어가 설치돼 있는지** — 설정 → 시간 및 언어 → 언어 및 지역 에 **한국어**가 있어야 UNIM이 그 아래에 붙는다. 없으면 한국어를 먼저 추가한다.
2. **키보드를 직접 추가** — 설정 → 시간 및 언어 → 언어 및 지역 → **한국어 → 키보드 → 키보드 추가** → `UNIM Korean IME`.
3. **입력기 재등록** — 설치 폴더(`C:\Program Files\UNIM\`)의 `register-tsf.bat` 를 **오른쪽 클릭 → 관리자 권한으로 실행**. 되돌리려면 같은 폴더의 `unregister-tsf.bat`. 이 두 스크립트는 입력기 등록만 손대고 파일이나 설정은 지우지 않는다.

> 잘 붙었다면 작업표시줄 시계 옆에 현재 모드가 글자로 보인다 — 한글이면 `한`, 영문이면 `A`. **왼쪽 클릭은 한/영 전환**, **오른쪽 클릭은 메뉴**(한/영 전환 · 기본 입력기로 설정 · 설정 열기)다.

---

## Q25. 설치할 때 "Windows의 PC 보호" 경고가 뜹니다.

**🪟 Windows** — 현재 배포되는 MSI는 **코드 서명이 되어 있지 않다.** 그래서 SmartScreen이 "알 수 없는 게시자"로 보고 경고를 띄운다. 계속하려면 경고 창의 **추가 정보 → 실행**을 누른다.

- 코드 서명은 예정된 과제지만 아직 인증서를 확보하지 않았다. 그리고 서명한 뒤에도 **경고가 곧바로 사라지지는 않는다** — SmartScreen 평판은 다운로드가 쌓이면서 붙는 것이라, 신규 게시자는 초기 구간에 경고가 남는다.
- 그때까지 무결성 확인은 **SHA256 대조**로 한다. 릴리스에 함께 올라오는 `SHA256SUMS-msi` 의 값과 비교하면 된다.

  ```powershell
  Get-FileHash .\unim-0.4.1-x64.msi -Algorithm SHA256
  ```

- `install.ps1` 로 설치하면 이 대조를 스크립트가 자동으로, 그것도 두 번 해 준다(Q26).

---

## Q26. `install.ps1`(한 줄 설치)는 안전한가요?

**🪟 Windows** — `irm ... | iex` 도 리눅스의 `curl | bash` 와 똑같은 걱정거리다. Windows 설치 스크립트에는 다음 안전장치가 있다.

1. **SHA256 검증을 두 번** — 다운로드 직후 릴리스의 `SHA256SUMS-msi` 와 1차 대조하고, 관리자 권한으로 승격한 뒤 **설치를 실행하기 직전에 다시 해시**해 2차 대조한다. 한 번이라도 불일치하면 **아무것도 설치하지 않고 중단**한다.
2. **권한을 최소로** — 스크립트는 **관리자가 아닌 상태로 시작**해 다운로드와 검증까지 마치고, MSI 실행 단계에서만 UAC(관리자 권한 확인 창)로 승격한다. UAC 창은 한 번만 뜬다.
3. **차단 해제는 검증 뒤에** — 인터넷에서 받은 파일에 붙는 차단 표시(MOTW) 제거는 1차 검증을 통과한 파일에만 적용한다.
4. **스크립트 전문 공개** — [main 브랜치](https://github.com/from104/unim/blob/main/install.ps1)에 그대로 공개돼 있다. 실행 전에 받아서 읽어 볼 수 있고, 설치할 버전을 직접 고정할 수도 있다.

   ```powershell
   # 먼저 내려받아 읽어 본 뒤 실행
   powershell -ExecutionPolicy Bypass -File .\install.ps1 -Check
   # 특정 버전 고정
   $env:UNIM_VERSION='v0.4.1'; irm https://raw.githubusercontent.com/from104/unim/main/install.ps1 | iex
   ```

`-Check`(설치 버전과 최신 버전만 보고, 아무것도 안 바꿈) · `-Update`(최신으로 갱신) · `-Force` 옵션이 있다.

> 한계: MSI 자체가 미서명이라 SmartScreen 경고는 그대로다(Q25). 또 `SHA256SUMS-msi` 가 MSI와 **같은 출처(GitHub Releases)**에 있으므로, 전송 무결성은 보장되지만 출처 인증은 GitHub의 TLS 신뢰에 의존한다 — 리눅스판과 같은 한계다.

---

## Q27. Windows에서 UNIM을 완전히 제거하려면?

**🪟 Windows** —

1. **먼저 글을 입력 중인 앱을 모두 닫는다.** 입력기 모듈은 한 번이라도 한글을 친 앱 안에 로드돼 있어서, 열려 있으면 파일 삭제가 다음 재부팅으로 미뤄질 수 있다.
2. **설정 → 앱 → 설치된 앱** 에서 `UNIM` 을 제거한다. 시작 메뉴 **UNIM → Uninstall UNIM** 단축키도 같은 일을 한다.
3. 제거가 끝나면 **재로그인하거나 재부팅**한다.

제거하면 설치 폴더(`C:\Program Files\UNIM\`)·입력기 등록 정보·시작 메뉴 단축키가 함께 사라진다. 다만 **사용자 설정(`%APPDATA%\unim\`)은 남는다** — 재설치하면 설정·억제 사전·사용자 사전이 그대로 이어진다. 완전히 지우고 싶으면 이 폴더를 직접 삭제한다.

> 리눅스처럼 "시스템 입력기 지정을 원래대로 되돌리는" 별도 절차(Q23)는 없다. Windows에서 UNIM은 입력기 목록에 **추가**됐을 뿐 기존 입력기를 지우지 않기 때문이다. 제거 후 한글 입력이 이상하면 **설정 → 시간 및 언어 → 한국어 → 키보드** 에서 남아 있는 입력기를 확인한다.

---

## Q28. 한자·특수문자·이모지 팝업이 안 뜹니다.

**🪟 Windows** — Windows에서 팝업은 `unim-popup-win.exe` 라는 **별도 프로그램**이 그린다. 이게 떠 있지 않으면 팝업만 안 나오고 한글 입력 자체는 멀쩡한, 딱 이 증상이 된다.

1. **키를 먼저 확인** — `한자` 키(또는 `F9`)는 상황에 따라 세 갈래로 갈린다.
   - 한글을 조합한 상태 → **한자** 후보
   - 초성 하나만(예 `ㅁ`) 조합한 상태 → **특수문자**
   - 아무것도 조합하지 않은 상태 → **이모지**
2. **프로그램이 떠 있는지 확인** — 작업 관리자에 `unim-popup-win.exe` 가 있는지 본다. 없으면 설치 폴더(`C:\Program Files\UNIM\`)의 `unim-popup-win.exe` 를 직접 실행한다. 실수로 두 번 실행해도 중복으로 뜨지 않는다(스스로 종료한다).
3. **한 번 재로그인** — 이 프로그램은 로그인할 때 자동으로 시작하도록 등록돼 있다. 방금 설치했다면 재로그인이 가장 확실하다.

> 팝업 조작키(9×9 격자 토글 `.`, 북마크 `Space`, 열 점프 `Q`~`O`, 카테고리 `A`~`L`)는 리눅스와 같게 맞춰 뒀다. 다르게 동작하면 [GitHub Issues](https://github.com/from104/unim/issues)로 알려 주면 된다.

---

## Q29. MS Word에서 글자가 한 자씩이 아니라 단어 단위로 확정됩니다.

**🪟 Windows** — **의도된 동작이다.** 앱에 따라 조합 중인 글자를 다루는 방식이 달라서, UNIM은 일부 앱에서 글자 단위가 아니라 **단어 단위로 확정**한다(스마트 확정 단위). 기본 대상은 `winword.exe`(MS Word)와 `wmux.exe` 두 개다.

- 다른 앱도 이 방식으로 쓰고 싶으면 **설정 창의 "단어 모드 앱"** 에 실행 파일 이름을 추가한다. Windows에만 있는 설정이다.
- 반대로 Word에서 글자 단위 조합을 쓰고 싶다면 목록에서 `winword.exe` 를 빼면 된다.

---

## Q30. 카카오톡·한컴 같은 32비트 앱, WezTerm·텔레그램에서도 되나요?

**🪟 Windows** — 된다.

- **32비트 앱**: 64비트 앱은 `unim_tsf.dll` 이, 32비트 앱은 별도의 32비트 입력기 `unim_tsf32.dll` 이 맡는다. 설치본이 둘 다 등록하므로 카카오톡·한컴처럼 32비트로 도는 앱에서도 한글이 입력된다.
- **콘솔·IMM32 계열 앱**: WezTerm·텔레그램처럼 한글 조합이 깨지던 앱들은 v0.4.0에서 CUAS(윈도우의 구형 입력 호환 계층) 규약을 따르도록 고쳐 정상 조합된다.
- 예전에 검토됐던 **IMM32 `.ime` 등록 갈래는 폐기했다.** "IMM32 폴백"을 배포 기능으로 안내하는 문서를 봤다면 오래된 것이다(Q11 참고).

특정 앱에서만 안 되면 그 **앱 이름과 32/64비트 여부**를 함께 적어 [GitHub Issues](https://github.com/from104/unim/issues)로 알려 주면 진단이 훨씬 빠르다.

---

## Q31. Windows판은 리눅스판과 기능이 같나요?

**🪟 Windows** — **입력의 알맹이는 같고, 주변 도구가 다르다.** 한글 조합, 자판 선택, 한자·특수문자·이모지 팝업, AutoTypeFix(순방향·역방향·학습), 억제 단어, 사용자 사전은 **같은 Rust 코어**를 그대로 쓴다. 차이는 이 표가 전부다.

| 항목 | 리눅스 | Windows |
|------|--------|---------|
| 설정 창 | 설정 앱 (`unim-settings`) | 설정 앱 (`unim-settings.exe`) — 일반 / 오타 교정 / 억제 단어 / 사용자 사전 4탭 |
| 설정 반영 | 데몬이 즉시 전파 | 설정 창에서 바꾸면 즉시, 파일을 직접 고치면 약 2초 뒤 |
| 명령줄 도구 `unim-cli` | 있음 | **없음** (설치본에 포함되지 않음) |
| 자판 스튜디오 · 타자 연습 | 있음 | **없음** |
| GNOME 확장 | 있음 | 해당 없음 |
| 현재 모드 표시 | 트레이 인디케이터 | 작업표시줄 입력 표시 (`한` / `A`) |

- **Windows 판은 리눅스보다 이력이 짧다.** 검증을 거친 앱의 폭이 좁아, 드문 앱에서는 문서와 실제가 다를 수 있다.
- 오프라인 도움말(지금 읽고 있는 이 문서)은 설치본에 함께 들어 있다 — **설정 창의 도움말 버튼**으로 언제든 열 수 있다.
- 리눅스에서 쓰던 설정을 옮기고 싶다면 `~/.config/unim/` 의 파일을 `%APPDATA%\unim\` 로 복사하면 된다. 파일 형식은 양쪽이 같다(Q6 · Q7).

<!-- @endplatform -->

---

## 더 읽을 거리

- [사용자 매뉴얼](../user-guide/README-ko.md)
- [트러블슈팅](../troubleshooting/README-ko.md)
- [변경 이력](../../../CHANGELOG-ko.md) — 모든 판의 상세 내역
- [릴리스 페이지](https://github.com/from104/unim/releases)
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md)
- [`docs/dev/specs/POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)
