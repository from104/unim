# UNIM Debian Packaging — Ground-Up Redesign Plan

> Status: **Plan only — no implementation.** Owner: `from104`. Last updated: 2026-04-19.
> **Revision r2 (2026-04-19)**: 패키지 분할 정책을 4 → **2 패키지(권장: 옵션 B)** 로 변경. §2.3, §3 Phase 1.5 신설.
>
> 본 문서는 현재 `debian/` 디렉토리 구조를 바닥부터 재검토하고, lintian zero-warning + 재현 가능한 빌드 + Debian/Ubuntu 멘토 업로드 가능한 수준의 메타데이터 정합성을 목표로 한 단계별 작업 계획을 정의한다.

---

## 0. 배경 · 목표 · 비목표

### 0.1 배경
- 현재 4개 바이너리 패키지 (`unim`, `unim-gui-gtk`, `unim-gui-qt`, `gnome-shell-extension-unim`)를 `dh` 시퀀서 + 프로젝트 `Makefile` 위임 방식으로 빌드 중.
- `dpkg-buildpackage`는 정상 종료하지만 `lintian`은 다수 E/W를 보고. 일부는 ABI 안전성과 직결 (RUNPATH, SONAME).
- `gnome-shell-extension-unim.install`은 사실상 구버전 leftover 라인 1개로, 현재 빌드 트리 구조와 불일치 가능성.
- **사용자 추가 요구사항(r2)**: "패키지 갯수를 최소화". 4 → 2 패키지로 통합.

### 0.2 목표
1. `lintian -E -W` 출력 **0건** (필요 시 정당화된 `*.lintian-overrides` 만 허용).
2. `dpkg-buildpackage -b -us -uc` warning **0건**.
3. **2 패키지 (`unim` + `gnome-shell-extension-unim`) 정상 설치/업그레이드/제거** (`piuparts` 통과).
4. **4패키지 → 2패키지 업그레이드 경로 검증** (`Conflicts/Replaces/Provides`로 무결성 보장).
5. 소스 트리 재현성: `make deb` 실행 후에도 워크트리에 패키지 산출물 leftover 없음 (debs/ 외).
6. Launchpad PPA / Debian mentors 업로드 가능 메타데이터 (Maintainer 이메일 정정, Standards-Version 갱신, Vcs-* 필드 추가).

### 0.3 비목표 (이번 라운드에서 다루지 않음)
- multiarch co-installable layout (현재 `Architecture: any`, 단일 arch만 빌드).
- snap/flatpak 패키징.
- 자동 systemd unit 활성화 (현재는 D-Bus activation만 사용).
- GNOME Shell extension의 EGO upload 자동화.
- 단일 패키지 (옵션 A) 채택 — 부록 A에 폴백으로만 명시.

---

## 1. 현황 진단 표 (Lintian + 메타데이터 + 빌드 시스템)

### 1.1 Lintian 이슈 매핑

| # | 태그 | 등급 | 패키지 | 원인 위치 | 근본 원인 | 해결 위치 |
|---|------|------|--------|-----------|-----------|-----------|
| L1 | `custom-library-search-path` | **E** | `unim` (gtk3 `im-unim.so`, gtk4 `libim-unim.so`) | CMake 설치 RPATH | `target_link_directories(... ${UNIM_CAPI_LIB_DIR})` 가 빌드 트리 절대경로를 RUNPATH에 박음. `INSTALL_RPATH` 미설정. | 각 frontend `CMakeLists.txt` (`set_target_properties` 또는 `INSTALL_RPATH_USE_LINK_PATH OFF`) + 보강책으로 `debian/rules` 후처리 (`chrpath -d`). |
| L2 | `custom-library-search-path` | **E** | `unim` (qt5/qt6 `libunim.so`) | 동일 (Qt frontend `CMakeLists.txt`) | 동일 | 동일 (qt5, qt6 `CMakeLists.txt`). |
| L3 | `sharedobject-in-library-directory-missing-soname` | **E** | `unim` (`/usr/lib/<triplet>/libunim_capi.so`) | `unim-capi/Cargo.toml` (cdylib) | Rust cdylib 기본 출력은 SONAME이 없음. | `unim-capi/build.rs`에서 `cargo:rustc-cdylib-link-arg=-Wl,-soname,libunim_capi.so.0`. 더불어 설치 단에서 `libunim_capi.so.0.0.1` + `.so.0` 심볼릭 링크 + `.so` (dev) 분리 가능. |
| L4 | `maintscript-calls-ldconfig` | **W** | `unim` (`postinst`, `postrm`) | `debian/postinst`, `debian/postrm` | `ldconfig` 직접 호출은 dpkg-trigger로 대체해야 함 (Policy 8.1.1). | `debian/unim.triggers` 신설 (`activate-noawait ldconfig`) + `postinst`/`postrm`에서 ldconfig 라인 삭제. |
| L5 | `no-manual-page` | **W** | `unim-cli`, `unim-config`, `unim-daemon`, `unim-xim`, `unim-wayland` (현재 `unim.1`만 존재) | `docs/man/` | manpage 누락. | `docs/man/unim-cli.1`, `unim-config.1`, `unim-daemon.1`, `unim-xim.1`, `unim-wayland.1` 신설 + `Makefile` `install-core` 확장. (단기 회피책: `unim.lintian-overrides`로 daemon/xim/wayland만 정당화 — libexec 한정 시 일부 면제 가능.) |

### 1.2 메타데이터 / 정합성 이슈

| # | 항목 | 현재 값 | 문제 | 수정 |
|---|------|--------|------|------|
| M1 | `Maintainer` | `from104 <from104@github.com>` | 가짜 이메일 | `from104 <from104@gmail.com>` (control + changelog 모두) |
| M2 | `Standards-Version` | `4.6.0` | 구버전 | `4.7.0` (현행 Debian Policy) |
| M3 | `Vcs-Browser` / `Vcs-Git` | 누락 | `lintian: I: vcs-fields-recommended` | `Vcs-Git: https://github.com/from104/unim.git` + `Vcs-Browser: https://github.com/from104/unim` |
| M4 | `Build-Depends` | `cargo`/`rustc` 누락 | rustup 가정 → buildd 환경에서 실패 가능 | `cargo:native (>= 1.75)`, `rustc:native (>= 1.75)`, `chrpath` (RPATH 후처리), `dh-sequence-...` 검토 |
| M5 | `Recommends` | core에 `im-config, dbus`만 | 기본 GUI 부재 시 미설치 가능 | `unim` `Recommends`에 `unim-gui-gtk \| unim-gui-qt` 추가 |
| M6 | `Depends` 명시성 | `${shlibs:Depends}`만 | `shlibdeps`가 못 잡는 런타임 (dconf, glib schemas, qt6-qpa, libadwaita 등) | 명시적 `libadwaita-1-0`, `libqt6qml6` 등 보강 (실측 후 확정) |
| M7 | `gnome-shell-extension-unim.install` | 1줄 (구식 경로) | 빈/잘못된 install 파일 가능 | `Makefile install-gnome-extension` 출력 경로와 일치하는지 실측 후 재작성 (또는 `dh_install --sourcedir=debian/tmp` 패턴으로 통일) |
| M8 | `Source: format` | `3.0 (native)` | native 패키지면 OK이나 PPA 업로드 시 quilt 권장 의견 있음 | 단기 유지, 장기 검토 항목으로만 표시 |

### 1.3 빌드 시스템 / 재현성 이슈

| # | 항목 | 현재 | 문제 | 방향 |
|---|------|------|------|------|
| B1 | `debian/rules` | `override_dh_auto_install: $(MAKE) install DESTDIR=...` | Makefile 위임 → debhelper 자동 install staging 우회 | 현행 위임 유지 (이중 빌드 시스템 통합 비용 과다). 단, `debian/tmp` 단일 staging으로 정규화 + `.install` glob 활용. |
| B2 | `override_dh_auto_clean` | `$(MAKE) clean-all` | `clean-all` → `clean + clean-deb` (`rm -rf debs/` 포함) → 사용자 산출물 삭제 위험 | `override_dh_auto_clean`에서는 `make clean`만 호출 (`clean-deb`는 분리). |
| B3 | `make deb` | `dpkg-buildpackage` → `mv ../*.deb debs/` | 부모 디렉토리 오염 (실패 시 leftover) | `dpkg-buildpackage` 출력 디렉토리 분리: `--buildinfo-option=-O...` 또는 빌드 후 무조건 정리. |
| B4 | `debian/unim/`, `debian/gnome-shell-extension-unim/` 등 잔존 | 이전 빌드 산출물 남아있음 | `clean-all`이 제대로 청소하지 못함 | `debian/rules`의 `override_dh_clean`에서 `dh_clean` + `rm -rf debian/<pkg>/ debian/tmp/ debian/.debhelper/ debian/files debian/*.substvars debian/*.debhelper-build-stamp` 명시. |
| B5 | `override_dh_auto_test: :` | 모든 테스트 skip | 패키지 빌드에서 회귀 검출 불가 | 단기 유지. Phase 후반에 `dh_auto_test`에서 `cargo test --workspace --release --no-run` 정도라도 실행 검토. |
| B6 | `gnome-shell-extension-unim.install` 비실효 | install 라인 1줄, 빌드 결과와 매칭 미확인 | 잠재적 패키지 파일 누락 | Phase 1에서 `debian/tmp` 트리 dump 후 재작성. |
| B7 | nimf 참조: `override_dh_shlibdeps` substvars sed 패치 | 미적용 | Qt6 ABI relaxation (Ubuntu vs Debian) | Phase 4에서 실제 의존성 충돌 발생 시 적용. |

---

## 2. 재설계 전략 — 5축 결정 사항

### 2.1 소스 트리 정리 vs 무결성 (정책)
- **단일 staging 디렉토리**: 모든 파일은 `debian/tmp` 로만 install. 패키지별 `debian/<pkg>/`로의 직접 install 금지.
- **`.install` 파일이 분배 결정자**: `dh_install`이 `debian/tmp` → `debian/<pkg>/` 이동 전담 (`--sourcedir=debian/tmp` 묵시).
- **`dh_missing --fail-missing`** 도입: install 누락 즉시 빌드 실패하도록 강제 (Phase 4).

### 2.2 빌드 시스템 (`debian/rules`)
- **Makefile 위임 유지** (이유: Makefile이 이미 install-* 타겟으로 6개 영역 분할되어 있어 재구현 비용 > 이득).
- 단, **`override_dh_auto_install`은 단일 staging으로 통일**:
  ```make
  override_dh_auto_install:
  	$(MAKE) install DESTDIR=$(CURDIR)/debian/tmp PREFIX=/usr
  ```
- **신규 override**:
  - `override_dh_strip`: 자동 (debhelper 13 기본 OK), 단 Rust 바이너리에 `--dbgsym-migration` 검토.
  - `override_dh_makeshlibs -- -V'libunim-capi0 (>= 0.0.1)'`: SONAME 부여 후 자동 shlibs 생성.
  - `override_dh_shlibdeps`: 필요 시 nimf 스타일 substvars 패치 (Phase 4 확정).
  - `override_dh_clean`: B4 명시.
- **Build-Depends 보강**: `cargo, rustc, chrpath, libadwaita-1-dev, libdbus-1-dev` (실측 후).

### 2.3 패키지 분할 & 메타데이터 (r2 — 4 → 2 패키지로 통합)

#### 2.3.1 옵션 비교

| 항목 | 옵션 A (단일 1패키지) | **옵션 B (2패키지) — 권장** | 현행 (4패키지) |
|---|---|---|---|
| 구성 | `unim` 하나에 전부 | `unim` (core+GTK+Qt+IM 모듈) + `gnome-shell-extension-unim` (arch:all) | core / gtk / qt / extension |
| `apt install` | `apt install unim` 1줄 | `apt install unim` (+ GNOME 사용자만 extension) | 사용자가 4개 중 선택 |
| GTK-only / Qt-only 사용자 | 양쪽 모두 받음 (불필요 ~MB) | 양쪽 모두 받음 (불필요 ~MB) | 선택 가능 |
| GNOME 미사용자 (KDE/Sway/Xfce) | **`gnome-shell` 의존성 강제** → 설치 거부 가능 | extension 미설치, 코어만 깔림 (정상) | 정상 |
| Arch-indep 자산(JS) 빌드 효율 | arch:any에 묶여 매 아키텍처 재빌드 | arch:all 분리 — 1회 빌드로 모든 아키텍처 공유 | 동일 (분리됨) |
| 메타데이터 복잡도 | 최저 (1 control stanza) | 낮음 (2 stanza, install 파일 2개) | 높음 (4 stanza) |
| Debian/Ubuntu 관례 부합도 | 낮음 (extension은 보통 분리) | **높음** (`gnome-shell-extension-*` 네이밍이 표준) | 높음 (단, 코어 분할은 과함) |
| 패키지 간 버전 sync 부담 | 0 | 거의 0 (extension은 `unim (>= ${source:Version})`) | 높음 (3개가 `= ${binary:Version}`) |

#### 2.3.2 권장: **옵션 B (2 패키지)** 채택 근거

1. **사용자 실태**: UNIM은 한국어 IME — 사용자는 Firefox(GTK), KakaoTalk(Qt 기반), VSCode(Electron=GTK), Telegram(Qt) 등 GTK·Qt 앱을 동시에 사용. **GUI 패키지 분할의 실효 이익이 사실상 없음.**
2. **GNOME extension은 명확한 선택**: KDE/Sway/Xfce/i3 사용자에게 `gnome-shell` 의존성을 강제하는 것은 부적절. 옵션 A는 이 지점에서 탈락.
3. **Debian 관례**: `gnome-shell-extension-*` 네이밍 prefix는 Debian/Ubuntu 표준. 기존 패키지명 유지가 sources.list 사용자 기대에 부합.
4. **Arch-indep JS 분리 이득**: extension은 순수 JS+JSON+CSS → `Architecture: all`로 빌드 1회 / 다중 아키텍처 공유. 옵션 A는 이를 포기.
5. **버전 sync 단순화**: 4패키지 시절의 `Depends: unim (= ${binary:Version})` 사슬이 `unim` 단일 패키지로 사라짐. extension만 `>= ${source:Version}`로 느슨하게 묶음.

#### 2.3.3 최종 패키지 구성 (옵션 B)

| 패키지 | Architecture | 내용 | 핵심 의존성 |
|---|---|---|---|
| `unim` | `any` | core daemon (`unim-daemon`, `unim-xim`, `unim-wayland`); CLI (`unim-cli`, `unim-config`); C-API (`libunim_capi.so.*`); GTK3 IM 모듈 (`im-unim.so`); GTK4 IM 모듈 (`libim-unim.so`); Qt5 platform input context (`libunim.so`); Qt6 platform input context (`libunim.so`); GTK GUI (`unim-gui-gtk`, libadwaita); Qt GUI (`unim-gui-qt`, qml); 아이콘, im-config, dbus services, manpage | `${shlibs:Depends}` 자동 (libgtk-3, libgtk-4, libadwaita-1-0, libqt5core5a, libqt6core6, libqt6qml6, libdbus-1-3, …) |
| `gnome-shell-extension-unim` | `all` | `unim-gnome@from104.github.io/` (JS/JSON/CSS/schema/icons) | `unim (>= ${source:Version})`, `gnome-shell` |

#### 2.3.4 control 메타데이터 정의

```debian-control
# unim
Architecture: any
Depends: ${shlibs:Depends}, ${misc:Depends}
Recommends: im-config, dbus
Suggests: gnome-shell-extension-unim
Conflicts: unim-gui-gtk (<< 0.0.2~), unim-gui-qt (<< 0.0.2~), unim-gtk, unim-qt
Replaces: unim-gui-gtk (<< 0.0.2~), unim-gui-qt (<< 0.0.2~), unim-gtk, unim-qt
Provides: unim-gui-gtk (= ${binary:Version}), unim-gui-qt (= ${binary:Version})
Description: Korean Input Method Engine (full)

# gnome-shell-extension-unim
Architecture: all
Depends: unim (>= ${source:Version}), ${misc:Depends}, gnome-shell
Description: UNIM GNOME Shell Extension
```

- `Conflicts/Replaces (<< 0.0.2~)`: 4패키지 시절 마지막 버전(`0.0.1-3`)을 흡수 후 갈아치움. `~` suffix로 pre-release 안전.
- `Provides`: 외부 패키지가 `unim-gui-gtk`로 추천했던 경우 충족 (호환성).
- `unim-gtk`, `unim-qt`: 이미 현행 control에 `Conflicts/Replaces`로 명시된 더 오래된 이름 — 그대로 보존.

#### 2.3.5 신규 control 공통 필드

- `Vcs-Git: https://github.com/from104/unim.git`
- `Vcs-Browser: https://github.com/from104/unim`
- `Bugs: https://github.com/from104/unim/issues`
- `Rules-Requires-Root: no` (유지)

#### 2.3.6 비분리 결정 (재확인)

- `unim-data` (arch indep): 50KB 수준 자산을 따로 떼는 건 옵션 B의 "최소화" 정신과 충돌. **미분리.**
- `libunim-capi-dev` 분리: 외부 바인딩 계획 시 재논의. **현재 미분리.**
- GTK3 별도 분리: GTK3 EOL 시점 (Debian 14 / Ubuntu 26.04 시점) 재논의. **현재 미분리.**

### 2.4 Lintian 클린 달성 매핑

| Lintian 태그 | 해결 위치 | 핵심 변경 | 검증 |
|---|---|---|---|
| L1, L2 RUNPATH | `unim-frontends/{gtk3,gtk4,qt5,qt6}/CMakeLists.txt` | `set_target_properties(<tgt> PROPERTIES BUILD_WITH_INSTALL_RPATH ON INSTALL_RPATH "" INSTALL_RPATH_USE_LINK_PATH OFF SKIP_BUILD_RPATH ON)` | `lintian` + `readelf -d <so> \| grep RUNPATH` (없어야 함) |
| L1, L2 보강 | `debian/rules` `override_dh_fixperms`/`execute_after_dh_install` | `find debian/tmp -name '*.so' -exec chrpath -d {} +` | 동일 |
| L3 SONAME | `unim-capi/build.rs` | `cargo:rustc-cdylib-link-arg=-Wl,-soname,libunim_capi.so.0` | `readelf -d libunim_capi.so \| grep SONAME` |
| L3 layout | `Makefile install-core` + `debian/unim.install` | `libunim_capi.so.0.0.1` 설치 → `.so.0`, `.so` 심링크 | `dpkg -c unim_*.deb \| grep capi` |
| L4 ldconfig | `debian/unim.triggers` (신규) | `activate-noawait ldconfig` | `lintian` + `dpkg-deb --info unim_*.deb` |
| L4 cleanup | `debian/postinst`, `debian/postrm` | `ldconfig` 호출 라인 삭제 (`#DEBHELPER#` 자리만 유지) | 동일 |
| L5 manpage | `docs/man/*.1` 5개 신설 + `Makefile install-core` 확장 + `debian/unim.install` 갱신 | 각 명령어별 1쪽 manpage | `lintian` |
| L5 회피 | `debian/unim.lintian-overrides` (libexec 바이너리만) | `unim binary: no-manual-page usr/libexec/unim-daemon` | 정당화 코멘트 필수 |

### 2.5 재현성 & 배포
- **`make deb` 결과 디렉토리**: `--buildinfo-option=--build=binary` + 결과물을 즉시 `debs/`로 이동, 실패 시에도 `../*.deb` 정리.
- **`clean-deb` vs `clean-all`**: `clean-all`은 dev 사용자용 (debs/ 포함 삭제). `debian/rules`의 `override_dh_auto_clean`은 **`clean`만 호출** (debs/ 보존).
- **PPA / mentors 준비**:
  - Maintainer 이메일 정정 (M1).
  - Standards-Version 4.7.0 (M2).
  - `debian/copyright` machine-readable (이미 OK).
  - `debian/upstream/metadata` (Repository 등) 추가 검토.
  - `dput` 설정은 본 문서 범위 외, 별도 README.

---

## 3. Phase 별 실행 계획

### Phase 1 — 진단 베이스라인 고정 (구현 0%, 측정만)
**목표**: 현 상태의 reproducible snapshot 확보.

| 작업 | 대상 파일 | 검증 |
|---|---|---|
| `lintian -EvIL +pedantic debs/*.deb` 실행, 출력 저장 | — | `docs/plans/lintian-baseline.txt` 생성 |
| `dpkg -c debs/*.deb` 전체 dump | — | `docs/plans/contents-baseline.txt` |
| `readelf -d` 모든 `.so` RPATH/SONAME 점검 | — | 동일 파일에 첨부 |
| `debian/tmp` 트리 dump (`find debian/tmp -type f`) | — | gnome-extension install 라인 검증 자료 |

**수용 기준**: baseline 파일 3종 생성. 코드 변경 없음.

---

### Phase 1.5 — 패키지 통합 (4 → 2) **[r2 신설]**

**목표**: `unim-gui-gtk`, `unim-gui-qt` 두 패키지를 `unim` 본체로 흡수. 후속 Phase는 모두 통합된 2패키지 구조를 전제.

| # | 작업 | 수정/삭제 대상 | 변경 내용 | 검증 |
| --- | --- | --- | --- | --- |
| 1.5.1 | `debian/control` stanza 정리 | `debian/control` | `unim-gui-gtk`, `unim-gui-qt` 두 stanza 삭제. `unim` stanza에 §2.3.4 메타데이터 적용 (`Conflicts/Replaces/Provides`, Suggests). | `dpkg-buildpackage` 후 `debs/`에 `unim_*.deb` + `gnome-shell-extension-unim_*.deb` 두 개만 생성 |
| 1.5.2 | install 파일 병합 | `debian/unim.install` (확장), `debian/unim-gui-gtk.install` (삭제), `debian/unim-gui-qt.install` (삭제) | gtk/qt install 라인을 `unim.install`로 흡수. (예: `usr/lib/*/gtk-3.0/3.0.0/immodules/im-unim.so`, `usr/lib/*/gtk-4.0/4.0.0/immodules/libim-unim.so`, `usr/lib/*/qt5/plugins/platforminputcontexts/libunim.so`, `usr/lib/*/qt6/plugins/platforminputcontexts/libunim.so`, `usr/bin/unim-gui-gtk`, `usr/bin/unim-gui-qt`, `etc/xdg/autostart/unim-gui-gtk.desktop`, …) | `dpkg -c debs/unim_*.deb` 출력에 모든 자산 포함 확인 |
| 1.5.3 | maintainer scripts 통합 | `debian/postinst`, `debian/postrm`, `debian/prerm` (확장); `debian/unim-gui-gtk.postinst`, `debian/unim-gui-gtk.postrm`, `debian/unim-gui-gtk.prerm`, `debian/unim-gui-qt.prerm` (삭제) | `postinst configure`에 GTK3 cache (`gtk-query-immodules-3.0 --update-cache`) 호출 추가; `postrm remove\|purge`에 동일 GTK3 cache 갱신 추가; `prerm remove\|upgrade\|deconfigure`에 `pkill -u "$(logname)" -x unim-gui-gtk`, `pkill -u "$(logname)" -x unim-gui-qt`, `pkill -u "$(logname)" -x unim-daemon` 모두 통합 (조용한 실패) | `dpkg -i` → `dpkg -r` → `dpkg --purge` 사이클에서 GTK3 cache 갱신 + 프로세스 정리 확인 |
| 1.5.4 | substvars 잔존물 정리 | `debian/unim-gui-gtk.substvars`, `debian/unim-gui-qt.substvars`, `debian/unim-gui-gtk/`, `debian/unim-gui-qt/` (삭제) | `git rm` 후 `.gitignore`에 패턴 추가 검토 | `git status` clean |
| 1.5.5 | changelog 항목 추가 | `debian/changelog` | `unim (0.0.2-1)` 신규 — "Merge unim-gui-gtk and unim-gui-qt into unim. Migration via Conflicts/Replaces/Provides." | `dpkg-parsechangelog` |
| 1.5.6 | 업그레이드 경로 검증 | (테스트 시나리오) | 깡통 컨테이너에 기존 `unim-gui-gtk_0.0.1-3*.deb` + `unim-gui-qt_0.0.1-3*.deb` + `unim_0.0.1-3*.deb` 설치 → 신규 `unim_0.0.2-1*.deb` 단일 설치로 자동 흡수 확인 | `apt list --installed \| grep unim` → `unim` 1개만 (+ `gnome-shell-extension-unim` 옵션) |

**수용 기준**:

- `debs/`에 `unim_*.deb`와 `gnome-shell-extension-unim_*.deb` 두 개만 생성.
- 4패키지 → 2패키지 업그레이드 시나리오 성공 (시나리오 1.5.6).
- `apt install ./debs/unim_*.deb` 단일 명령으로 GTK + Qt + IM 모듈 + GUI 모두 설치.
- GNOME 미사용 환경(KDE/Sway)에서 `unim`만 설치 가능 (gnome-shell 의존성 없음).

**위험**:

- `Conflicts/Replaces/Provides` 조합이 잘못되면 dpkg가 "trying to overwrite" 오류로 실패할 수 있음 — Phase 1.5.6 테스트로 사전 검증.
- 기존 사용자가 `apt install unim-gui-gtk`만 한 경우, 신규에서는 `Provides`가 충족하지만 `apt`가 가상 패키지를 어떻게 해석하는지 배포판별 차이 가능 (`apt-get` vs `apt`) — Debian 12 / Ubuntu 22.04 / 24.04 3종 검증.
- Phase 3 이후 표(L1, L2의 패키지 컬럼)가 `unim`으로 통합됨에 따라 §1.1 표는 이미 r2에서 갱신됨.

---

### Phase 2 — 메타데이터 정정 (저위험, 즉시 효과)

| 작업 | 수정 대상 | 변경 내용 | 검증 |
|---|---|---|---|
| Maintainer 이메일 정정 | `debian/control`, `debian/changelog`, `debian/copyright` (Upstream-Contact) | `from104@github.com` → `from104@gmail.com` | `lintian` (W: maintainer-script-contains-systemctl 등 무관) |
| Standards-Version | `debian/control` | `4.6.0` → `4.7.0` | `lintian` (`I: out-of-date-standards-version` 사라짐) |
| Vcs/Bugs 필드 추가 | `debian/control` | `Vcs-Git`, `Vcs-Browser`, `Bugs` | `lintian` (`I: vcs-fields-recommended` 사라짐) |
| Recommends/Suggests 보강 | `debian/control` | `unim` Recommends에 GUI 대안, Suggests에 extension | `apt-cache show unim` |
| Build-Depends 보강 | `debian/control` | `cargo, rustc, chrpath, libadwaita-1-dev` 추가 | `dpkg-buildpackage` 의존성 검사 |
| changelog 신규 항목 | `debian/changelog` | `unim (0.0.1-4)` 항목 — "Debian packaging redesign Phase 1-2" | `dch --check-dirname-level=0` |

**수용 기준**: lintian I-급 1-2개 감소, E/W 변동 없음 (이 단계는 메타데이터만).

**위험**: `chrpath` Build-Depends는 Phase 4에서만 사용되므로 Phase 4와 함께 추가해도 됨.

---

### Phase 3 — RPATH / SONAME 근본 수정 (E 2건 해결)

| 작업 | 수정 대상 | 변경 내용 | 검증 |
|---|---|---|---|
| GTK3 RPATH 차단 | `unim-frontends/gtk3/CMakeLists.txt` | `set_target_properties(im-unim PROPERTIES BUILD_WITH_INSTALL_RPATH ON INSTALL_RPATH "" INSTALL_RPATH_USE_LINK_PATH OFF SKIP_BUILD_RPATH ON)` | `readelf -d im-unim.so \| grep -E '(RUNPATH\|RPATH)'` → 빈 결과 |
| GTK4 RPATH 차단 | `unim-frontends/gtk4/CMakeLists.txt` | 동일 패턴 | 동일 |
| Qt5 RPATH 차단 | `unim-frontends/qt5/CMakeLists.txt` | 동일 패턴 | 동일 |
| Qt6 RPATH 차단 | `unim-frontends/qt6/CMakeLists.txt` | 동일 패턴 | 동일 |
| 보강: `chrpath` 후처리 | `debian/rules` | `execute_after_dh_install: find debian/tmp -name '*.so' -exec chrpath -d {} +` (CMake 수정만으로 부족할 때) | `lintian` (L1, L2 사라짐) |
| SONAME 부여 | `unim-capi/build.rs` 신설 | `cargo:rustc-cdylib-link-arg=-Wl,-soname,libunim_capi.so.0` (cbindgen 호출과 병행) | `readelf -d libunim_capi.so \| grep SONAME` → `libunim_capi.so.0` |
| 라이브러리 layout | `Makefile install-core` | `libunim_capi.so` → `libunim_capi.so.0.0.1` 설치 + `ln -s` 2단 | `dpkg -c` 출력에 `.so.0`, `.so.0.0.1`, `.so` 3개 확인 |
| `.install` 갱신 | `debian/unim.install` | `usr/lib/*/libunim_capi.so.*` 패턴 | `dh_missing --fail-missing` 통과 |

**수용 기준**: `lintian` E 0건. `unim-capi` shlibs 파일 자동 생성 확인 (`dpkg-deb -e unim_*.deb && cat DEBIAN/shlibs`).

**위험**:
- cbindgen은 build.rs에서 이미 동작 중 (Cargo.toml [build-dependencies]) — build.rs가 없으므로 신설. cbindgen 호출 코드도 함께 옮겨야 할 수 있음. (현재 어디서 cbindgen이 실행되는지 별도 확인 필요 — Phase 3 착수 전 점검.)
- SONAME 부여 후 ABI bump 시 `unim` 패키지명에 `0` 접미사 필요해질 수 있음 (`libunim-capi0`). 현재 단일 패키지에 묶여 있어 필수는 아님.

---

### Phase 4 — Maintainer Scripts + Triggers + dh_missing

| 작업 | 수정 대상 | 변경 내용 | 검증 |
|---|---|---|---|
| triggers 도입 | `debian/unim.triggers` (신규) | `activate-noawait ldconfig` | `dpkg-deb --info debs/unim_*.deb \| grep -i trigger` |
| ldconfig 제거 | `debian/postinst`, `debian/postrm` | `ldconfig` 호출 라인 제거 (Phase 1.5에서 GTK3 cache 갱신은 유지). `#DEBHELPER#` 만 남기면 빈 스크립트 가능 → 파일 삭제 후 dh가 자동 생성하도록 | `lintian` (L4 사라짐) |
| `gnome-shell-extension-unim.install` 재작성 | `debian/gnome-shell-extension-unim.install` | `usr/share/gnome-shell/extensions/unim-gnome@from104.github.io/` (전체 디렉토리) | `dpkg -c` 검증 |
| `dh_missing --fail-missing` | `debian/rules` | `override_dh_missing: dh_missing --fail-missing` | 누락 파일 있으면 빌드 실패 |
| `override_dh_clean` 명시화 | `debian/rules` | B4 항목의 명시적 청소 | `git clean -ndx debian/` 출력 최소화 |
| `override_dh_auto_clean` 분리 | `debian/rules` | `$(MAKE) clean-all` → `$(MAKE) clean` | `make deb && ls debs/` 후에도 보존 |

**수용 기준**: `lintian` W 1건 (L4) 감소. `piuparts` 통과 (install → upgrade → remove → purge 사이클).

---

### Phase 5 — Manpage + Lintian Overrides + Dependencies 마무리

| 작업 | 수정 대상 | 변경 내용 | 검증 |
|---|---|---|---|
| Manpage 작성 | `docs/man/unim-cli.1`, `unim-config.1`, `unim-daemon.1`, `unim-xim.1`, `unim-wayland.1` | help2man 또는 수기 작성 (Section 1 / Section 8 결정 필요 — daemon은 8 적합) | `man -l docs/man/unim-cli.1` |
| 설치 추가 | `Makefile install-core` | 신규 manpage들 install + gzip은 dh_compress 자동 | `dpkg -c \| grep man` |
| lintian-overrides | `debian/unim.lintian-overrides` (정당화 코멘트 포함) | libexec 바이너리에 한해 면제 (manpage 작성 안 한 경우) | `lintian -I` |
| 의존성 실측 보강 | `debian/control` Depends | `dpkg-shlibdeps` 출력 분석 후 명시적 추가 | `apt install ./unim_*.deb` 깡통 컨테이너 (debian:stable) |
| Qt6 substvars 패치 (조건부) | `debian/rules` `override_dh_shlibdeps` | nimf 패턴 — Ubuntu/Debian Qt6 ABI 차이 발생 시만 | 양쪽 배포판 설치 테스트 |

**수용 기준**: `lintian -E -W` 0건 (override 정당화 포함).

---

### Phase 6 — 재현성 / 출력 디렉토리 정리

| 작업 | 수정 대상 | 변경 내용 | 검증 |
|---|---|---|---|
| `make deb` 출력 격리 | `Makefile` `deb` target | 빌드 전 `mktemp -d`에서 build → 결과만 `debs/`로 복사. 실패 시에도 `../*.deb` cleanup | `git status` clean 유지 |
| `clean-deb` vs `clean-all` 구분 명확화 | `Makefile` | help 메시지 갱신 | `make help` |
| README 추가 (선택) | `debian/README.source` | "How to build .deb / lintian / piuparts" 안내 | — |
| `debian/upstream/metadata` | 신규 | YAML — Repository, Bug-Database 등 | `lintian` (`I: upstream-metadata-missing`) |

**수용 기준**: 깨끗한 워크트리에서 `make deb && git status` → debs/만 untracked.

---

### Phase 7 — 배포 검증 (PPA / mentors 준비)

| 작업 | 도구 | 검증 |
|---|---|---|
| `pbuilder` 또는 `sbuild` chroot 빌드 | `pbuilder-dist jammy build ../unim_*.dsc` | 깡통 환경에서 빌드 성공 |
| `piuparts debs/*.deb` | piuparts | install/upgrade/remove/purge 무결성 |
| `lintian --pedantic --info` | lintian | 0건 + 정당화된 override만 |
| 멀티 배포판 매트릭스 (선택) | Debian 12, Ubuntu 22.04/24.04 | 각 환경 설치 |

**수용 기준**: 모든 검증 도구 클린.

---

## 4. 참조 패턴

### 4.1 nimf (`debian/rules` 발췌, 본 프로젝트와 비교)
```make
override_dh_shlibdeps:
	dh_shlibdeps
	# Ubuntu Qt6 ABI relaxation
	sed -i 's/libqt6core6t64/libqt6core6/g' debian/*.substvars
```
- **적용 시점**: Phase 5에서 Ubuntu/Debian 양쪽 패키지 호환 충돌 실제 발생 시.

### 4.2 ibus-hangul / kime (조사 TODO)
- ibus-hangul: `triggers` + `lintian-overrides` 패턴 참조 가치.
- kime: Rust IM 동종 사례. `cargo`/`cmake` 혼합 빌드 패턴 — Phase 3 SONAME 처리 비교 검토.
- **착수 전 별도 단기 조사 작업**으로 분리 권장.

### 4.3 Debian Policy / Reference
- Debian Policy 4.7.0 §8 (Shared libraries): SONAME 필수.
- Debian Policy §8.1.1: ldconfig는 trigger로.
- `dh_missing(1)`: `--fail-missing` 권장.

---

## 5. 위험 / 롤백 전략

| 위험 | 확률 | 영향 | 완화 |
|---|---|---|---|
| **4 → 2 패키지 통합 시 `Conflicts/Replaces/Provides` 조합 오류** → dpkg "trying to overwrite" 실패 | **중** | **높음** | Phase 1.5.6 시나리오를 Debian 12 / Ubuntu 22.04 / 24.04 컨테이너 3종에서 사전 검증. `Provides`에 `(= ${binary:Version})` 명시하여 의존 사슬 충족. |
| 기존 사용자가 `unim-gui-gtk`만 깔린 환경에서 신규 `unim` 단독 업그레이드 시 GUI 누락 인식 가능 | 낮 | 낮음 | `Replaces`로 자동 흡수되므로 실 영향 없음. release notes에 "이제 `unim` 한 패키지에 모두 포함" 명기. |
| KDE/Sway 사용자가 `gnome-shell-extension-unim`까지 같이 받게 되는 오해 | 낮 | 낮음 | extension은 Suggests로만 노출 — `apt install --no-install-recommends unim`이 기본 동작은 아니지만 Suggests는 자동 설치 안 됨. |
| CMake `INSTALL_RPATH` 변경 후 런타임 라이브러리 못 찾음 | 중 | 높음 | `libunim_capi.so`가 표준 `/usr/lib/<triplet>/`에 설치되므로 RPATH 불필요. `ldd`로 사전 확인. |
| SONAME 부여 후 기존 빌드 캐시와 충돌 | 낮 | 중 | Phase 3 착수 전 `cargo clean -p unim-capi` |
| `dh_missing --fail-missing`로 갑자기 빌드 실패 | 중 | 낮음 | Phase 4에서 도입, 실패 항목은 즉시 `not-installed` 또는 `.install` 추가로 수습 |
| Manpage 수기 작성 부담 | 중 | 낮음 | Phase 5 우선순위 최저. 단기 lintian-overrides로 회피 가능. |
| Maintainer 이메일 변경이 changelog 무결성에 영향 | 낮 | 낮음 | 신규 changelog 항목부터 적용 (과거 entry 수정 금지) |

**롤백**: 각 Phase는 독립 PR로. `git revert` 단위 롤백 가능. CMake/build.rs 변경은 `make deb` zero-warning 유지 검증 통과 후 머지.

---

## 6. 수용 기준 (전체)

- [ ] `lintian -E -W debs/*.deb` → 0건 (override는 정당화 코멘트 포함하여 허용)
- [ ] `dpkg-buildpackage -b -us -uc` → warning 0건
- [ ] **`debs/`에 `unim_*.deb` + `gnome-shell-extension-unim_*.deb` 정확히 2개만 생성** (r2)
- [ ] `piuparts debs/*.deb` → 2 패키지 모두 통과 (install/upgrade/remove/purge)
- [ ] **4 → 2 패키지 업그레이드 경로 검증**: 기존 `unim-gui-gtk`/`unim-gui-qt` 설치 환경에서 신규 `unim` 설치 시 자동 흡수 (Conflicts/Replaces/Provides 동작) — Debian 12 / Ubuntu 22.04 / 24.04 3종 (r2)
- [ ] **GNOME 미사용 환경(KDE/Sway 컨테이너)에서 `unim` 단독 설치 성공** (gnome-shell 의존성 없음 확인) (r2)
- [ ] `readelf -d` → RUNPATH 0건, `libunim_capi.so` SONAME 부여 확인
- [ ] `make deb` 후 `git status` → `debs/`만 untracked
- [ ] `apt install ./debs/unim_*.deb` 한 줄로 GTK + Qt + IM 모듈 + GUI 모두 설치되어 한국어 입력 동작 (manual)
- [ ] Maintainer 이메일 정정 (`@gmail.com`)
- [ ] `Standards-Version: 4.7.0`
- [ ] `Vcs-Git`, `Vcs-Browser` 필드 존재

---

## 7. 부록 — 미정 사항 (Phase 착수 전 조사 필요)

1. **cbindgen 실행 위치**: `unim-capi/Cargo.toml`에 `[build-dependencies] cbindgen` 있으나 `build.rs` 부재. 어디서 헤더가 생성되는가? Phase 3 build.rs 신설 시 cbindgen 호출 통합 필요.
2. **`gnome-shell-extension-unim.install` 1줄의 경로 (`debian/tmp/usr/share/gnome-shell/extensions/unim-gnome@from104.github.io`)** 가 실제 `make install` 출력과 일치하는가? Phase 1에서 검증.
3. **GTK3 EOL 시점**: GTK3 IM 모듈을 별도 패키지로 분리할지 여부 — Debian 14 / Ubuntu 26.04 출시 시점에 재검토.
4. **`unim-data` 분리 재검토**: arch indep 자산 증가 시 (예: 다국어 manpage, 사전 데이터) 재논의.
5. **Qt6 substvars 패치 필요성**: 실제 Debian 12 (qt6 6.4) / Ubuntu 24.04 (qt6 6.4t64) 양쪽 설치 테스트 후 결정.

---

## 부록 A — 옵션 A (단일 1패키지) 폴백 명세 **[r2 신설]**

> 옵션 B (2패키지)가 권장이나, 사용자가 "패키지 갯수 절대 최소화"를 요구할 경우의 폴백.

### A.1 채택 시 차이점

| 항목 | 옵션 B (권장) | 옵션 A (폴백) |
| --- | --- | --- |
| 패키지 수 | 2 (`unim`, `gnome-shell-extension-unim`) | **1 (`unim` only)** |
| GNOME extension 위치 | 별도 arch:all 패키지 | `unim` 안에 동봉 (arch:any에 묶임) |
| `gnome-shell` 의존성 | extension 패키지에만 | **`unim` Recommends에 `gnome-shell`** (Depends 강제는 금지 — KDE/Sway 사용자 차단) |
| GNOME 미사용자 영향 | 없음 (extension 미설치) | extension 파일이 디스크에 존재하지만 비활성. `gnome-shell`이 없으면 단순히 로드 안 됨 (무해). |
| arch:all 빌드 효율 | 1회 빌드로 다중 아키텍처 공유 | 매 아키텍처마다 JS 재포함 (낭비) |
| changelog 항목 | `(0.0.2-1)` "Merge GUI packages" | `(0.0.2-1)` "Merge into single package" |

### A.2 옵션 A로 전환 시 추가 작업

- `debian/control`에서 `gnome-shell-extension-unim` stanza 제거.
- `debian/gnome-shell-extension-unim.install` 내용을 `debian/unim.install`로 흡수.
- `unim`의 `Conflicts/Replaces`에 `gnome-shell-extension-unim (<< 0.0.2~)` 추가.
- `unim`의 `Provides`에 `gnome-shell-extension-unim (= ${binary:Version})` 추가.
- `unim`의 `Recommends`에 `gnome-shell` 추가 (Depends 금지).
- 관련 Phase 1.5의 1.5.1, 1.5.2, 1.5.5만 수정. Phase 1.5.3/1.5.4/1.5.6은 그대로 적용.

### A.3 권장 비채택 사유 (재확인)

- arch:all 자산을 arch:any에 묶는 것은 Debian/Ubuntu mirror 트래픽 낭비.
- `gnome-shell-extension-*` 네이밍 prefix는 사용자 검색 / EGO upload 호환성 측면에서 표준.
- 1패키지 절감 대비 기술적 부채 증가.

→ 따라서 **본 plan은 옵션 B를 default로 진행**. 옵션 A는 사용자 명시적 결정이 있을 때만 부록 A.2에 따라 전환.
