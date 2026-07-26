# Changelog

<!-- markdownlint-disable MD024 -->

UNIM(Universal Next-generation Input Method) 프로젝트에 대한 모든 주목할만한 변경 사항은 이 파일에 기록됩니다.

형식은 [Keep a Changelog (korean)]를 기반으로 하며 이 프로젝트는 [Semantic Versioning (korean)]을 따릅니다.

## [Unreleased]

---

## [0.4.0] 2026-07-19

이 릴리스는 한 줄 설치(`curl … | bash`), 첫 실행 설정 마법사, Slint 기반 크로스플랫폼 설정앱, 자판 스튜디오·타자 연습 도구, 단어 단위 입력, 그리고 대규모 Windows 포팅(실험적)을 담았습니다.

### ✨ 새 기능

- **원클릭 설치 (`curl … | bash`)**: 이제 한 줄 명령으로 UNIM을 설치할 수 있습니다.

  ```bash
  curl -fsSL https://raw.githubusercontent.com/from104/unim/main/install.sh | bash
  ```

  스크립트는 리눅스·amd64·apt 환경을 확인한 뒤, 최신 GitHub 릴리스에서 deb 11종을 내려받아 `SHA256SUMS` 매니페스트로 무결성을 검증하고 `apt-get`으로 설치합니다. 설치 후 재로그인과 첫 실행 마법사 안내가 이어집니다. 저장소 환경 변수(`UNIM_VERSION`, `UNIM_BASE_URL`)로 특정 버전 고정·미러 지정도 가능합니다.

- **첫 실행 설정 마법사**: 설치 후 처음 로그인하면 마법사가 자동으로 실행되어, UNIM을 기본 입력기로 설정합니다(im-config, 실패 시 xinputrc 폴백). 기본 설정에 실패하면 오류 카드로 안내하고 완료를 막습니다. 한 번 완료하면 다시 뜨지 않습니다(XDG state에 기록).

- **Slint 설정앱 (`unim-settings`)**: 설정앱이 GTK4에서 Slint 기반 크로스플랫폼 앱으로 새로 작성되어, 리눅스와 Windows가 동일한 설정 화면을 공유합니다. 리눅스에서는 기존 GTK 설정앱과 똑같이 `config.yaml`에 저장하고 DBus로 데몬에 즉시 통지하며, 이미 창이 떠 있으면 중복 실행 대신 기존 창을 알립니다. 폰트가 없는 배포판에서도 한글이 깨지지 않도록 시스템 폰트를 조회합니다.

- **한/영 전환 소리(비프)**: 한/영 모드가 바뀔 때 짧은 소리로 알려줍니다(한글 880Hz, 영문 440Hz). 외부 라이브러리나 사운드 파일 없이 동작하며 입력을 지연시키지 않습니다. 키 토글, AutoTypeFix 자동 전환, 트레이·확장에서의 전환 모두에 울립니다. 설정에서 끌 수 있습니다.

- **키 자동 반복 무시(접근성)**: 키를 오래 누르고 있어서 생기는 자동 반복(연타)을 데몬이 무시하도록 켤 수 있습니다. 억제 대상은 한/영 토글 키와 한글 모드의 문자 키이고, 백스페이스·방향키 같은 편집키와 영문 직접 입력의 반복은 그대로 둡니다. 손 떨림 등으로 키를 오래 누르게 되는 지체장애 사용자를 위한 기능입니다. **기본값은 꺼짐**이라 켜기 전에는 동작이 전혀 바뀌지 않습니다. 설정 앱의 「조합키 자동반복 억제」 스위치나 `unim-cli config set ignore-key-repeat true` 로 켭니다. Wayland·Qt5/6·GNOME 확장에서는 반복 여부를 정확히 가려내고, GTK3/4·XIM·ibus 호환 경로에서는 80ms 시간창으로 근사 판정합니다(첫 반복 1회는 통과될 수 있고, 시스템 키 반복 간격을 80ms보다 길게 잡았다면 걸러지지 않을 수 있음 — 어느 경우든 "덜 막는" 쪽으로 안전하게 동작).

- **자판 스튜디오 · 타자 연습 (신규 GTK4 도구)**: 한글 자판을 한 자리에서 보고·수정·연습할 수 있는 두 개의 독립 앱이 추가되었습니다.
  - **자판 스튜디오(`unim-keymap-studio`)**: 빌트인·사용자 자판의 키 배치와 초·중·종성 조합을 표로 보고, 키를 클릭해 편집한 뒤 사용자 자판(`~/.config/unim/layouts/*.json`)으로 저장합니다.
  - **타자 연습(`unim-typing-practice`)**: 길이별 예문을 골라 연습하며 실시간 WPM·CPM·정확도·오타율을 보여주고, 끝나면 키별 오타 히트맵을 표시합니다. 한글 음절을 자모로 분해해 키 입력 수를 세며, WPM은 한국 표준 방식(CPM÷5)으로 계산합니다.

- **자동 영문 전환 — Ctrl/Alt/Super 조합 트리거**: 자동 영문 트리거에 수정자 조합을 쓸 수 있습니다(예: `key:Ctrl+B`, `key:Ctrl+Shift+B`, `key:Alt+F1`, `key:Super+Space`). tmux/wmux의 prefix(`Ctrl+B`)처럼 조합키가 필요한 환경에 맞춰, 한글 모드에서 해당 조합을 누르면 영문 모드로 전환하고 그 키는 앱에 그대로 전달합니다. 표기한 수정자가 정확히 눌렸을 때만 발동합니다. GTK·Qt·XIM·GNOME에서 동작하며, Windows 지원은 후속 작업입니다.

- **AutoTypeFix 토글 단축키 3종**: 자동 오타 교정을 단축키로 켜고 끌 수 있습니다 — 전체 / 순방향(영문→한글)만 / 역방향(한글→영문)만 각각 지정 가능. **전체 토글의 기본값은 `Shift+F8`** 이고(F9~F11 은 미디어 키·리매핑으로 소실되는 키보드가 있어 피했습니다), 순방향·역방향 전용은 비어 있습니다(필요한 사람만 지정). 단축키에는 수정자 조합을 쓸 수 있으며(`Shift+F8`, `Ctrl+Left` 등) 표기한 수정자가 정확히 눌렸을 때만 발동하므로, 설정하지 않은 조합(예: `Shift+F10` 컨텍스트 메뉴)은 앱으로 그대로 전달됩니다. 맨 `F9`는 종전대로 한자/이모지 팝업입니다. 키를 누르고 있어도(오토리핏) 떨림 없이 한 번만 토글됩니다. 쓰지 않으려면 목록을 비우면 됩니다. 구분자는 `+` 가 정식이며 `+` 가 없는 표기에서는 `-` 도 허용됩니다(`Ctrl-F8` = `Ctrl+F8`). 잘못된 표기는 저장 시 CLI·설정 앱이 경고하고 데몬 로그에도 기록됩니다 — 조용히 무시되지 않습니다. GNOME(Wayland)에서는 `Ctrl+Left` 같은 Ctrl/Alt/Super 조합도 동작합니다(확장이 토글 조합을 엔진으로 전달). Windows(TSF/IMM32)에서도 조합 표기가 리눅스와 동일하게 동작합니다 — 플랫폼 구분 없이 기본값 `Shift+F8` 그대로 쓰면 됩니다.

- **비밀번호 필드 자동 보호**: 비밀번호 등 민감 입력란에 들어가면 UNIM이 자동으로 영문 모드로 전환하고, 벗어나면 이전 상태로 되돌립니다. 이때 눌린 키는 버퍼·실행취소·최근 교정·학습 사전·surrounding 문맥 어디에도 남지 않습니다. Wayland·GTK·Qt·Windows(TSF — 포커스 유지 중 필드 성격이 바뀌는 경우까지 추적)에서 동작합니다. Windows IMM32 는 최선노력 감지입니다(`ES_PASSWORD` 스타일의 표준 Edit/RichEdit 컨트롤만 — 직접 그리는 커스텀 비밀번호 칸은 감지 불가). XIM 레거시 앱은 감지되지 않으며, 한계는 FAQ·트러블슈팅에 명시했습니다.

- **단어 단위 입력 (`commit_unit`)**: 조합 확정 단위를 음절 대신 단어로 둘 수 있습니다. 단어 모드에서는 교정 후에도 단어 전체가 조합 상태로 유지됩니다. 단, 터미널·XIM·ibus 계열 프런트엔드와 모아치기 자판에서는 오작동을 막기 위해 자동으로 음절 단위로 강등됩니다(WordGate). CLI(`unim-cli config`)와 설정앱에서 선택할 수 있습니다.

- **4개 앱 고유 아이콘 + 역DNS 명명 정합**: 인디케이터·설정·자판 스튜디오·타자 연습 앱이 각각 고유 아이콘을 가집니다. 앱 ID가 역DNS 명명으로 통일되어(앱 ID == `.desktop` 파일명 == 아이콘 이름, 예: `io.github.from104.unim.Settings`), GNOME Wayland 작업표시줄·오버뷰에 아이콘이 정상 표시됩니다. 트레이 한/영 상태 아이콘은 그대로 유지됩니다.

### 🔄 달라진 점

- **설정앱 이원화 — Slint 정식, GTK 레거시**: 새 Slint 설정앱이 `unim-settings`라는 이름을 넘겨받고, 기존 GTK4 설정앱은 `unim-settings-gtk`로 개명되어 당분간 함께 배포됩니다(추후 퇴역 예정). 데스크톱 메뉴에는 Slint 앱만 노출되고 GTK 앱은 숨겨지며, 트레이·GNOME 확장·타자 연습 등 모든 컴포넌트의 "설정 열기"가 새 앱을 가리킵니다.

- **deb 패키지 11종으로 개편**: 패키지 구성이 정리되었습니다. 설정앱이 `unim-settings`(Slint)와 `unim-settings-gtk`(레거시)로 나뉘고, `unim-keymap-studio`·`unim-typing-practice`가 새 패키지로 추가되었습니다. 인디케이터·설정·팝업 서비스는 `unim-desktop`으로 묶였습니다.

- **자판 스튜디오 재설계**: 기존 좌측 사이드바 + 2탭 구성이 헤더 3단 드롭다운(언어 › 출처 › 자판) + 4탭(기본 / 자판 / 조합 / 확장) 구성으로 전면 개편되었습니다. 빌트인 자판은 보호되어("다른 이름으로 저장"만 가능), 사용자 자판은 제자리에 저장됩니다. `unim-keymap-studio/README.md` 참고.

- **자판 목록을 동적으로 열거**: 설정앱의 자판 목록이 정적인 4종 고정 목록에서 실제 등록된 프로필(`ProfileRegistry`) 열거로 바뀌어, 안마태와 사용자 자판이 친화적인 이름으로 표시되고 모아치기 관련 UI가 함께 활성화됩니다. CLI의 자판 검증도 같은 소스를 씁니다.

- **GNOME 확장 아이콘 정비**: 트레이/패널 아이콘이 단색 SVG 세트로 교체되고, 입력기 비활성(`unim-disabled`) 상태가 별도 아이콘으로 표시됩니다.

- **잘못된 단축키 표기 경고 — 모든 키 설정 필드로 확대**: AutoTypeFix 토글 단축키 3종에만 있던 "저장할 때 검증하고 경고" 가 **한/영 전환키·한자 키·자동 영문 트리거**까지 확대되었습니다. 엔진이 해석하지 못하는 표기(오타, 지원하지 않는 조합 등)를 넣으면 이제 ① 설정 앱 상태줄(모든 저장 경로에서 일관되게 표시), ② `unim-cli config set` 의 경고 메시지(문법 오류·중복·문자·편집 키 등 무효한 지정 포함), ③ 데몬 로그 세 곳에서 알려 줍니다. 설정 앱에서는 저장할 때마다 무효 키가 있으면 저장 확인 문구 대신 경고를 표시하므로, 다른 저장 문구에 가려지지 않습니다. GNOME 확장·레거시 설정 앱이 쓰는 D-Bus `SetConfig` 경로에도 같은 진단 로그가 남아, 포커스된 창이 없어 엔진이 아직 설정을 읽지 않은 상태에서도 곧바로 확인할 수 있습니다. 정책은 부분 무효까지는 **"저장은 하되 경고"** 이고, 예외가 하나 있습니다 — 목록의 **전 항목**이 무효라면 엔진이 전부 걸러 그 기능이 통째로 죽으므로(예: 한/영 전환 수단 소실), 빈 목록과 동일하게 저장을 반려합니다. 이 반려는 CLI·D-Bus·설정 앱 세 창구에서 동일하게 적용됩니다. 판정은 전부 엔진 파서 한 곳에서 이뤄지므로 리눅스와 Windows 의 경고 기준이 완전히 동일합니다. 참고로 한/영 전환키·한자 키는 조합 표기(`Ctrl+X`)를 지원하지 않고 단일 키 이름만 받으며(예: `Korean`, `RightAlt`, `Hanja`, `F9`), 자동 영문 트리거는 `key:Escape`·`char:/`·`key:Ctrl+B` 형식을 씁니다.

### 🐛 버그 수정

- **도움말이 브라우저 대신 다른 앱으로 열리던 문제**: 트레이·설정 앱·GNOME 확장의 「도움말」이 text/html 기본 핸들러를 그대로 따라가서, VS Code 계열 IDE 가 text/html 을 자기 앱으로 등록한 시스템에서는 매뉴얼이 브라우저 대신 IDE 로 열렸습니다. 이제 사용자의 **기본 웹 브라우저**를 명시적으로 우선하고, 브라우저가 없는 환경에서만 종전 방식(MIME 핸들러)으로 폴백합니다.
- **오른쪽 Alt(RightAlt) 한/영 토글이 모든 환경에서 동작**: 두 겹의 문제로 오른쪽 Alt를 한/영 토글로 지정해도 동작하지 않았습니다 — 엔진에서는 수정자 키 판정이 토글 키 검사보다 먼저 처리됐고(종성 조합 회귀 테스트와 함께 수정), 프런트엔드에서는 GTK3/4·Qt5/6·GNOME 확장이 오른쪽 Alt를 자체적으로 걸러내 아예 데몬까지 오지 않았습니다(XIM·순수 Wayland는 원래 동작). 이제 프런트엔드의 자체 스킵을 없애고 토글 판정을 데몬으로 일원화해 어디서든 동일하게 동작합니다. AltGr(`ISO_Level3_Shift`)은 계속 통과시키므로 AltGr 레이아웃에는 영향이 없고, GNOME은 이벤트를 소비하지 않고 알리기만 하므로 Sticky Keys 등 접근성 기능도 그대로입니다. 다만 토글하는 순간 앱도 Alt 눌림/뗌을 함께 받을 수 있어(예: 일부 앱의 메뉴바 포커스), 원치 않으면 `toggle_keys`에서 RightAlt를 빼 옵트아웃할 수 있습니다.

- **Super/Meta 조합 키 인식 (GTK/Qt immodule)**: GTK3/4·Qt5/6 입력 모듈의 Super/Meta 수정자 마스크 비트가 어긋나 있어, `key:Super+X` 같은 조합 트리거가 이 경로에서 동작하지 않던 문제를 바로잡았습니다(엔진의 X11 마스크 해석과 정합).

- **단어 모드 음절 강등 안내 (WordGate)**: 단어 단위 입력이 터미널 등에서 음절 단위로 자동 강등될 때 "설정이 고장 났다"고 오인하지 않도록, 판정 결과를 로그(`[WordGate]`)로 남기고 설정앱·CLI 설명에 상시 음절 예외를 명시했습니다.

- **단축키 칸이 존재하지 않는 키를 예시로 안내하던 문제**: 설정 앱의 오타 교정 단축키 입력란 안내(`예: ScrollLock, F10`)와 CLI 경고 문구가 UNIM 이 인식하지 않는 `ScrollLock`·`Pause` 를 권장하고 있어서, 안내대로 적으면 그 단축키가 조용히 죽었습니다. 예시를 실제로 유효한 표기(`F10`, `Shift+F9`)로 바로잡았습니다. 같은 이유로 한/영 전환키·한자 키 입력란의 예시(`Hangul`, `Control+Enter`)도 인식되지 않는 표기였어서 `Korean`, `RightAlt` / `Hanja`, `F9` 로 교체했습니다. 아울러 오타 교정 단축키 설명에 남아 있던 "수정자 조합은 지원하지 않습니다" 라는 옛 문구도 정정했습니다 — 이번 릴리스부터 `Shift+F8` 같은 조합 표기가 정식 지원됩니다.

### 🪟 Windows 지원 (실험적)

이번 개발 주기에 Windows 포팅이 크게 진행되었습니다. 실기기 검증이 진행 중인 실험적 지원으로, 아래는 구현된 범위입니다.

- **TSF 완전 네이티브 아키텍처**: 모든 UI를 단일 `unim_tsf.dll`에 통합하고 별도 도우미 실행 파일(`unim-windows`)을 제거했습니다. 한자·특수문자·이모지 팝업(9×9 격자·즐겨찾기·페이지 이동), AutoTypeFix(순방향·역방향·수동·억제 목록·실행취소), 설정, 언어바를 포함합니다. 공유 코어·리눅스 프런트엔드는 수정하지 않았습니다.

- **콘솔·IMM32 앱 한글 조합 (CUAS 계약 준수)**: WezTerm·텔레그램 등 인라인 조합을 쓰는 콘솔/IMM32 앱에서 한글 조합이 복구되었습니다.

- **32비트 앱 지원 (카카오톡·한컴 등)**: 32비트 앱이 64비트 전용 TIP를 찾지 못하던 문제를 해결하기 위해 32비트 TSF TIP(`unim_tsf32.dll`)를 함께 등록합니다. Win11에서 무의미한 IMM32 `.ime` 등록은 폐기했습니다.

- **접근성**: 조합·후보 창을 TSF UIA/UILess로 노출하고, 조합키 자동반복 억제 옵션(`ignore_key_repeat`, 지체장애 사용자용)과 한/영 전환 화면 낭독기 통지(NotifyWinEvent, 옵션 비프)를 추가했습니다.

- **MSI 배포**: windows-rs 0.62.2로 올리고 WiX 3.x 기반 MSI 빌드 체인을 정비했습니다.

### 🧹 내부 정리

- **unim-capi 헤더 동기화**: 공개 C 헤더(`unim.h`)를 Rust 표면과 동기화하고(`UnimInputResult` 2필드, `POPUP_KEY_PERIOD` 상수 추가), 헤더가 어긋나면 빌드 시 자동으로 경고하는 가드(`build.rs` + cbindgen)를 추가했습니다.
- **키맵 도구 키보드 위젯 공유**: 자판 스튜디오와 타자 연습이 5행 키보드 위젯 하나를 공유하도록 정리하고(중복 3벌 → 1벌), 죽은 코드(구식 `KeyboardWidget`·`ProfileSidebar` 등)를 제거했습니다.
- **프런트엔드 정리**: GTK/Qt IM 모듈에서 쓰이지 않던 임베디드 팝업 위젯을 제거했습니다(모든 팝업은 popup-service가 전담). 프런트엔드의 unim-capi 링크 의존도 해제했습니다.
- **줄바꿈 정규화**: `.gitattributes`(eol=lf)와 `.editorconfig`를 도입해 리눅스·Windows 혼용 개발에서 줄바꿈이 일관되게 유지됩니다.

---

## [0.3.0] 2026-05-19

이 릴리스는 모아치기(동시 자모 입력) 기능, 팝업 마우스 조작 전면 개편, 즐겨찾기(★), 단일 통합 설정 창, 그리고 UNIM 최초의 빌트인 모아치기 자판 안마태를 선보입니다.

### ✨ 새 기능

- **안마태(Ahnmatae 2003) 자판 기본 내장**: 2003년 안마태 신부(Matthew Y. Ahn)가 설계한 한손 모아치기 세벌식 자판이 빌트인으로 추가됩니다. 여러 자모를 동시에 눌러 한 음절을 입력하는 방식으로, 설정 → 자판에서 선택할 수 있습니다.

- **모아치기(동시 자모 입력) 엔진 v4**: 안마태 등 모아치기 지원 자판을 선택하면, 여러 자모를 동시에 눌렀을 때 UNIM이 음절로 조합해 줍니다. 기본 chord 창은 60ms이며, 처음 쓰는 분께는 100~150ms를 권장합니다. 설정 → 자판 → 동시 입력 시간(ms) 슬라이더로 조정할 수 있고, 0으로 설정하면 모아치기가 꺼집니다.
  - **자모 역순 결합** (기본 OFF): 켜면 초·중·종성 결합을 두 가지 순서로 모두 시도해, 손가락이 어느 순서로 닿아도 올바른 음절이 완성됩니다. 이 옵션은 순차 입력(시간 간격을 두고 누르는 일반 타이핑)에도 함께 적용됩니다.
  - 모아치기 도중이나 직후 Backspace를 누르면 자모를 하나씩 지우고 남은 자모로 음절을 재합성합니다. 일반 세벌식과 동일한 느낌입니다.
  - 구두점·기호는 chord 창 안에서 눌러도 자모 결합에 포함되지 않고 항상 별도 문자로 처리됩니다.

- **팝업 바깥 클릭으로 닫기**: GNOME 환경에서 한자·특수문자·이모지 팝업이 떴을 때, 팝업 바깥을 좌클릭하면 팝업이 닫힙니다. 그 클릭은 아래 창에 그대로 전달됩니다. 이전에는 ESC 만 가능했습니다.

- **팝업 마우스 페이지 이동 (◀/▶)**: 한자·특수문자·이모지 팝업 모두 하단에 ◀(이전)·▶(다음) 버튼이 생겼습니다. 버튼 클릭 또는 팝업 안에서 우클릭으로 다음 페이지로 이동할 수 있습니다. 페이지가 하나뿐이면 버튼은 숨겨집니다.

- **한자 즐겨찾기 (★/☆)**: 자주 쓰는 한자 후보에 별표를 달 수 있습니다. 같은 음절을 다시 변환하면 즐겨찾기 항목이 목록 맨 위에 나타납니다. GTK·Qt에서는 Space 키로, GNOME에서는 우클릭으로 토글합니다. 별표를 해제하면 후보가 사전순 원위치로 이동하고, 도착한 칸이 노란색으로 잠깐 깜박여(140ms) 어디로 옮겨졌는지 알 수 있습니다.

- **한자 팝업 9×9 확장 격자 모드**: 한자 팝업은 기본으로 9개씩 표시합니다. 오른쪽 하단의 ⊞ 아이콘을 누르면 81개를 한 화면에 볼 수 있는 확장 격자 모드로 전환됩니다. ⊟를 누르면 다시 기본 모드로 돌아옵니다.

- **이모지 팝업 카테고리 탭 + 단축키**: 이모지 팝업 왼쪽에 9개 카테고리(Smileys, Animals, Food…) 탭이 세로로 생겼습니다. A / S / D / F … 키를 누르면 마우스 없이 즉시 해당 카테고리로 이동합니다.

- **AutoTypeFix(자동 한영 오타 교정) 학습 블랙리스트**: AutoTypeFix가 잘못 교정한 단어를 우클릭 → "자동 교정 안 함"으로 등록하면, 이후 그 단어는 자동 교정되지 않습니다. 등록한 단어 목록은 설정 → 억제 단어에서 확인·관리할 수 있습니다.

- **RPM 패키지 지원**: Fedora·openSUSE·RHEL 계열 사용자를 위한 `.rpm` 패키지 빌드를 지원합니다. 다만 spec 파일이 새로 작성된 것으로, 모든 배포판 환경에서 충분히 검증되지 않았습니다. 문제 발생 시 [GitHub Issues](https://github.com/from104/unim/issues)로 제보해 주세요.

### 🔄 달라진 점

- **단일 통합 설정 창 (`unim-settings`)**: 기존에 GTK·Qt 두 가지로 나뉘던 설정 창이 GTK4 + libadwaita 기반의 `unim-settings` 하나로 통합되었습니다. 데스크톱 환경에 관계없이 동일한 설정 화면을 사용합니다.

- **트레이 인디케이터 분리 (`unim-indicator`)**: 트레이 아이콘이 별도 프로세스로 분리되었습니다. 설정 창을 닫아도 트레이 아이콘이 사라지지 않으며, 반대로 트레이를 종료해도 설정 창은 유지됩니다.

- **팝업 렌더링 별도 서비스 분리 (`unim-popup-service`)**: 한자·특수문자·이모지 팝업 렌더링이 별도 백그라운드 프로세스로 분리되었습니다. 처음 한자 변환을 실행하면 자동으로 시작되므로, 별도로 켤 필요가 없습니다.

- **설정 화면 수치 입력이 슬라이더로 변경**: 동시 입력 시간 등 숫자를 입력하는 항목이 슬라이더 + 눈금 표시 방식으로 바뀌었습니다. 마우스 한 번으로 원하는 값에 가까운 위치를 클릭할 수 있습니다.

- **설정 도움말 텍스트 개선**: 설정 화면의 모든 항목에 더 명확한 설명과 권장값이 추가되었습니다. 각 옵션이 무엇을 하는지, 언제 켜면 좋은지, 어떤 값부터 시작하면 좋은지 구체적인 예시와 함께 안내합니다.

- **이모지 팝업 항상 사용 가능**: 이모지 팝업 사용 여부를 별도로 설정하는 옵션이 없어졌습니다. 이제 한글 조합 중이 아닐 때 한자 키를 누르면 언제든 이모지 팝업이 열립니다. 기존 설정 파일에 `engine.emoji_popup` 항목이 있으면 조용히 무시되고, 다음 저장 시 제거됩니다.

- **모아치기 창 범위·기본값 변경**: 동시 입력 시간의 상한이 200ms(기존 100ms)로 늘어났고, 기본값은 60ms(기존 50ms)로 조정되었습니다. 동시 입력이 어렵게 느껴지는 분은 100~150ms부터 시작해 보세요.

### 🐛 고친 버그

- **XIM 환경(XTerm·WezTerm 등 OVER-THE-SPOT 방식)에서 음절 확정 직후 다음 자모가 안 보이던 문제 해소**: 한 음절을 입력하고 바로 다음 자모를 누르면 조합 중 표시(preedit)가 즉시 나타납니다. 이전에는 첫 자모가 보이지 않다가 두 번째 자모를 눌러야 나타났습니다. 일부 드문 XIM 앱에서는 잔존할 수 있습니다 — 알려진 이슈 참고.

- **빌드 경고 0건 달성**: 이번 릴리스는 컴파일러 경고 없이 빌드됩니다. 사용자가 직접 느끼는 변화는 아니지만, 향후 버그 발생 가능성을 낮춥니다.

### 🗑️ 제거

- **쿼티형 세벌식(`ko_3bul_qwerty`) 빌트인에서 제거**: 이 자판이 기본 목록에서 빠졌습니다. 기존에 이 자판을 사용하고 있었다면 업그레이드 후 자동으로 다른 자판으로 전환되지 않으니, 설정 → 자판에서 새 자판을 선택해야 합니다. 계속 쓰고 싶다면 `docs/references/keymaps/ko_3bul_qwerty_v2.json`을 `~/.config/unim/layouts/ko_3bul_qwerty.json`으로 복사하면 사용자 자판으로 불러올 수 있습니다.

- **Qt 설정 창(`unim-gui-qt`) 제거**: Qt6 기반 대체 설정 창이 제거되었습니다. KDE Plasma 포함 모든 환경에서 GTK4 `unim-settings`가 단일 설정 창입니다.

### ⚠️ 업그레이드 안내

- **0.2.0에서 올리는 경우**: 설정 파일(`~/.config/unim/config.yaml`)과 사용자 자판(`~/.config/unim/layouts/*.json`)은 그대로 유지됩니다. 별도 마이그레이션이 필요 없습니다.
- **`unim-gui-qt` 사용자**: `apt remove unim-gui-qt && apt install unim-settings unim-popup-service` 로 전환하세요.
- **`ko_3bul_qwerty` 사용자**: 자판 선택이 자동으로 이전되지 않습니다. 업그레이드 후 설정 → 자판에서 다시 선택하거나, 위 제거 항목의 우회 방법을 따라 사용자 자판으로 등록하세요.
- **UNIM 0.1.x에서 만든 사용자 자판 JSON(v0 형식)**: `~/.config/unim/layouts/`에 직접 작성한 자판 파일이 있다면 v1 형식으로 변환해야 합니다(`"schema_version": 1`과 `combinations` 블록 추가). 변환 방법은 `docs/archive/plans/LAYOUT_PROFILE_V1.md`를 참고하세요.

### 🚧 알려진 이슈

- **KDE Plasma 5.x Wayland**: 한자·특수문자·이모지 팝업이 표시되지 않습니다. 필요한 시스템 라이브러리(`gtk4-layer-shell`)가 Ubuntu 24.04 표준 저장소에 없습니다. X11 세션을 사용하거나 GNOME으로 전환하세요.
- **KDE Plasma 6 Wayland / Sway / Hyprland / river 등 단독 Wayland 컴포지터**: 이번 릴리스에서 충분히 검증되지 않았습니다. 팝업 위치·IME 포커스 전환·입력 포커스 전환에 미세 회귀가 있을 수 있습니다. 문제 발생 시 [GitHub Issues](https://github.com/from104/unim/issues)로 제보해 주세요.
- **일부 드문 XIM ON-THE-SPOT 앱**: 음절 확정 직후 다음 자모의 preedit이 한 프레임 누락될 수 있습니다. XTerm·WezTerm·GTK·Qt·Wayland·GNOME 환경은 영향 없습니다.

---

## [0.2.0] 2026-04-26

### 추가됨

- **자판 프로필 v1 (사양 + 엔진 + 설정 + CLI + GUI)**: 빌트인 자판이 자체완결형 v1 JSON 프로필(`src/keystroke/keymap/*.json`)로 통일되어, 기존 Rust 상수 + 부분 JSON 혼합 경로를 대체.
  - **사용자 프로필**: `~/.config/unim/layouts/*.json`에 v1 JSON을 두면 데몬이 시작 시 스캔하고 mtime 기반으로 핫리로드.
  - **inherits 체인 해석**: 자식 프로필이 `"inherits": "base_name"`을 선언하면 `ProfileRegistry`가 사이클 감지 + 메타데이터/레이아웃/룰셋 레이어 머지로 체인 해석.
  - **룰셋(rule sets)**: 프로필별로 명명된 옵션 서브룰(`rule_sets.<name>`) 선언 가능 — 예: `ko_3bul390`의 `sun_arae_batchim` — GUI SwitchRow 또는 CLI `set korean-active-rule-sets`로 토글.
  - **설정 필드 추가** (가산적, 미설정 시 영향 없음): `korean.custom_layout: Option<String>`, `korean.active_rule_sets: Vec<String>`. 5지점 동기화(config.rs ↔ `unim-cli config` ConfigKey ↔ locales ↔ unim-dbus ↔ settings dialog) 적용.
  - **`unim-cli config layout` 서브커맨드**: `list` / `describe <name>` / `validate <file.json>` (종료 코드 0=통과, 1=경고, 2=오류).
  - **GUI — Adw.ComboRow + 동적 SwitchRow**: 설정 다이얼로그가 모든 한글 프로필(빌트인 10개 + 사용자)을 표시하고, 선택한 프로필의 룰셋을 즉시 토글 가능한 SwitchRow로 노출.
  - **빌트인 프로필 추가 — `ko_3bul_qwerty`** (쿼티형 세벌식): Shift 없는 26자리 알파벳 포화 레이아웃 (14 초성 / 15 중성 / 19 종성). 빌트인 9개 → 10개.
  - 사양: [`docs/archive/plans/LAYOUT_PROFILE_V1.md`](docs/archive/plans/LAYOUT_PROFILE_V1.md).
- **AutoTypeFix 롤백 학습 억제 사전**(`src/typefix_blacklist.rs`, `~/.config/unim/typefix-blacklist.yaml`): 마지막 교정 위에서 일어나는 자연스러운 롤백 패턴(백스페이스 + 입력 모드 전환)을 관찰. 동일 ASCII로 두 번째 AutoTypeFix 시도(retrigger)가 발생하면 한 번에 tentative 억제 항목 등록 + 해당 시도 억제. GUI "확정" 버튼으로 Tentative → Confirmed 수동 승격, `tentative_expiry_hours`(기본 1시간, 1..=12) 후 Inactive로 자동 만료. 데몬이 mtime 변경을 감지해 자동 리로드.
- **AutoTypeFix 신규 설정 3종**: `auto_typefix.*` 하위에 `rollback_detection`(bool, 기본 true), `tentative_expiry_hours`(u16, 기본 1, 1..=12), `observation_timeout_secs`(u8, 기본 10, 5..=15). 3지점 동기화 적용.
- **설정 GUI "억제 단어" 페이지**(`unim-gui-gtk`): 신규 `Adw.PreferencesPage`, 세 그룹(Tentative / Confirmed / Inactive) 구성, 각 행에 확정 / 비활성화 / 제거 / 재활성화 액션.
- **한자 팝업 9×9 확장 격자 모드**: Period 키로 compact(9) ↔ expanded(81) 모드 전환. GTK Standalone, GTK IM, Qt IM, XIM 프런트엔드 모두에 GNOME 익스텐션과 동일하게 적용. ⊞/⊟ 아이콘으로 현재 모드 표시.
- **한자 즐겨찾기 UI** (☆/★): 포커스된 후보에 Space로 즐겨찾기 토글. `HanjaBookmarkChanged` DBus 시그널로 모든 열린 팝업(GTK / Qt / XIM / Wayland / GNOME) 실시간 갱신.
- **역방향 AutoTypeFix 사용자 사전**: 단축키로 선택 영역을 영문 측 사전 항목으로 등록(`RegisterUserDictFromSelection` DBus 메서드). 추가 / 제거 / 갱신 GUI 페이지 제공.
- **트리거 키 자동 영어 모드 전환**: 트리거 키 목록 설정(예: `:`, `/`)으로 한글 → 영어 모드를 경계 문자에서 자동 전환. 기본값은 빈 목록(역호환).
- **이모지 팝업 (Super+.)**: 카테고리 탭, 검색, MRU 즐겨찾기 지원. GTK Standalone(`unim-gui-gtk/src/emoji_popup.rs`) + GNOME Shell 익스텐션(`unim-gnome-extension/emoji_popup.js`) 구현.

### 변경됨

- **`KoreanLayout` enum 제거 (Phase 8)**: 한글 자판 필드가 평문 프로필 이름 문자열로 변경 (`KoreanLayout`은 공개 `String` 타입 별칭). `korean.layout`은 빌트인(`ko_2bulstd`, `ko_3bul390`, `ko_3bul391`, `ko_3bul_noshift`, `ko_3bul_qwerty`) 또는 사용자 프로필 이름 모두 허용. 기존 `custom_layout: Option<String>` 필드는 `layout`으로 통합. 기존 `config.yaml`의 `layout: Dubeolsik`과 `typefix-blacklist.yaml` 항목은 serde compat 레이어로 자동 정규화. C API setter/getter는 C 문자열을 받고/반환.
- **`EnglishLayout` enum 제거 (Phase 9)**: 한글 변경과 대칭. `english.layout`은 String이 되고 빌트인은 `qwerty` / `dvorak` / `colemak` / `colemak_dh` / `workman`. 기존 YAML 값은 serde `from = "EnglishConfigCompat"`로 자동 정규화. C API: `UnimEnglishLayout` enum 삭제, setter/getter는 C 문자열.
- **AutoTypeFix 역방향 롤백 게이트 BS-AND-switch → BS-OR-switch 완화**: 역방향 교정은 `clear_preedit=true`로 동작해 IM 모듈이 롤백 BS를 로컬 소비하므로 `engine_worker`로 절대 전달되지 않음 → AND 게이트는 구조적으로 도달 불가. 역방향은 모드 전환 관찰만으로 롤백 증거로 충분. 순방향은 BS-AND-switch 유지.
- **AutoTypeFix 역방향 억제 키 버그 수정**: `RecentCorrection.ascii`가 역방향에서는 `fix.corrected`(커밋된 영문 단어), 순방향에서는 `fix.original`(ASCII 런)을 저장. 이전엔 모든 역방향 항목이 `""`로 블랙리스팅되어 어떤 후속 쿼리와도 매칭되지 않았음.
- **AutoTypeFix 블랙리스트 등록 시점 이동 (rollback-moment → retrigger-moment)**: 기존 "롤백 시점 등록" 모델은 단발 모드 전환에서 false positive 다수 발생, 순방향 직관과도 일치하지 않음. 이제 BS / 모드 전환 관찰은 보류 교정 표시만 하고, retrigger 시점에 한 번에 tentative 등록 + 중복 시도 억제.
- **`unim-config` 고립 크레이트 제거**: 레거시 CLI 서브크레이트를 `unim-cli config` 서브커맨드로 통합 (설정 CLI Single Source of Truth).
- `unim-daemon`의 `GlobalModeChanged` 시그널 수신 시 `unim-gui` 트레이 아이콘과 팝업이 즉시 동기화되도록 리팩터링.

### 고쳐짐

- **IME — 영어 모드 Space가 직접 commit 경로**(`consumed=true`, `commit=" "`)로 커밋되도록 수정, 한글 모드와 동일. 이전엔 영어 모드 Space가 `not_consumed`를 반환해 GTK IM 모듈이 간헐적으로 공백을 누락(gedit에서 관찰).
- **IME — Focus-out 시 RPC 반환값 위에 추가로 발사되던 중복 `CommitText` DBus 시그널 제거**. 시그널은 컨텍스트 스코프가 아니라 같이 브로드캐스트하면 `늘` 같은 글자가 gedit에서 두 번 커밋되는 문제 발생. `FocusOut()` RPC 반환값이 focus-out 단일 commit 채널.
- **AutoTypeFix — `tentative_expiry_days`(1..=90) → `tentative_expiry_hours`(1..=12)로 변경**. 일 단위는 실용적 블랙리스트 큐레이션엔 너무 거칢. 기존 YAML의 옛 키는 제거 권장, 신규 기본값(1시간)이 자동 적용.
- **gedit / gnome-text-editor용 TypeFix surrounding-text 지원**: GTK IM 모듈이 `request_surrounding()`로 컨텍스트를 가져와, 기존에 커밋 텍스트를 노출하지 않던 앱에서도 역방향 교정 가능.
- **GTK preedit-end 키 잠금 버그**: GTK3/4 IM 모듈이 `unim_emit_preedit` 헬퍼로 `preedit-end`를 발사. preedit이 명시적 시그널 없이 끝날 때 발생하던 ghostty/터미널 키 잠금 해소.
- **XIM AutoTypeFix 재구현**: N+1 BS 프로토콜 모델로 전환, XIM 프런트엔드에서 다문자 교정이 정상 동작 (Chrome preedit edge case는 잔존).

## [0.1.0] 2026-04-21 — 첫 정식 릴리스

UNIM(Universal Next-generation Input Method)의 첫 번째 정식 릴리스. 한국어 입력기 엔진을 처음부터 Rust로 재설계한 결과물로, 다음 컴포넌트로 구성된다.

### 추가됨 — 엔진 코어

- **순수 Rust 한글 엔진 (`src/`)**: 2-bul / 3-bul 390 / 3-bul 391 한글 조합·분해 로직. UI/플랫폼 의존성 0.
- **DBus 데몬 아키텍처 (`unim-daemon` + `unim-dbus`)**: D-Bus 세션 활성화 기반 시스템 와이드 입력 상태 관리. 서비스명 `org.atit.unim.InputMethod`.
- **C-API 래퍼 (`unim-capi` / `libunim_capi`)**: Rust 코어를 C/C++ 프런트엔드에서 사용 가능하도록 노출.
- **통합 CLI (`unim-cli`)**: 한↔영 변환기 + `config` 서브커맨드 (show / set / path / reset / interactive).

### 추가됨 — 프런트엔드

- **GTK 입력기 모듈**: GTK3 (`unim-frontends/gtk3/`)와 GTK4 (`unim-frontends/gtk4/`) 모듈, 공용 컴포넌트 `unim-frontends/gtk-common/`.
- **Qt 플랫폼 입력 컨텍스트 플러그인**: Qt5 (`unim-frontends/qt5/`)와 Qt6 (`unim-frontends/qt6/`) `QPlatformInputContext` 구현, 공용 `unim-frontends/qt-common/`.
- **XIM 프런트엔드 (`unim-frontends/xim/`)**: 네이티브 Rust 기반 X11 XIM 프로토콜 구현, Over-The-Spot Preedit 지원, X11R7.6 XIM 명세 11개 적합성 항목 검증.
- **Wayland 프런트엔드 (`unim-frontends/wayland/`)**: `input-method-v2` + `virtual-keyboard-v1` 프로토콜 지원, KDE Plasma 환경 기초 지원, `zwp_input_popup_surface_v2` 한자/특수문자 팝업 통합.
- **GNOME Shell 익스텐션 (`unim-gnome-extension/`)**: 네이티브 통합 JS 익스텐션. 자판 변환 단축키(`gksrmf` ↔ `한국어`), 터미널 인식 paste 모드 등.

### 추가됨 — GUI

- **GTK4 / libadwaita 설정 다이얼로그 (`unim-gui-gtk`)**: 트레이 아이콘, 한자/특수문자 팝업, 설정 다이얼로그.
- **Qt6 / cxx-qt 대체 GUI (`unim-gui-qt`)**: GTK 대체 옵션. `unim-gui-gtk`와 충돌 없이 공존.
- **im-config 통합**: 시스템 IM 선택 도구와 자동 연동.

### 추가됨 — 기능

- **한글 자판**: 2-bul(두벌식 표준) + 3-bul(세벌식 390 / 391 / no-shift) 빌트인.
- **AutoTypeFix (TypeFix)**: 자동 한↔영 오타 교정 (순방향: 영문 입력 → 한글, 역방향: 한글 입력 → 영문). XIM / GTK / Qt / GNOME 지원.
- **한자 변환**: 한자 변환 팝업, 검색·페이지네이션·인덱스 키 네비게이션.
- **특수문자 / 이모지 검색**: 특수문자/이모지 검색 팝업.
- **앱별 입력 모드 규칙**: 앱별 입력 모드 자동 전환 규칙.

### 추가됨 — 패키징 및 문서

- **Debian 패키징 — 9개 바이너리 패키지**(`debian/control`):
  - `unim-common` (코어 + 데몬 + CLI + libunim_capi)
  - `unim-im-gtk` (GTK3/4 IM 모듈)
  - `unim-im-qt` (Qt5/6 플러그인)
  - `unim-xim` (X11 XIM 프런트엔드)
  - `unim-wayland` (Wayland 입력기 프런트엔드)
  - `unim-gui-gtk` (GTK4 / libadwaita 설정 GUI + 트레이)
  - `unim-gui-qt` (Qt6 / cxx-qt 설정 GUI + 트레이, 대체)
  - `unim-gnome` (GNOME Shell 익스텐션, `unim-gui-gtk` 의존)
  - `unim` (메타패키지 — 전체 스택)
- **종합 문서화**: 컴포넌트별 12개 `SPEC.md`, `IME_BEHAVIOR.md`(프런트엔드 동작 일관성), `POPUP_SPEC.md`(통합 팝업 디자인).

[Keep a Changelog (korean)]: https://keepachangelog.com/ko/1.0.0/
[Semantic Versioning (korean)]: https://semver.org/lang/ko/
