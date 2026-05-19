SHELL := /bin/bash

# ─────────────────────────────────────────────────────────────────────────────
# UNIM Input Method Engine - Build System
# ─────────────────────────────────────────────────────────────────────────────

PREFIX ?= /usr/local
EXEC_PREFIX ?= $(PREFIX)
BINDIR ?= $(EXEC_PREFIX)/bin
LIBDIR ?= $(EXEC_PREFIX)/lib
LIBEXECDIR ?= $(EXEC_PREFIX)/libexec
INCLUDEDIR ?= $(PREFIX)/include
DATADIR ?= $(PREFIX)/share
SYSCONFDIR ?= /etc

MULTIARCH ?= $(shell dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -print-multiarch 2>/dev/null)
ifeq ($(MULTIARCH),)
    REAL_LIBDIR := $(LIBDIR)
else
    ifeq ($(PREFIX),/usr)
        REAL_LIBDIR := $(LIBDIR)/$(MULTIARCH)
    else
        REAL_LIBDIR := $(LIBDIR)
    endif
endif

GTK3_IMMODULE_DIR ?= $(REAL_LIBDIR)/gtk-3.0/3.0.0/immodules
GTK4_IMMODULE_DIR ?= $(REAL_LIBDIR)/gtk-4.0/4.0.0/immodules
QT5_PLUGIN_DIR    ?= $(REAL_LIBDIR)/qt5/plugins/platforminputcontexts
QT6_PLUGIN_DIR    ?= $(REAL_LIBDIR)/qt6/plugins/platforminputcontexts
IM_CONFIG_DATA_DIR ?= $(DATADIR)/im-config/data
DBUS_SERVICES_DIR  ?= $(DATADIR)/dbus-1/services
SYSTEMD_USER_DIR   ?= $(REAL_LIBDIR)/systemd/user

UUID := unim-gnome@from104.github.io
VERSION := $(shell sed -n 's/.*"version": "\([^"]*\)".*/\1/p' unim-gnome-extension/metadata.json)
ZIP_FILE := $(UUID)-$(VERSION).zip
GNOME_EXTENSION_DIR := $(DATADIR)/gnome-shell/extensions/$(UUID)

DEB_DIR := $(CURDIR)/debs
# Cargo: sudo 실행 시에도 원래 유저의 rustup cargo를 사용
_REAL_HOME := $(or $(shell [ -n "$$SUDO_USER" ] && eval echo ~$$SUDO_USER),$(HOME))
CARGO ?= $(or $(shell which cargo 2>/dev/null),$(wildcard $(_REAL_HOME)/.cargo/bin/cargo),cargo)
export RUSTUP_HOME ?= $(_REAL_HOME)/.rustup
export CARGO_HOME ?= $(_REAL_HOME)/.cargo

# ─── Helpers ──────────────────────────────────────────────────────────────────

# Build a CMake project: $(call cmake_build,dir_path,label)
NPROC := $(shell nproc 2>/dev/null || echo 4)
define cmake_build
	@echo "  → Building $(2)..."
	@mkdir -p $(1)/build && cd $(1)/build && cmake .. && $(MAKE) -j$(NPROC) --no-print-directory
endef

# ─── Phony ───────────────────────────────────────────────────────────────────

.PHONY: all help build build-rust build-frontends build-tests clean clean-all \
        gen-popup-css gen-popup-css-check \
        _check-build \
        install install-core install-frontends install-icons \
        install-indicator install-settings install-popup-service \
        install-keymap-studio install-typing-practice \
        install-gnome-extension install-extension install-systemd \
        uninstall uninstall-core uninstall-frontends uninstall-icons \
        uninstall-indicator uninstall-settings uninstall-popup-service \
        uninstall-keymap-studio uninstall-typing-practice \
        uninstall-gnome-extension uninstall-extension uninstall-systemd \
        enable-systemd disable-systemd status-systemd \
        gnome-extension pack enable-gnome-extension disable-gnome-extension log-gnome-extension \
        deb clean-deb rpm clean-rpm test test-dbus dev-restart \
        dev-gtk3 dev-gtk4 dev-qt5 dev-qt6 dev-core dev-daemon dev-xim dev-wayland \
        dev-indicator dev-settings dev-popup-service dev-extension \
        dev-keymap-studio dev-typing-practice \
        build-keymap-studio build-typing-practice \
        check-windows build-windows clean-windows

# ─── Help ────────────────────────────────────────────────────────────────────

help:
	@echo "UNIM Build System — make [target] [PREFIX=/usr]"
	@echo ""
	@echo "  build              Full build (Rust + frontends)"
	@echo "  build-rust         Rust workspace only"
	@echo "  build-frontends    GTK3/4 + Qt5/6 IM modules"
	@echo "  build-tests        All test applications"
	@echo ""
	@echo "  install            Install all (requires sudo for system paths)"
	@echo "  uninstall          Remove all installed components"
	@echo ""
	@echo "  test-{gtk3,gtk4,qt5,qt6,xim,gnome,wayland,dbus}"
	@echo "  sandbox-{gtk3,gtk4,qt5,qt6,xim,indicator}"
	@echo ""
	@echo "  dev-{gtk3,gtk4,qt5,qt6,core,daemon,xim,wayland,indicator,settings,popup-service,extension,restart}"
	@echo ""
	@echo "  check-windows / build-windows / clean-windows  (WIN_TARGET=...)"
	@echo "  install-gnome-extension / uninstall-gnome-extension / pack"
	@echo "  install-systemd / enable-systemd / disable-systemd / status-systemd"
	@echo "  deb / clean / clean-all"

# ─── Build ───────────────────────────────────────────────────────────────────

all: build

build: gen-popup-css build-rust build-frontends
	@echo "✅ UNIM 전체 빌드 완료!"

# Popup CSS — design tokens (popup_tokens.toml) → GTK CSS + GNOME Shell stylesheet.
# 직접 편집 금지: 디자인 변경은 popup_tokens.toml 수정 후 `make gen-popup-css`.
gen-popup-css:
	@echo "🎨 Generating popup CSS from design tokens..."
	@python3 tools/popup-styles/gen.py

# CI guard — 토큰/template 변경 후 commit 안 한 drift 검출.
gen-popup-css-check:
	@python3 tools/popup-styles/gen.py --check

build-rust:
	@echo "🔨 Building Rust workspace..."
	@$(CARGO) build --release --workspace

build-frontends: build-rust
	@echo "🔨 Building IM Frontends..."
	$(call cmake_build,unim-frontends/gtk3,GTK3 IM Module)
	$(call cmake_build,unim-frontends/gtk4,GTK4 IM Module)
	$(call cmake_build,unim-frontends/qt5,Qt5 IM Plugin)
	$(call cmake_build,unim-frontends/qt6,Qt6 IM Plugin)

# ─── Install ─────────────────────────────────────────────────────────────────

install: _check-build install-core install-indicator install-settings install-popup-service install-frontends install-icons install-gnome-extension
	@echo "✅ UNIM 설치 완료! (PREFIX=$(PREFIX))"

# 빌드 산출물 존재 여부 확인 (sudo make install 시 빌드를 root로 실행하는 것을 방지)
_check-build:
	@if [ ! -f target/release/unim-daemon ]; then \
		echo "❌ 빌드 산출물이 없습니다. 먼저 make build 를 실행하세요."; \
		exit 1; \
	fi

install-core:
	@echo "Installing core components..."
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(REAL_LIBDIR) $(DESTDIR)$(LIBEXECDIR) \
	           $(DESTDIR)$(INCLUDEDIR) $(DESTDIR)$(IM_CONFIG_DATA_DIR) $(DESTDIR)$(DBUS_SERVICES_DIR) \
	           $(DESTDIR)$(SYSCONFDIR)/xdg/autostart
	install -m 755 target/release/libunim_capi.so $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so.0.1.0
	ln -sf libunim_capi.so.0.1.0 $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so.0
	ln -sf libunim_capi.so.0.1.0 $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so
	install -m 644 unim-capi/include/unim.h $(DESTDIR)$(INCLUDEDIR)/
	install -m 755 target/release/unim-cli $(DESTDIR)$(BINDIR)/
	install -m 755 target/release/unim-daemon target/release/unim-xim target/release/unim-wayland $(DESTDIR)$(LIBEXECDIR)/
	install -m 644 im-config/25_unim.conf $(DESTDIR)$(IM_CONFIG_DATA_DIR)/
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" im-config/25_unim.rc > $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc && chmod 644 $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" scripts/org.atit.unim.InputMethod.service > $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service && chmod 644 $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service
	# 비-GNOME 데스크톱(KDE/XFCE/LXDE) 부팅 시 daemon 자동 시작.
	# GNOME 은 gnome-shell 이 직접 daemon 을 DBus 자동활성하므로 NotShowIn=GNOME 으로 제외.
	# daemon 이 살아있어야 popup-service kickstart 도 동작.
	install -m 644 unim-daemon/data/unim-daemon.desktop $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/
	install -d $(DESTDIR)$(PREFIX)/share/man/man1
	install -m 644 docs/man/unim.1 docs/man/unim-cli.1 \
	               docs/man/unim-indicator.1 docs/man/unim-settings.1 docs/man/unim-popup-service.1 \
	               $(DESTDIR)$(PREFIX)/share/man/man1/

install-frontends:
	@echo "Installing IM modules..."
	install -d $(DESTDIR)$(GTK3_IMMODULE_DIR) $(DESTDIR)$(GTK4_IMMODULE_DIR) \
	           $(DESTDIR)$(QT5_PLUGIN_DIR) $(DESTDIR)$(QT6_PLUGIN_DIR)
	install -m 755 unim-frontends/gtk3/build/im-unim.so $(DESTDIR)$(GTK3_IMMODULE_DIR)/
	install -m 755 unim-frontends/gtk4/build/libim-unim.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/
	install -m 755 unim-frontends/qt5/build/libunim.so $(DESTDIR)$(QT5_PLUGIN_DIR)/
	install -m 755 unim-frontends/qt6/build/libunim.so $(DESTDIR)$(QT6_PLUGIN_DIR)/

install-icons:
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 data/icons/unim-korean.svg data/icons/unim-english.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/

install-indicator:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(SYSCONFDIR)/xdg/autostart
	install -m 755 target/release/unim-indicator $(DESTDIR)$(BINDIR)/
	install -m 644 unim-indicator/data/unim-indicator.desktop $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/

install-settings:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(DATADIR)/applications
	install -m 755 target/release/unim-settings $(DESTDIR)$(BINDIR)/
	install -m 644 unim-settings/data/unim-settings.desktop $(DESTDIR)$(DATADIR)/applications/

install-popup-service:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(DBUS_SERVICES_DIR)
	install -m 755 target/release/unim-popup-service $(DESTDIR)$(BINDIR)/
	# D-Bus auto-activation — daemon 이 PopupService 호출 시 자동 launching.
	# .xdg/autostart 의존 race(daemon 미준비 시 register_frontend NoReply 에서 stuck)
	# 를 회피한다. unim-daemon InputMethod.service 와 동일 패턴.
	sed "s|@BINDIR@|$(BINDIR)|g" scripts/org.atit.unim.PopupService.service > $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.PopupService.service && chmod 644 $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.PopupService.service

install-keymap-studio:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(DATADIR)/applications
	install -m 755 target/release/unim-keymap-studio $(DESTDIR)$(BINDIR)/
	install -m 644 unim-keymap-studio/data/unim-keymap-studio.desktop $(DESTDIR)$(DATADIR)/applications/

install-typing-practice:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(DATADIR)/applications
	install -m 755 target/release/unim-typing-practice $(DESTDIR)$(BINDIR)/
	install -m 644 unim-typing-practice/data/unim-typing-practice.desktop $(DESTDIR)$(DATADIR)/applications/

# ─── Uninstall ───────────────────────────────────────────────────────────────

uninstall: uninstall-core uninstall-indicator uninstall-settings uninstall-popup-service uninstall-frontends uninstall-icons uninstall-gnome-extension
	@echo "✅ UNIM 제거 완료!"

uninstall-core:
	rm -f $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so $(DESTDIR)$(INCLUDEDIR)/unim.h \
	      $(DESTDIR)$(BINDIR)/unim-cli \
	      $(DESTDIR)$(LIBEXECDIR)/unim-daemon $(DESTDIR)$(LIBEXECDIR)/unim-xim $(DESTDIR)$(LIBEXECDIR)/unim-wayland \
	      $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.conf $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc \
	      $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service \
	      $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-daemon.desktop \
	      $(DESTDIR)$(PREFIX)/share/man/man1/unim.1 \
	      $(DESTDIR)$(PREFIX)/share/man/man1/unim-cli.1 \
	      $(DESTDIR)$(PREFIX)/share/man/man1/unim-indicator.1 \
	      $(DESTDIR)$(PREFIX)/share/man/man1/unim-settings.1 \
	      $(DESTDIR)$(PREFIX)/share/man/man1/unim-popup-service.1

uninstall-frontends:
	rm -f $(DESTDIR)$(GTK3_IMMODULE_DIR)/im-unim.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim.so \
	      $(DESTDIR)$(QT5_PLUGIN_DIR)/libunim.so $(DESTDIR)$(QT6_PLUGIN_DIR)/libunim.so

uninstall-icons:
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-korean.svg \
	      $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-english.svg

uninstall-indicator:
	rm -f $(DESTDIR)$(BINDIR)/unim-indicator \
	      $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-indicator.desktop

uninstall-settings:
	rm -f $(DESTDIR)$(BINDIR)/unim-settings \
	      $(DESTDIR)$(DATADIR)/applications/unim-settings.desktop

uninstall-popup-service:
	rm -f $(DESTDIR)$(BINDIR)/unim-popup-service \
	      $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.PopupService.service \
	      $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-popup-service.desktop

uninstall-keymap-studio:
	rm -f $(DESTDIR)$(BINDIR)/unim-keymap-studio \
	      $(DESTDIR)$(DATADIR)/applications/unim-keymap-studio.desktop

uninstall-typing-practice:
	rm -f $(DESTDIR)$(BINDIR)/unim-typing-practice \
	      $(DESTDIR)$(DATADIR)/applications/unim-typing-practice.desktop

# ─── Systemd ─────────────────────────────────────────────────────────────────

install-systemd:
	install -d $(DESTDIR)$(SYSTEMD_USER_DIR)
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" scripts/unim-daemon.service > $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service
	chmod 644 $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service
	@echo "✅ Run: systemctl --user daemon-reload && systemctl --user enable --now unim-daemon.service"

uninstall-systemd:
	rm -f $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service

enable-systemd:
	systemctl --user daemon-reload && systemctl --user enable --now unim-daemon.service
	@systemctl --user status unim-daemon.service --no-pager

disable-systemd:
	systemctl --user disable --now unim-daemon.service

status-systemd:
	@systemctl --user status unim-daemon.service --no-pager || true
	@journalctl --user -u unim-daemon.service -n 10 --no-pager || true

# ─── GNOME Extension ─────────────────────────────────────────────────────────

gnome-extension:
	@mkdir -p unim-gnome-extension/icons
	@cp -f data/icons/unim-korean.svg data/icons/unim-english.svg unim-gnome-extension/icons/ 2>/dev/null \
		|| (sudo chown -R $(shell id -u):$(shell id -g) unim-gnome-extension/icons/ && \
		    cp -f data/icons/unim-korean.svg data/icons/unim-english.svg unim-gnome-extension/icons/)
	@# schemas/ may be root-owned from a previous sudo install — reclaim before glib-compile.
	@if [ -d unim-gnome-extension/schemas ] && [ ! -w unim-gnome-extension/schemas ]; then \
		sudo chown -R $(shell id -u):$(shell id -g) unim-gnome-extension/schemas; \
	fi
	@glib-compile-schemas unim-gnome-extension/schemas 2>/dev/null || true
	@# locale/ likewise — reclaim ownership before msgfmt writes .mo files.
	@if [ -d unim-gnome-extension/locale ] && [ ! -w unim-gnome-extension/locale ]; then \
		sudo chown -R $(shell id -u):$(shell id -g) unim-gnome-extension/locale; \
	fi
	@if command -v msgfmt >/dev/null 2>&1; then \
		for po in unim-gnome-extension/po/*.po; do \
			lang=$$(basename $$po .po); \
			mkdir -p unim-gnome-extension/locale/$$lang/LC_MESSAGES; \
			if [ -e unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo ] && \
			   [ ! -w unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo ]; then \
				sudo chown $(shell id -u):$(shell id -g) unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo; \
			fi; \
			msgfmt $$po -o unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo; \
		done; \
	fi

pack: gnome-extension
	@rm -f $(ZIP_FILE) && cd unim-gnome-extension && zip -r ../$(ZIP_FILE) .

install-gnome-extension: gnome-extension
	@install -d "$(DESTDIR)$(GNOME_EXTENSION_DIR)"
	@cp -rf unim-gnome-extension/* "$(DESTDIR)$(GNOME_EXTENSION_DIR)/"
	@rm -rf "$(DESTDIR)$(GNOME_EXTENSION_DIR)/bin" \
		"$(DESTDIR)$(GNOME_EXTENSION_DIR)/po" \
		"$(DESTDIR)$(GNOME_EXTENSION_DIR)/SPEC.md"
	@glib-compile-schemas "$(DESTDIR)$(GNOME_EXTENSION_DIR)/schemas"
	@echo "✅ GNOME Extension 설치 완료!"

install-extension: install-gnome-extension
uninstall-extension: uninstall-gnome-extension

uninstall-gnome-extension:
	@rm -rf "$(DESTDIR)$(GNOME_EXTENSION_DIR)"

enable-gnome-extension:
	@gnome-extensions enable $(UUID)

disable-gnome-extension:
	@gnome-extensions disable $(UUID)

log-gnome-extension:
	@journalctl -f -o cat /usr/bin/gnome-shell

# ─── Debian ──────────────────────────────────────────────────────────────────

deb:
	@dpkg-buildpackage -us -uc -b -jauto
	@mkdir -p $(DEB_DIR)
	@mv -f ../*.deb ../*.ddeb ../unim*.changes ../unim*.buildinfo $(DEB_DIR)/ 2>/dev/null || true
	@echo "✅ Debian packages: $(DEB_DIR)/" && ls -la $(DEB_DIR)/

clean-deb:
	@rm -rf $(DEB_DIR)
	@rm -f ../*.deb ../*.ddeb ../unim*.changes ../unim*.buildinfo ../unim*.tar.gz ../unim*.dsc

# ─── RPM ─────────────────────────────────────────────────────────────────────

RPM_VERSION := $(shell grep '^version' Cargo.toml | head -1 | sed 's/.*"\(.*\)".*/\1/')
RPM_TOPDIR  := $(CURDIR)/rpm/build

rpm: _check-build
	@echo "  → Preparing RPM source tarball..."
	@mkdir -p $(RPM_TOPDIR)/{BUILD,BUILDROOT,RPMS,SOURCES,SPECS,SRPMS}
	@git archive --format=tar.gz --prefix=unim-$(RPM_VERSION)/ HEAD \
	    -o $(RPM_TOPDIR)/SOURCES/unim-$(RPM_VERSION).tar.gz
	@cp rpm/unim.spec $(RPM_TOPDIR)/SPECS/unim.spec
	@echo "  → Running rpmbuild..."
	@rpmbuild --define "_topdir $(RPM_TOPDIR)" -ba $(RPM_TOPDIR)/SPECS/unim.spec
	@echo "✅ RPM packages: $(RPM_TOPDIR)/RPMS/" && find $(RPM_TOPDIR)/RPMS -name '*.rpm' | sort

clean-rpm:
	@rm -rf $(RPM_TOPDIR)
	@echo "✅ RPM build directory cleaned"

# ─── Windows (native / cross-compile) ────────────────────────────────────────
# 호스트가 Windows면 네이티브 빌드, Linux/mac이면 cross-compile.
# WIN_TARGET 을 명시하면 해당 트리플 사용. 미지정 시 rustup 설치된 windows
# 트리플 중 첫 번째(알파벳순으로 gnu 우선)를 자동 선택.
#
# 사전 준비 (Linux cross-compile):
#   rustup target add x86_64-pc-windows-gnu       # mingw (권장)
#   sudo apt install mingw-w64                    # gnu 타겟용 linker
#   # 또는
#   rustup target add x86_64-pc-windows-msvc      # msvc (lld linker 필요)

WIN_CRATES := -p unim -p unim-capi -p unim-windows -p unim-tsf

ifeq ($(OS),Windows_NT)
    WIN_NATIVE  := 1
    WIN_TARGET  ?=
else
    WIN_NATIVE  :=
    WIN_TARGET  ?= $(shell rustup target list --installed 2>/dev/null | \
                          grep -E '^x86_64-pc-windows-(gnu|msvc)$$' | head -1)
endif

WIN_CARGO_FLAGS := $(if $(WIN_TARGET),--target $(WIN_TARGET))
WIN_OUT_DIR     := target/$(if $(WIN_TARGET),$(WIN_TARGET)/,)release

# 네이티브/크로스 환경 검증 (mingw 타겟이면 mingw-w64 toolchain 필수)
define _check_windows_env
	@if [ -z "$(WIN_NATIVE)" ] && [ -z "$(WIN_TARGET)" ]; then \
		echo "❌ Windows target 미설치."; \
		echo "   rustup target add x86_64-pc-windows-gnu   (권장: mingw)"; \
		echo "   rustup target add x86_64-pc-windows-msvc  (대안: lld 필요)"; \
		exit 1; \
	fi
	@if [ "$(WIN_TARGET)" = "x86_64-pc-windows-gnu" ] && \
	    ! command -v x86_64-w64-mingw32-gcc >/dev/null 2>&1; then \
		echo "❌ mingw-w64 toolchain 누락. sudo apt install mingw-w64"; \
		exit 1; \
	fi
endef

check-windows:
	@echo "🔍 Windows 컴파일 검증 ($(if $(WIN_NATIVE),native,$(if $(WIN_TARGET),cross: $(WIN_TARGET),no-target)))..."
	$(_check_windows_env)
	@$(CARGO) check $(WIN_CARGO_FLAGS) $(WIN_CRATES)
	@echo "✅ Windows check 통과"

build-windows:
	@echo "🔨 Windows 빌드 ($(if $(WIN_NATIVE),native,$(if $(WIN_TARGET),cross: $(WIN_TARGET),no-target)))..."
	$(_check_windows_env)
	@$(CARGO) build --release $(WIN_CARGO_FLAGS) $(WIN_CRATES)
	@echo "✅ Windows 빌드 완료: $(WIN_OUT_DIR)/"
	@ls -1 $(WIN_OUT_DIR)/*.{exe,dll} 2>/dev/null | sed 's|^|   |' || true

clean-windows:
	@rm -rf target/x86_64-pc-windows-gnu target/x86_64-pc-windows-msvc
	@echo "✅ Windows target 디렉토리 정리 완료"

# ─── Test & Verification ─────────────────────────────────────────────────────

test:
	@echo "UNIM 설치 상태 확인"
	@for f in $(REAL_LIBDIR)/libunim_capi.so \
	          $(GTK3_IMMODULE_DIR)/im-unim.so $(GTK4_IMMODULE_DIR)/libim-unim.so \
	          $(QT5_PLUGIN_DIR)/libunim.so $(QT6_PLUGIN_DIR)/libunim.so; do \
		printf "  %-55s %s\n" "$$f" "$$([ -f $(DESTDIR)$$f ] && echo '✓' || echo '✗')"; \
	done
	@for cmd in unim-cli unim-indicator unim-settings unim-popup-service; do \
		printf "  %-55s %s\n" "$(BINDIR)/$$cmd" "$$([ -f $(DESTDIR)$(BINDIR)/$$cmd ] && echo '✓' || echo '✗')"; \
	done
	@for cmd in unim-daemon unim-xim unim-wayland; do \
		printf "  %-55s %s\n" "$(LIBEXECDIR)/$$cmd" "$$([ -f $(DESTDIR)$(LIBEXECDIR)/$$cmd ] && echo '✓' || echo '✗')"; \
	done

# CMake-based test apps (static pattern rule) — 빌드 후 바로 실행
test-gtk3 test-gtk4 test-qt5 test-qt6 test-xim test-gnome: test-%:
	$(call cmake_build,tests/unim-test-$*,$* test app)
	@echo "🚀 Launching unim-test-$* ..."
	@./tests/unim-test-$*/build/unim-test-$* &

test-wayland: build-rust
	@$(CARGO) build --release -p unim-test-wayland
	@echo "🚀 Launching unim-test-wayland ..."
	@./target/release/unim-test-wayland &

test-dbus: build-rust
	@./target/release/unim-daemon -n &
	@sleep 2
	@busctl --user list 2>/dev/null | grep -i unim || echo "⚠️  unim 서비스 없음"
	@busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod 2>/dev/null | head -15 || true
	@pkill -f "unim-daemon" 2>/dev/null || true

build-tests: build-frontends
	$(call cmake_build,tests/unim-test-gtk3,GTK3 Test App)
	$(call cmake_build,tests/unim-test-gtk4,GTK4 Test App)
	$(call cmake_build,tests/unim-test-qt5,Qt5 Test App)
	$(call cmake_build,tests/unim-test-qt6,Qt6 Test App)
	$(call cmake_build,tests/unim-test-xim,XIM Test App)
	$(call cmake_build,tests/unim-test-gnome,GNOME Test App)
	@$(CARGO) build --release -p unim-test-wayland
	@echo "✅ 모든 테스트 앱 빌드 완료!"

# ─── Sandbox (Xephyr) ────────────────────────────────────────────────────────

sandbox: build-tests
	@./scripts/sandbox.sh $(SANDBOX_APP)

sandbox-gtk3 sandbox-gtk4 sandbox-qt5 sandbox-qt6 sandbox-xim: sandbox-%: build-tests
	@./scripts/sandbox.sh $*

sandbox-indicator: build-tests
	@./scripts/sandbox.sh --indicator

# ─── Clean ───────────────────────────────────────────────────────────────────

clean:
	@rm -f $(ZIP_FILE)
	@rm -rf unim-gnome-extension/icons unim-gnome-extension/locale
	@rm -f unim-gnome-extension/schemas/gschemas.compiled
	@rm -rf unim-frontends/gtk3/build unim-frontends/gtk4/build \
	        unim-frontends/qt5/build unim-frontends/qt6/build
	@rm -rf tests/unim-test-gtk3/build tests/unim-test-gtk4/build \
	        tests/unim-test-qt5/build tests/unim-test-qt6/build \
	        tests/unim-test-xim/build tests/unim-test-gnome/build

clean-all: clean clean-deb
	@$(CARGO) clean

# ─── Quick Dev (requires initial: make build && sudo make install PREFIX=/usr)

dev-gtk3:
	@cd unim-frontends/gtk3/build && $(MAKE) --no-print-directory
	@sudo cp unim-frontends/gtk3/build/im-unim.so $(GTK3_IMMODULE_DIR)/
	@echo "✅ GTK3 모듈 배포 완료!"

dev-gtk4:
	@cd unim-frontends/gtk4/build && $(MAKE) --no-print-directory
	@sudo cp unim-frontends/gtk4/build/libim-unim.so $(GTK4_IMMODULE_DIR)/
	@echo "✅ GTK4 모듈 배포 완료!"

dev-qt5:
	@cd unim-frontends/qt5/build && $(MAKE) --no-print-directory
	@sudo cp unim-frontends/qt5/build/libunim.so $(QT5_PLUGIN_DIR)/
	@echo "✅ Qt5 플러그인 배포 완료!"

dev-qt6:
	@cd unim-frontends/qt6/build && $(MAKE) --no-print-directory
	@sudo cp unim-frontends/qt6/build/libunim.so $(QT6_PLUGIN_DIR)/
	@echo "✅ Qt6 플러그인 배포 완료!"

dev-core:
	@$(CARGO) build --release -p unim-capi
	@sudo cp target/release/libunim_capi.so $(REAL_LIBDIR)/
	@echo "✅ 코어 라이브러리 배포 완료!"

# dev-* 타겟은 실제 설치된 경로를 자동 감지하여 배포
# (make install PREFIX=/usr 로 설치했으면 /usr/libexec, 아니면 /usr/local/libexec)
DEV_DAEMON_PATH := $(shell command -v unim-daemon 2>/dev/null || find /usr/libexec /usr/local/libexec -name unim-daemon -print -quit 2>/dev/null)
DEV_LIBEXECDIR  := $(if $(DEV_DAEMON_PATH),$(dir $(DEV_DAEMON_PATH)),$(LIBEXECDIR)/)
DEV_BINDIR      := $(shell dirname $$(command -v unim-cli 2>/dev/null || echo $(BINDIR)/unim-cli))

dev-daemon:
	@$(CARGO) build --release -p unim-daemon
	@pkill -9 -x unim-daemon 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-daemon $(DEV_LIBEXECDIR)
	@UNIM_DEVELOP=1 $(DEV_LIBEXECDIR)unim-daemon -n --replace &
	@sleep 1
	@echo "✅ 데몬 빌드→중지→설치($(DEV_LIBEXECDIR))→재시작 완료!"

dev-xim:
	@$(CARGO) build --release -p unim-xim
	@pkill -9 -x unim-xim 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-xim $(DEV_LIBEXECDIR)
	@echo "✅ XIM 서버 배포 완료!"

dev-wayland:
	@$(CARGO) build --release -p unim-wayland
	@pkill -9 -x unim-wayland 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-wayland $(DEV_LIBEXECDIR)
	@echo "✅ Wayland IM 배포 완료!"

dev-indicator:
	@$(CARGO) build --release -p unim-indicator
	@pkill -9 -x unim-indicator 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-indicator $(DEV_BINDIR)/
	@echo "✅ unim-indicator 배포 완료!"

dev-settings:
	@$(CARGO) build --release -p unim-settings
	@pkill -9 -x unim-settings 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-settings $(DEV_BINDIR)/
	@echo "✅ unim-settings 배포 완료!"

dev-popup-service:
	@$(CARGO) build --release -p unim-popup-service
	@pkill -9 -x unim-popup-service 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-popup-service $(DEV_BINDIR)/
	@echo "✅ unim-popup-service 배포 완료!"

dev-keymap-studio:
	@$(CARGO) run -p unim-keymap-studio

dev-typing-practice:
	@$(CARGO) run -p unim-typing-practice

build-keymap-studio:
	@$(CARGO) build --release -p unim-keymap-studio

build-typing-practice:
	@$(CARGO) build --release -p unim-typing-practice

dev-extension:
	@mkdir -p ~/.local/share/gnome-shell/extensions/$(UUID)/schemas
	@mkdir -p ~/.local/share/gnome-shell/extensions/$(UUID)/icons
	@cp -f unim-gnome-extension/*.js unim-gnome-extension/*.css \
		unim-gnome-extension/metadata.json \
		~/.local/share/gnome-shell/extensions/$(UUID)/
	@cp -f unim-gnome-extension/schemas/*.xml \
		~/.local/share/gnome-shell/extensions/$(UUID)/schemas/ 2>/dev/null || true
	@cp -f data/icons/unim-korean.svg data/icons/unim-english.svg \
		~/.local/share/gnome-shell/extensions/$(UUID)/icons/ 2>/dev/null || true
	@glib-compile-schemas ~/.local/share/gnome-shell/extensions/$(UUID)/schemas 2>/dev/null || true
	@echo "✅ Extension 배포 완료! GNOME Shell 재시작 필요 (로그아웃→로그인)."

dev-restart:
	@pkill -9 -x unim-daemon 2>/dev/null; pkill -9 -x unim-xim 2>/dev/null; \
	 pkill -9 -x unim-wayland 2>/dev/null; pkill -9 -x unim-indicator 2>/dev/null; \
	 pkill -9 -x unim-settings 2>/dev/null; pkill -9 -x unim-popup-service 2>/dev/null; sleep 1
	@UNIM_DEVELOP=1 $(DEV_LIBEXECDIR)unim-daemon -n --replace &
	@sleep 1
	@echo "✅ 모든 UNIM 프로세스 재시작 완료!"

# ─── Test Automation ─────────────────────────────────────────────────────────

log-check:
	@./scripts/unim-log-check.sh

log-watch:
	@./scripts/unim-log-check.sh --watch

log-clear:
	@./scripts/unim-log-check.sh --clear

smoke-test:
	@./scripts/dbus-smoke-test.sh

test-dbus-auto: build-rust
	@$(CARGO) build --release -p unim-test-dbus
	@./target/release/unim-test-dbus

dev-test: build-rust
	@echo ""
	@echo "═══ UNIM Dev Test ═══"
	@echo ""
	@echo "── 1. 로그 초기화 ──"
	@> ~/.unim-errors.log
	@echo "✅ 로그 클리어"
	@echo ""
	@echo "── 2. 데몬 재시작 ──"
	@pkill -f unim-daemon 2>/dev/null || true
	@sleep 1
	@echo "✅ 데몬 종료 (DBus 자동활성화)"
	@echo ""
	@echo "── 3. 유닛 테스트 ──"
	@$(CARGO) test --workspace --quiet
	@echo ""
	@echo "── 4. DBus 스모크 테스트 ──"
	@./scripts/dbus-smoke-test.sh
	@echo ""
	@echo "── 5. DBus 자동 테스트 ──"
	@$(CARGO) build --release -p unim-test-dbus --quiet
	@./target/release/unim-test-dbus
	@echo ""
	@echo "── 6. 로그 체크 ──"
	@./scripts/unim-log-check.sh
	@echo ""
	@echo "═══ Dev Test 완료 ═══"
