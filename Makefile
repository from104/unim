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

# Systemd user service directory
SYSTEMD_USER_DIR ?= $(REAL_LIBDIR)/systemd/user

# GNOME Extension settings
UUID := unim-indicator@from104.github.io
VERSION := $(shell sed -n 's/.*"version": "\([^"]*\)".*/\1/p' unim-gnome-extension/metadata.json)
ZIP_FILE := $(UUID)-$(VERSION).zip
GNOME_EXTENSION_DIR := $(DATADIR)/gnome-shell/extensions/$(UUID)

# Build paths
CAPI_LIB := $(CURDIR)/target/release/libunim_capi.so
CAPI_INC := $(CURDIR)/unim-capi/include
DEB_DIR := $(CURDIR)/debs

CARGO := cargo

.PHONY: all clean clean-all build build-rust build-frontends build-settings build-tests \
        install install-core install-frontends install-settings install-icons install-autostart install-gnome-extension install-extension \
        install-systemd uninstall-systemd enable-systemd disable-systemd status-systemd \
        uninstall uninstall-core uninstall-frontends uninstall-settings uninstall-icons uninstall-autostart uninstall-gnome-extension uninstall-extension \
        gnome-extension pack enable-gnome-extension disable-gnome-extension log-gnome-extension \
        deb clean-deb help \
        test test-gtk3 test-gtk4 test-qt5 test-qt6 test-gnome test-xim test-wayland test-dbus \
        sandbox sandbox-gtk3 sandbox-gtk4 sandbox-qt5 sandbox-qt6 sandbox-xim sandbox-indicator \
        dev-gtk3 dev-gtk4 dev-qt5 dev-qt6 dev-daemon dev-core dev-xim dev-wayland dev-indicator dev-extension dev-restart

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
	@echo "  install-extension         - Alias for install-gnome-extension"
	@echo "  uninstall-extension       - Alias for uninstall-gnome-extension"
	@echo "  pack                      - Create distributable .zip file"
	@echo ""
	@echo "Systemd User Service:"
	@echo "  install-systemd  - Install systemd user service file"
	@echo "  enable-systemd   - Enable and start the service"
	@echo "  disable-systemd  - Disable and stop the service"
	@echo "  status-systemd   - Show service status and recent logs"
	@echo ""
	@echo "Packaging:"
	@echo "  deb              - Build Debian packages (saved to ./debs/)"
	@echo ""
	@echo "Clean:"
	@echo "  clean            - Remove build artifacts"
	@echo "  clean-all        - Remove all artifacts including Cargo target"
	@echo ""
	@echo "Testing:"
	@echo "  test-gtk3        - Build and run GTK3 test application"
	@echo "  test-gtk4        - Build and run GTK4 test application"
	@echo "  test-qt5         - Build and run Qt5 test application"
	@echo "  test-qt6         - Build and run Qt6 test application"
	@echo "  test-gnome       - Build and run GNOME IME test application"
	@echo "  test-xim         - Build and run XIM test application"
	@echo "  test-wayland     - Build and run Wayland test application"
	@echo "  test-dbus        - Verify DBus service registration"
	@echo "  test             - Check installed UNIM components"
	@echo ""
	@echo "Sandbox (isolated testing):"
	@echo "  sandbox          - Launch Xephyr sandbox with default terminal"
	@echo "  sandbox-gtk3     - Launch sandbox with GTK3 test app"
	@echo "  sandbox-gtk4     - Launch sandbox with GTK4 test app"
	@echo "  sandbox-qt5      - Launch sandbox with Qt5 test app"
	@echo "  sandbox-qt6      - Launch sandbox with Qt6 test app"
	@echo "  sandbox-xim      - Launch sandbox with XIM test app"
	@echo "  sandbox-indicator- Launch sandbox with indicator test"
	@echo "  build-tests      - Build all test applications"
	@echo ""
	@echo "Quick Development (no deb needed, 최초 1회 install 필요):"
	@echo "  dev-gtk4         - GTK4 IM 모듈 증분 빌드 + 시스템 배포"
	@echo "  dev-gtk3         - GTK3 IM 모듈 증분 빌드 + 시스템 배포"
	@echo "  dev-qt5          - Qt5 플러그인 증분 빌드 + 시스템 배포"
	@echo "  dev-qt6          - Qt6 플러그인 증분 빌드 + 시스템 배포"
	@echo "  dev-core         - libunim_capi.so 빌드 + 배포"
	@echo "  dev-daemon       - unim-daemon 빌드 + 배포 + 재시작"
	@echo "  dev-xim          - unim-xim 빌드 + 배포 + 재시작"
	@echo "  dev-wayland      - unim-wayland 빌드 + 배포 + 재시작"
	@echo "  dev-indicator    - unim-indicator 빌드 + 배포 + 재시작"
	@echo "  dev-extension    - GNOME Extension 증분 빌드 + 로컬(User) 배포"
	@echo "  dev-restart      - 데몬 및 프론트엔드 재시작"
	@echo ""
	@echo "Variables:"
	@echo "  PREFIX           - Installation prefix (default: /usr/local)"
	@echo "  DESTDIR          - Staging directory for packaging"
	@echo ""
	@echo "Examples:"
	@echo "  make build"
	@echo "  sudo make install PREFIX=/usr"
	@echo "  sudo make uninstall PREFIX=/usr"
	@echo "  make sandbox-gtk4  (Xephyr sandbox with GTK4 test)"
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
		cmake .. && make
	@# GTK4 IM Module
	@echo "  → Building GTK4 IM Module..."
	@mkdir -p unim-frontends/gtk4/build && cd unim-frontends/gtk4/build && \
		cmake .. && make
	@# Qt5 IM Plugin
	@echo "  → Building Qt5 IM Plugin..."
	@mkdir -p unim-frontends/qt5/build && cd unim-frontends/qt5/build && \
		cmake .. && make
	@# Qt6 IM Plugin
	@echo "  → Building Qt6 IM Plugin..."
	@mkdir -p unim-frontends/qt6/build && cd unim-frontends/qt6/build && \
		cmake .. && make

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

install: install-core install-frontends install-settings install-icons install-autostart install-gnome-extension
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
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" im-config/25_unim.rc > $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc
	chmod 644 $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc
	@# DBus service (session bus auto-activation)
	install -d $(DESTDIR)$(DBUS_SERVICES_DIR)
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" scripts/org.atit.unim.InputMethod.service > $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service
	chmod 644 $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service

install-frontends:
	@echo "Installing IM modules..."
	@# GTK3
	install -d $(DESTDIR)$(GTK3_IMMODULE_DIR)
	install -m 755 unim-frontends/gtk3/build/libim-unim.so $(DESTDIR)$(GTK3_IMMODULE_DIR)/
	@# GTK4
	install -d $(DESTDIR)$(GTK4_IMMODULE_DIR)
	install -m 755 unim-frontends/gtk4/build/libim-unim.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/
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
	install -m 644 data/icons/unim-korean.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/
	install -m 644 data/icons/unim-english.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/

install-autostart:
	@echo "Installing autostart entry..."
	install -d $(DESTDIR)$(SYSCONFDIR)/xdg/autostart
	install -m 644 unim-indicator/data/unim-indicator.desktop $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/

# ─────────────────────────────────────────────────────────────────────────────
# Uninstall Targets
# ─────────────────────────────────────────────────────────────────────────────

uninstall: uninstall-core uninstall-frontends uninstall-settings uninstall-icons uninstall-autostart uninstall-gnome-extension
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
	rm -f $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim.so
	rm -f $(DESTDIR)$(QT5_PLUGIN_DIR)/libunim.so
	rm -f $(DESTDIR)$(QT6_PLUGIN_DIR)/libunim.so

uninstall-settings:
	@echo "Removing settings tools..."
	rm -f $(DESTDIR)$(BINDIR)/unim-gtk-settings
	rm -f $(DESTDIR)$(BINDIR)/unim-qt-settings

uninstall-icons:
	@echo "Removing icons..."
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-korean.svg
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-english.svg

uninstall-autostart:
	@echo "Removing autostart entry..."
	rm -f $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-indicator.desktop

# ─────────────────────────────────────────────────────────────────────────────
# Systemd User Service
# ─────────────────────────────────────────────────────────────────────────────

install-systemd:
	@echo "Installing systemd user service..."
	install -d $(DESTDIR)$(SYSTEMD_USER_DIR)
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" scripts/unim-daemon.service > $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service
	chmod 644 $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service
	@echo ""
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ Systemd 서비스 설치 완료!"
	@echo ""
	@echo "서비스 활성화:"
	@echo "  systemctl --user daemon-reload"
	@echo "  systemctl --user enable --now unim-daemon.service"
	@echo ""
	@echo "상태 확인:"
	@echo "  systemctl --user status unim-daemon.service"
	@echo "════════════════════════════════════════════════════════════"

uninstall-systemd:
	@echo "Removing systemd user service..."
	rm -f $(DESTDIR)$(SYSTEMD_USER_DIR)/unim-daemon.service
	@echo "서비스 비활성화: systemctl --user disable --now unim-daemon.service"

enable-systemd:
	@echo "Enabling and starting unim-daemon service..."
	systemctl --user daemon-reload
	systemctl --user enable --now unim-daemon.service
	@systemctl --user status unim-daemon.service --no-pager

disable-systemd:
	@echo "Disabling and stopping unim-daemon service..."
	systemctl --user disable --now unim-daemon.service

status-systemd:
	@systemctl --user status unim-daemon.service --no-pager || true
	@echo ""
	@journalctl --user -u unim-daemon.service -n 10 --no-pager || true

# ─────────────────────────────────────────────────────────────────────────────
# GNOME Shell Extension (User-level installation)
# ─────────────────────────────────────────────────────────────────────────────

gnome-extension:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building GNOME Shell Extension..."
	@echo "════════════════════════════════════════════════════════════"
	@echo "  → Copying icons from data/icons/..."
	@mkdir -p unim-gnome-extension/icons
	@cp data/icons/unim-korean.svg data/icons/unim-english.svg unim-gnome-extension/icons/
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
	@echo "Installing GNOME extension to $(DESTDIR)$(GNOME_EXTENSION_DIR)..."
	@install -d "$(DESTDIR)$(GNOME_EXTENSION_DIR)"
	@cp -rf unim-gnome-extension/* "$(DESTDIR)$(GNOME_EXTENSION_DIR)/"
	@echo "Compiling schemas in target directory..."
	@glib-compile-schemas "$(DESTDIR)$(GNOME_EXTENSION_DIR)/schemas" || (echo "Error: Failed to compile schemas"; exit 1)
	@echo "✅ GNOME Extension 설치 완료!"

install-extension: install-gnome-extension
uninstall-extension: uninstall-gnome-extension

uninstall-gnome-extension:
	@echo "Uninstalling GNOME extension from $(DESTDIR)$(GNOME_EXTENSION_DIR)..."
	@rm -rf "$(DESTDIR)$(GNOME_EXTENSION_DIR)"
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
	@dpkg-buildpackage -us -uc -b -jauto
	@echo "  → Moving .deb files to $(DEB_DIR)..."
	@mv -f ../unim*.deb $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../unim*.ddeb $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../unim*.changes $(DEB_DIR)/ 2>/dev/null || true
	@mv -f ../unim*.buildinfo $(DEB_DIR)/ 2>/dev/null || true
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ Debian packages saved to $(DEB_DIR)/"
	@echo "════════════════════════════════════════════════════════════"
	@ls -la $(DEB_DIR)/

clean-deb:
	@echo "Cleaning Debian build artifacts..."
	@rm -rf $(DEB_DIR)
	@rm -f ../unim*.deb ../unim*.ddeb ../unim*.changes ../unim*.buildinfo ../unim*.tar.gz ../unim*.dsc

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
	@if [ -f $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim.so ]; then \
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
	@for libcmd in unim-daemon unim-xim unim-wayland; do \
		if [ -f $(DESTDIR)$(LIBEXECDIR)/$$libcmd ]; then \
			echo "   ✓ $$libcmd 설치됨"; \
		else \
			echo "   ✗ $$libcmd 미설치"; \
		fi; \
	done
	@echo ""
	@echo "✅ 5. 설정 도구"
	@for cmd in unim-gtk-settings unim-qt-settings; do \
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
	@rm -rf unim-gnome-extension/icons
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
	@rm -rf unim-test-qt5/build
	@rm -rf unim-test-qt6/build
	@rm -rf unim-test-gnome/build
	@echo "Done."

clean-all: clean clean-deb
	@echo "Cleaning Rust target directory..."
	@$(CARGO) clean
	@echo "All build artifacts cleaned."

# ─────────────────────────────────────────────────────────────────────────────
# Quick Development (incremental build + deploy, no deb needed)
# 사용 전 최초 1회: make build && sudo make install PREFIX=/usr
# ─────────────────────────────────────────────────────────────────────────────

dev-gtk4:
	@echo "🔧 [dev] GTK4 IM 모듈 증분 빌드 + 배포..."
	@cd unim-frontends/gtk4/build && make
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp unim-frontends/gtk4/build/libim-unim.so $(GTK4_IMMODULE_DIR)/
	@echo "✅ GTK4 모듈 배포 완료! GTK4 앱을 재시작하세요."

dev-gtk3:
	@echo "🔧 [dev] GTK3 IM 모듈 증분 빌드 + 배포..."
	@cd unim-frontends/gtk3/build && make
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp unim-frontends/gtk3/build/libim-unim.so $(GTK3_IMMODULE_DIR)/
	@echo "✅ GTK3 모듈 배포 완료! GTK3 앱을 재시작하세요."

dev-qt5:
	@echo "🔧 [dev] Qt5 플러그인 증분 빌드 + 배포..."
	@cd unim-frontends/qt5/build && make
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp unim-frontends/qt5/build/libunim.so $(QT5_PLUGIN_DIR)/
	@echo "✅ Qt5 플러그인 배포 완료! Qt5 앱을 재시작하세요."

dev-qt6:
	@echo "🔧 [dev] Qt6 플러그인 증분 빌드 + 배포..."
	@cd unim-frontends/qt6/build && make
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp unim-frontends/qt6/build/libunim.so $(QT6_PLUGIN_DIR)/
	@echo "✅ Qt6 플러그인 배포 완료! Qt6 앱을 재시작하세요."

dev-core:
	@echo "🔧 [dev] libunim_capi.so 빌드 + 배포..."
	@$(CARGO) build --release -p unim-capi
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp target/release/libunim_capi.so $(REAL_LIBDIR)/
	@echo "✅ 코어 라이브러리 배포 완료! 프론트엔드 모듈을 다시 빌드하세요."

dev-daemon:
	@echo "🔧 [dev] unim-daemon 빌드 + 배포 + 재시작..."
	@$(CARGO) build --release -p unim-daemon
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp target/release/unim-daemon $(LIBEXECDIR)/
	@echo "  → 데몬 재시작..."
	@pkill -f unim-daemon 2>/dev/null || true
	@sleep 1
	@echo "✅ 데몬 배포 완료! (DBus 자동활성화로 다음 요청 시 재시작됩니다)"

dev-xim:
	@echo "🔧 [dev] unim-xim 빌드 + 배포 + 재시작..."
	@$(CARGO) build --release -p unim-xim
	@echo "  → XIM 서버 종료..."
	@pkill -x unim-xim 2>/dev/null || true
	@sleep 0.5
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp target/release/unim-xim $(LIBEXECDIR)/
	@echo "✅ XIM 서버 배포 완료!"

dev-wayland:
	@echo "🔧 [dev] unim-wayland 빌드 + 배포 + 재시작..."
	@$(CARGO) build --release -p unim-wayland
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp target/release/unim-wayland $(LIBEXECDIR)/
	@echo "  → Wayland IM 재시작..."
	@pkill -f unim-wayland 2>/dev/null || true
	@echo "✅ Wayland IM 배포 완료!"

dev-indicator:
	@echo "🔧 [dev] unim-indicator 빌드 + 배포 + 재시작..."
	@$(CARGO) build --release -p unim-indicator
	@echo "  → 시스템에 복사 (sudo 필요)..."
	@sudo cp target/release/unim-indicator $(BINDIR)/
	@echo "  → 인디케이터 재시작..."
	@pkill -f unim-indicator 2>/dev/null || true
	@echo "✅ 인디케이터 배포 완료!"

dev-extension: gnome-extension
	@echo "🔧 [dev] GNOME Extension 로컬 배포 (User)..."
	@mkdir -p ~/.local/share/gnome-shell/extensions/$(UUID)
	@cp -rf unim-gnome-extension/* ~/.local/share/gnome-shell/extensions/$(UUID)/
	@glib-compile-schemas ~/.local/share/gnome-shell/extensions/$(UUID)/schemas 2>/dev/null || true
	@echo "✅ Extension 배포 완료! GNOME Shell을 재시작해 주세요 (X11: Alt+F2, r / Wayland: 로그아웃 후 재로그인)."

dev-restart:
	@echo "🔧 [dev] UNIM 데몬 및 프론트엔드 재시작..."
	@pkill -f unim-daemon 2>/dev/null || true
	@pkill -f unim-xim 2>/dev/null || true
	@pkill -f unim-wayland 2>/dev/null || true
	@pkill -f unim-indicator 2>/dev/null || true
	@sleep 1
	@echo "✅ 모든 UNIM 프로세스가 종료되었습니다. (DBus 자동활성화로 다음 요청 시 재시작됩니다)"

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

test-qt6:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Qt6 Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-qt6/build && cd unim-test-qt6/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-qt6/build/unim-test-qt6"
	@echo "   또는: QT_IM_MODULE=unim ./unim-test-qt6/build/unim-test-qt6"
	@echo "════════════════════════════════════════════════════════════"

test-qt5:
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

test-wayland: build-rust
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building Wayland Test Application (Rust)..."
	@echo "════════════════════════════════════════════════════════════"
	@$(CARGO) build --release -p unim-test-wayland
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./target/release/unim-test-wayland"
	@echo "   (Wayland 세션에서 실행하세요)"
	@echo "════════════════════════════════════════════════════════════"

test-gnome:
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building GNOME IME Test Application..."
	@echo "════════════════════════════════════════════════════════════"
	@mkdir -p unim-test-gnome/build && cd unim-test-gnome/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 빌드 완료! 실행: ./unim-test-gnome/build/unim-test-gnome"
	@echo "   (GTK_IM_MODULE이 자동 해제되어 GNOME Shell IME 경로를 테스트합니다)"
	@echo "════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Build All Test Applications
# ─────────────────────────────────────────────────────────────────────────────

build-tests: build-frontends
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔨 Building All Test Applications..."
	@echo "════════════════════════════════════════════════════════════"
	@# GTK3 Test
	@echo "  → Building GTK3 Test App..."
	@mkdir -p unim-test-gtk3/build && cd unim-test-gtk3/build && cmake .. && make
	@# GTK4 Test
	@echo "  → Building GTK4 Test App..."
	@mkdir -p unim-test-gtk4/build && cd unim-test-gtk4/build && cmake .. && make
	@# Qt5 Test
	@echo "  → Building Qt5 Test App..."
	@mkdir -p unim-test-qt5/build && cd unim-test-qt5/build && cmake .. && make
	@# Qt6 Test
	@echo "  → Building Qt6 Test App..."
	@mkdir -p unim-test-qt6/build && cd unim-test-qt6/build && cmake .. && make
	@# XIM Test
	@echo "  → Building XIM Test App..."
	@mkdir -p unim-test-xim/build && cd unim-test-xim/build && cmake .. && make
	@# Wayland Test
	@echo "  → Building Wayland Test App (Rust)..."
	@$(CARGO) build --release -p unim-test-wayland
	@# GNOME Test
	@echo "  → Building GNOME IME Test App..."
	@mkdir -p unim-test-gnome/build && cd unim-test-gnome/build && cmake .. && make
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 모든 테스트 앱 빌드 완료!"
	@echo "════════════════════════════════════════════════════════════"

# ─────────────────────────────────────────────────────────────────────────────
# Sandbox Environment (Xephyr-based isolated testing)
# ─────────────────────────────────────────────────────────────────────────────

sandbox: build build-tests
	@echo "════════════════════════════════════════════════════════════"
	@echo "🧪 Launching UNIM Sandbox Environment..."
	@echo "════════════════════════════════════════════════════════════"
	@echo ""
	@echo "이 명령은 Xephyr 기반 격리 환경에서 UNIM을 테스트합니다."
	@echo "시스템 IM 설정에 영향을 주지 않습니다."
	@echo ""
	@./scripts/sandbox.sh $(SANDBOX_APP)

sandbox-gtk3: build build-tests
	@./scripts/sandbox.sh gtk3

sandbox-gtk4: build build-tests
	@./scripts/sandbox.sh gtk4

sandbox-qt5: build build-tests
	@./scripts/sandbox.sh qt5

sandbox-qt6: build build-tests
	@./scripts/sandbox.sh qt6

sandbox-xim: build build-tests
	@./scripts/sandbox.sh xim

sandbox-indicator: build build-tests
	@echo "════════════════════════════════════════════════════════════"
	@echo "🧪 Launching UNIM Sandbox with Indicator..."
	@echo "════════════════════════════════════════════════════════════"
	@echo "stalonetray가 설치되어 있어야 합니다: sudo apt install stalonetray"
	@echo ""
	@./scripts/sandbox.sh --indicator