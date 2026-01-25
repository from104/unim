SHELL := /bin/bash

# ─────────────────────────────────────────────────────────────────────────────
# UNIM Input Method Engine - Build System
# ─────────────────────────────────────────────────────────────────────────────

# Installation prefix (default: /usr/local, use PREFIX=/usr for system-wide)
PREFIX ?= /usr/local
EXEC_PREFIX ?= $(PREFIX)
BINDIR ?= $(EXEC_PREFIX)/bin
LIBDIR ?= $(EXEC_PREFIX)/lib
LIBEXECDIR ?= $(EXEC_PREFIX)/libexec
INCLUDEDIR ?= $(PREFIX)/include
DATADIR ?= $(PREFIX)/share
SYSCONFDIR ?= /etc
LOCALEDIR ?= $(DATADIR)/locale

# Attempt to detect multiarch (primarily for Debian/Ubuntu)
MULTIARCH ?= $(shell dpkg-architecture -qDEB_HOST_MULTIARCH 2>/dev/null || gcc -print-multiarch 2>/dev/null)

# Adjusted LIBDIR for Debian Multiarch
ifeq ($(MULTIARCH),)
    REAL_LIBDIR := $(LIBDIR)
else
    # Only use multiarch if we are installing to /usr
    ifeq ($(PREFIX),/usr)
        REAL_LIBDIR := $(LIBDIR)/$(MULTIARCH)
    else
        REAL_LIBDIR := $(LIBDIR)
    endif
endif

# GTK/Qt module paths (respecting PREFIX)
GTK3_IMMODULE_DIR ?= $(REAL_LIBDIR)/gtk-3.0/3.0.0/immodules
GTK4_IMMODULE_DIR ?= $(REAL_LIBDIR)/gtk-4.0/4.0.0/immodules
QT5_PLUGIN_DIR ?= $(REAL_LIBDIR)/qt5/plugins/platforminputcontexts
QT6_PLUGIN_DIR ?= $(REAL_LIBDIR)/qt6/plugins/platforminputcontexts

# im-config integration path
IM_CONFIG_DATA_DIR ?= $(DATADIR)/im-config/data

# DBus service directory (session bus)
DBUS_SERVICES_DIR ?= $(DATADIR)/dbus-1/services

# GNOME Extension settings
UUID := unim-typefix@from104.github.io
VERSION := $(shell sed -n 's/.*"version": "\([^"]*\)".*/\1/p' unim-gnome-extension/metadata.json)
ZIP_FILE := $(UUID)-$(VERSION).zip

# Build paths
CAPI_LIB := $(CURDIR)/target/release/libunim_capi.so
CAPI_INC := $(CURDIR)/unim-capi/include
DEB_DIR := $(CURDIR)/debs

CARGO := cargo

.PHONY: all clean build build-rust build-frontends build-settings \
        install install-core install-frontends install-settings install-icons install-autostart \
        uninstall uninstall-core uninstall-frontends uninstall-settings uninstall-icons uninstall-autostart \
        pack install-gnome-extension uninstall-gnome-extension enable disable log test test-dbus test-xim \
        deb clean-deb clean-all help \
        test-gtk3 test-gtk4

# ─────────────────────────────────────────────────────────────────────────────
# Help
# ─────────────────────────────────────────────────────────────────────────────

help:
	@echo "════════════════════════════════════════════════════════════════════"
	@echo "  UNIM Input Method Engine - Build System"
	@echo "════════════════════════════════════════════════════════════════════"
	@echo ""
	@echo "Usage: make [target] [PREFIX=/usr/local]"
	@echo ""
	@echo "Build targets:"
	@echo "  build            - Full build (Rust + frontends + settings)"
	@echo "  build-rust       - Build Rust workspace only"
	@echo "  build-frontends  - Build GTK3/GTK4/Qt5/Qt6 IM modules"
	@echo "  build-settings   - Build GTK/Qt settings tools"
	@echo ""
	@echo "Install targets (requires sudo for system paths):"
	@echo "  install          - Install all components"
	@echo "  install-core     - Install core library, daemon, and CLI tools"
	@echo "  install-frontends - Install GTK/Qt IM modules"
	@echo "  install-settings - Install settings tools"
	@echo ""
	@echo "Uninstall targets:"
	@echo "  uninstall        - Remove all installed components"
	@echo ""
	@echo "GNOME Extension:"
	@echo "  install-gnome-extension   - Install to user's GNOME Shell"
	@echo "  uninstall-gnome-extension - Remove from user's GNOME Shell"
	@echo "  pack             - Create distributable .zip file"
	@echo ""
	@echo "Packaging:"
	@echo "  deb              - Build Debian packages (saved to ./debs/)"
	@echo ""
	@echo "Clean:"
	@echo "  clean            - Remove build artifacts"
	@echo "  clean-all        - Remove all artifacts including Cargo target"
	@echo ""
	@echo "DBus / XIM Testing:"
	@echo "  test-dbus        - Verify DBus service registration"
	@echo "  test-xim         - Build and run XIM test application"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX           - Installation prefix (default: /usr/local)"
	@echo "  DESTDIR          - Staging directory for packaging"
	@echo ""
	@echo "Examples:"
	@echo "  make build"
	@echo "  sudo make install PREFIX=/usr"
	@echo "  sudo make uninstall PREFIX=/usr"
	@echo "════════════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Main Build Targets
# ─────────────────────────────────────────────────────────────────────────────

all: build

# Full build: Rust workspace + all frontends + settings tools
build: build-rust build-frontends build-settings
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ UNIM 전체 빌드 완료!"
	@echo "════════════════════════════════════════════════════════════"

# Build Rust workspace (core, CLI, C-API, daemon, config, indicator, xim, wayland)
build-rust:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Rust workspace..."
	@echo "════════════════════════════════════════════════════════════"
	@$(CARGO) build --release --workspace

# Build GTK3/GTK4/Qt5/Qt6 IM Modules
build-frontends: build-rust
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building IM Frontends (GTK3, GTK4, Qt5, Qt6)..."
	@echo "════════════════════════════════════════════════════════════"
	@# GTK3 IM Module
	@echo "  → Building GTK3 IM Module..."
	@mkdir -p unim-frontends/gtk3/build && cd unim-frontends/gtk3/build && \
		cmake .. -DUNIM_CAPI_LIB=$(CAPI_LIB) -DUNIM_CAPI_INCLUDE=$(CAPI_INC) && make
	@# GTK4 IM Module
	@echo "  → Building GTK4 IM Module..."
	@mkdir -p unim-frontends/gtk4/build && cd unim-frontends/gtk4/build && \
		cmake .. -DUNIM_CAPI_LIB=$(CAPI_LIB) -DUNIM_CAPI_INCLUDE=$(CAPI_INC) && make
	@# Qt5 IM Plugin
	@echo "  → Building Qt5 IM Plugin..."
	@mkdir -p unim-frontends/qt5/build && cd unim-frontends/qt5/build && \
		cmake .. -DUNIM_CAPI_LIB=$(CAPI_LIB) -DUNIM_CAPI_INCLUDE=$(CAPI_INC) && make
	@# Qt6 IM Plugin
	@echo "  → Building Qt6 IM Plugin..."
	@mkdir -p unim-frontends/qt6/build && cd unim-frontends/qt6/build && \
		cmake .. -DUNIM_CAPI_LIB=$(CAPI_LIB) -DUNIM_CAPI_INCLUDE=$(CAPI_INC) && make

# Build GTK/Qt settings tools
build-settings: build-rust
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Settings Tools (GTK, Qt)..."
	@echo "════════════════════════════════════════════════════════════"
	@# GTK Settings
	@echo "  → Building GTK Settings..."
	@mkdir -p unim-gtk-settings/build && cd unim-gtk-settings/build && cmake .. && make
	@# Qt Settings
	@echo "  → Building Qt Settings..."
	@mkdir -p unim-qt-settings/build && cd unim-qt-settings/build && cmake .. && make

# ─────────────────────────────────────────────────────────────────────────────
# Install Targets
# ─────────────────────────────────────────────────────────────────────────────

install: install-core install-frontends install-settings install-icons install-autostart
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ UNIM 설치 완료! (PREFIX=$(PREFIX))"
	@echo ""
	@echo "다음 단계:"
	@echo "  1. 로그아웃 후 재로그인"
	@echo "  2. im-config에서 unim 선택 또는 환경변수 설정:"
	@echo "     export GTK_IM_MODULE=unim"
	@echo "     export QT_IM_MODULE=unim"
	@echo "     export XMODIFIERS=@im=unim"
	@echo "════════════════════════════════════════════════════════════"

install-core:
	@echo "Installing core components to $(DESTDIR)$(PREFIX)..."
	@# Directories
	install -d $(DESTDIR)$(BINDIR)
	install -d $(DESTDIR)$(REAL_LIBDIR)
	install -d $(DESTDIR)$(LIBEXECDIR)
	install -d $(DESTDIR)$(INCLUDEDIR)
	install -d $(DESTDIR)$(IM_CONFIG_DATA_DIR)
	@# Core library
	install -m 755 target/release/libunim_capi.so $(DESTDIR)$(REAL_LIBDIR)/
	@# Header file
	install -m 644 unim-capi/include/unim.h $(DESTDIR)$(INCLUDEDIR)/
	@# Executables
	install -m 755 target/release/unim-cli $(DESTDIR)$(BINDIR)/
	install -m 755 target/release/unim-config $(DESTDIR)$(BINDIR)/
	install -m 755 target/release/unim-indicator $(DESTDIR)$(BINDIR)/
	@# Daemons / servers
	install -m 755 target/release/unim-daemon $(DESTDIR)$(LIBEXECDIR)/
	install -m 755 target/release/unim-xim $(DESTDIR)$(LIBEXECDIR)/
	install -m 755 target/release/unim-wayland $(DESTDIR)$(LIBEXECDIR)/
	@# im-config integration
	install -m 644 im-config/25_unim.conf $(DESTDIR)$(IM_CONFIG_DATA_DIR)/
	install -m 644 im-config/25_unim.rc $(DESTDIR)$(IM_CONFIG_DATA_DIR)/
	@# DBus service (session bus auto-activation)
	install -d $(DESTDIR)$(DBUS_SERVICES_DIR)
	install -m 644 scripts/org.atit.unim.InputMethod.service $(DESTDIR)$(DBUS_SERVICES_DIR)/

install-frontends:
	@echo "Installing IM modules..."
	@# GTK3
	install -d $(DESTDIR)$(GTK3_IMMODULE_DIR)
	install -m 755 unim-frontends/gtk3/build/libim-unim.so $(DESTDIR)$(GTK3_IMMODULE_DIR)/
	@# GTK4
	install -d $(DESTDIR)$(GTK4_IMMODULE_DIR)
	install -m 755 unim-frontends/gtk4/build/libim-unim-gtk4.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/
	@# Qt5
	install -d $(DESTDIR)$(QT5_PLUGIN_DIR)
	install -m 755 unim-frontends/qt5/build/libunim.so $(DESTDIR)$(QT5_PLUGIN_DIR)/
	@# Qt6
	install -d $(DESTDIR)$(QT6_PLUGIN_DIR)
	install -m 755 unim-frontends/qt6/build/libunim.so $(DESTDIR)$(QT6_PLUGIN_DIR)/

install-settings:
	@echo "Installing settings tools..."
	install -d $(DESTDIR)$(BINDIR)
	install -m 755 unim-gtk-settings/build/unim-gtk-settings $(DESTDIR)$(BINDIR)/
	install -m 755 unim-qt-settings/build/unim-qt-settings $(DESTDIR)$(BINDIR)/

install-icons:
	@echo "Installing icons..."
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 unim-indicator/data/icons/hicolor/scalable/apps/unim-hangul.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/
	install -m 644 unim-indicator/data/icons/hicolor/scalable/apps/unim-latin.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/

install-autostart:
	@echo "Installing autostart entry..."
	install -d $(DESTDIR)$(SYSCONFDIR)/xdg/autostart
	install -m 644 unim-indicator/data/unim-indicator.desktop $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/

# ─────────────────────────────────────────────────────────────────────────────
# Uninstall Targets
# ─────────────────────────────────────────────────────────────────────────────

uninstall: uninstall-core uninstall-frontends uninstall-settings uninstall-icons uninstall-autostart
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ UNIM 제거 완료!"
	@echo "════════════════════════════════════════════════════════════"

uninstall-core:
	@echo "Removing core components from $(DESTDIR)$(PREFIX)..."
	rm -f $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so
	rm -f $(DESTDIR)$(INCLUDEDIR)/unim.h
	rm -f $(DESTDIR)$(BINDIR)/unim-cli
	rm -f $(DESTDIR)$(BINDIR)/unim-config
	rm -f $(DESTDIR)$(BINDIR)/unim-indicator
	rm -f $(DESTDIR)$(LIBEXECDIR)/unim-daemon
	rm -f $(DESTDIR)$(LIBEXECDIR)/unim-xim
	rm -f $(DESTDIR)$(LIBEXECDIR)/unim-wayland
	rm -f $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.conf
	rm -f $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc
	rm -f $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service

uninstall-frontends:
	@echo "Removing IM modules..."
	rm -f $(DESTDIR)$(GTK3_IMMODULE_DIR)/libim-unim.so
	rm -f $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim-gtk4.so
	rm -f $(DESTDIR)$(QT5_PLUGIN_DIR)/libunim.so
	rm -f $(DESTDIR)$(QT6_PLUGIN_DIR)/libunim.so

uninstall-settings:
	@echo "Removing settings tools..."
	rm -f $(DESTDIR)$(BINDIR)/unim-gtk-settings
	rm -f $(DESTDIR)$(BINDIR)/unim-qt-settings

uninstall-icons:
	@echo "Removing icons..."
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-hangul.svg
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-latin.svg

uninstall-autostart:
	@echo "Removing autostart entry..."
	rm -f $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-indicator.desktop

# ─────────────────────────────────────────────────────────────────────────────
# GNOME Shell Extension (User-level installation)
# ─────────────────────────────────────────────────────────────────────────────

gnome-extension: build-rust
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building GNOME Shell Extension..."
	@echo "════════════════════════════════════════════════════════════"
	@echo "  → Copying unim-cli binary..."
	@mkdir -p unim-gnome-extension/bin
	@cp target/release/unim-cli unim-gnome-extension/bin/
	@echo "  → Compiling GSettings schema..."
	@glib-compile-schemas unim-gnome-extension/schemas 2>/dev/null || echo "Note: glib-compile-schemas not available"
	@echo "  → Compiling translations..."
	@if command -v msgfmt >/dev/null 2>&1; then \
		for po in unim-gnome-extension/po/*.po; do \
			lang=$$(basename $$po .po); \
			mkdir -p unim-gnome-extension/locale/$$lang/LC_MESSAGES; \
			msgfmt $$po -o unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo; \
		done; \
	else \
		echo "Warning: msgfmt not found, skipping translation compilation."; \
	fi

pack: gnome-extension
	@echo "Packing extension into $(ZIP_FILE)..."
	@rm -f $(ZIP_FILE)
	@cd unim-gnome-extension && zip -r ../$(ZIP_FILE) .

install-gnome-extension: gnome-extension
	@echo "Installing GNOME extension to user directory..."
	@INSTALL_DIR="$(HOME)/.local/share/gnome-shell/extensions/$(UUID)"; \
	mkdir -p "$$INSTALL_DIR"; \
	cp -rf unim-gnome-extension/* "$$INSTALL_DIR/"; \
	echo "Compiling schemas in target directory..."; \
	glib-compile-schemas "$$INSTALL_DIR/schemas" || (echo "Error: Failed to compile schemas"; exit 1)
	@echo "✅ GNOME Extension 설치 완료!"

uninstall-gnome-extension:
	@echo "Uninstalling GNOME extension..."
	@rm -rf "$(HOME)/.local/share/gnome-shell/extensions/$(UUID)"
	@echo "✅ GNOME Extension 제거 완료!"

enable-gnome-extension:
	@echo "Enabling extension..."
	@gnome-extensions enable $(UUID)

disable-gnome-extension:
	@echo "Disabling extension..."
	@gnome-extensions disable $(UUID)

log-gnome-extension:
	@echo "Showing logs..."
	@journalctl -f -o cat /usr/bin/gnome-shell

# ─────────────────────────────────────────────────────────────────────────────
# Debian Packaging
# ─────────────────────────────────────────────────────────────────────────────

deb: build gnome-extension
	@echo "════════════════════════════════════════════════════════════"
	@echo "📦 Building Debian packages..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p $(DEB_DIR)
	@dpkg-buildpackage -us -uc -b
	@echo "  → Moving .deb files to $(DEB_DIR)..."
	@mv -f ../*.deb $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../*.ddeb $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../*.changes $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../*.buildinfo $(DEB_DIR)/ 2>/dev/null || true
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ Debian packages saved to $(DEB_DIR)/"
	@echo "════════════════════════════════════════════════════════════"
	@ls -la $(DEB_DIR)/

clean-deb:
	@echo "Cleaning Debian build artifacts..."
	@rm -rf $(DEB_DIR)
	@rm -f ../*.deb ../*.ddeb ../*.changes ../*.buildinfo ../*.tar.gz ../*.dsc

# ─────────────────────────────────────────────────────────────────────────────
# Test & Verification
# ─────────────────────────────────────────────────────────────────────────────

test:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔍 UNIM 입력기 설치 상태 확인"
	@echo "════════════════════════════════════════════════════════════"
	@echo ""
	@echo "✅ 1. 코어 라이브러리"
	@if [ -f $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so ]; then \
		echo "   ✓ libunim_capi.so 설치됨"; \
	else \
		echo "   ✗ libunim_capi.so 미설치"; \
	fi
	@echo ""
	@echo "✅ 2. GTK IM 모듈"
	@if [ -f $(DESTDIR)$(GTK3_IMMODULE_DIR)/libim-unim.so ]; then \
		echo "   ✓ GTK3 모듈 설치됨"; \
	else \
		echo "   ✗ GTK3 모듈 미설치"; \
	fi
	@if [ -f $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim-gtk4.so ]; then \
		echo "   ✓ GTK4 모듈 설치됨"; \
	else \
		echo "   ✗ GTK4 모듈 미설치"; \
	fi
	@echo ""
	@echo "✅ 3. Qt IM 플러그인"
	@if [ -f $(DESTDIR)$(QT5_PLUGIN_DIR)/libunim.so ]; then \
		echo "   ✓ Qt5 플러그인 설치됨"; \
	else \
		echo "   ✗ Qt5 플러그인 미설치"; \
	fi
	@if [ -f $(DESTDIR)$(QT6_PLUGIN_DIR)/libunim.so ]; then \
		echo "   ✓ Qt6 플러그인 설치됨"; \
	else \
		echo "   ✗ Qt6 플러그인 미설치"; \
	fi
	@echo ""
	@echo "✅ 4. CLI 및 데몬"
	@for cmd in unim-cli unim-config unim-indicator; do \
		if [ -f $(DESTDIR)$(BINDIR)/$$cmd ]; then \
			echo "   ✓ $$cmd 설치됨"; \
		else \
			echo "   ✗ $$cmd 미설치"; \
		fi; \
	done
	@echo ""
	@echo "════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Clean
# ─────────────────────────────────────────────────────────────────────────────

clean:
	@echo "Cleaning build artifacts..."
	@# GNOME extension artifacts
	@rm -f $(ZIP_FILE)
	@rm -rf unim-gnome-extension/bin
	@rm -f unim-gnome-extension/schemas/gschemas.compiled
	@rm -rf unim-gnome-extension/locale
	@# Frontend build directories
	@rm -rf unim-frontends/gtk3/build
	@rm -rf unim-frontends/gtk4/build
	@rm -rf unim-frontends/qt5/build
	@rm -rf unim-frontends/qt6/build
	@# Settings build directories
	@rm -rf unim-gtk-settings/build
	@rm -rf unim-qt-settings/build
	@# Test apps
	@rm -rf unim-test-gtk3/build
	@rm -rf unim-test-gtk4/build
	@echo "Done."

clean-all: clean clean-deb
	@echo "Cleaning Rust target directory..."
	@$(CARGO) clean
	@echo "All build artifacts cleaned."

# ─────────────────────────────────────────────────────────────────────────────
# Test Applications
# ─────────────────────────────────────────────────────────────────────────────

test-gtk4:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building GTK4 Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-gtk4/build && cd unim-test-gtk4/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-gtk4/build/unim-test-gtk4"
	@echo "   또는: GTK_IM_MODULE=unim ./unim-test-gtk4/build/unim-test-gtk4"
	@echo "════════════════════════════════════════════════════════════"

test-gtk3:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building GTK3 Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-gtk3/build && cd unim-test-gtk3/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-gtk3/build/unim-test-gtk3"
	@echo "   또는: GTK_IM_MODULE=unim ./unim-test-gtk3/build/unim-test-gtk3"
	@echo "════════════════════════════════════════════════════════════"

test-q6:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Qt6 Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-qt6/build && cd unim-test-qt6/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-qt6/build/unim-test-qt6"
	@echo "   또는: QT_IM_MODULE=unim ./unim-test-qt6/build/unim-test-qt6"
	@echo "════════════════════════════════════════════════════════════"

test-q5:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Qt5 Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-qt5/build && cd unim-test-qt5/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-qt5/build/unim-test-qt5"
	@echo "   또는: QT_IM_MODULE=unim ./unim-test-qt5/build/unim-test-qt5"
	@echo "════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# DBus / XIM Testing
# ─────────────────────────────────────────────────────────────────────────────

test-dbus: build-rust
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔍 DBus 서비스 검증..."
	@echo "════════════════════════════════════════════════════════════"
	@./target/debug/unim-daemon -n &
	@sleep 2
	@echo "✅ DBus 버스 이름 확인:"
	@busctl --user list 2>/dev/null | grep -i unim || echo "   ⚠️  unim 서비스 없음"
	@echo ""
	@echo "✅ DBus 인터페이스 확인:"
	@busctl --user introspect org.atit.unim.InputMethod /org/atit/unim/InputMethod 2>/dev/null | head -15 || echo "   ⚠️  introspect 실패"
	@pkill -f "unim-daemon" 2>/dev/null || true
	@echo "════════════════════════════════════════════════════════════"

test-xim:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building XIM Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-xim/build && cd unim-test-xim/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-xim/build/unim-test-xim"
	@echo "   또는: XMODIFIERS=@im=unim ./unim-test-xim/build/unim-test-xim"
	@echo "════════════════════════════════════════════════════════════"