# UNIM FAQ (한국어)

> UNIM 0.2.0에 대해 사람들이 정말 자주 묻는 질문 모음.
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

### 내장 (0.2.0 기준 한국어 10종)

`ko_2bulstd`(두벌식 표준), `ko_3bul390`(세벌식 390), `ko_3bul391`(세벌식 391), `ko_3bul_noshift`(세벌식 순아래), `ko_3bul_qwerty`(세벌식 쿼티형), 그 외 변종 5종.

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

스키마 상세는 [`docs/dev/plans/LAYOUT_PROFILE_V1.md`](../../dev/plans/LAYOUT_PROFILE_V1.md).

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

**아니다.** 비밀번호 필드는 `content_purpose`로 식별되어 자동으로 영문 강제 전환된다. AutoTypeFix·한자 변환·특수문자 팝업 모두 비활성화. 입력값은 데몬 메모리에도 남기지 않는다.

> 단, 자동 검출은 앱이 `content_purpose=password`를 정확히 보고할 때만 동작. 일부 웹폼이 보고하지 않는 경우는 사용자가 직접 한/영 키로 영문 모드 확인 권장.

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

## 더 읽을 거리

- [사용자 매뉴얼](../user-guide/README-ko.md)
- [트러블슈팅](../troubleshooting/README-ko.md)
- [릴리즈 노트 0.2.0](../release-notes/0.2.0/RELEASE_NOTES-ko.md)
- [`IME_BEHAVIOR.md`](../../dev/architecture/IME_BEHAVIOR.md)
- [`docs/dev/specs/POPUP_SPEC.md`](../../dev/specs/POPUP_SPEC.md)
