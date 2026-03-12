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
CARGO := cargo

# ─── Helpers ──────────────────────────────────────────────────────────────────

# Build a CMake project: $(call cmake_build,dir_path,label)
NPROC := $(shell nproc 2>/dev/null || echo 4)
define cmake_build
	@echo "  → Building $(2)..."
	@mkdir -p $(1)/build && cd $(1)/build && cmake .. && $(MAKE) -j$(NPROC) --no-print-directory
endef

# ─── Phony ───────────────────────────────────────────────────────────────────

.PHONY: all help build build-rust build-frontends build-tests clean clean-all \
        install install-core install-frontends install-icons install-gui-gtk install-gui-qt \
        install-gnome-extension install-extension install-systemd \
        uninstall uninstall-core uninstall-frontends uninstall-icons uninstall-gui-gtk uninstall-gui-qt \
        uninstall-gnome-extension uninstall-extension uninstall-systemd \
        enable-systemd disable-systemd status-systemd \
        gnome-extension pack enable-gnome-extension disable-gnome-extension log-gnome-extension \
        deb clean-deb test test-dbus dev-restart

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
	@echo "  dev-{gtk3,gtk4,qt5,qt6,core,daemon,xim,wayland,gui-gtk,gui-qt,extension,restart}"
	@echo ""
	@echo "  install-gnome-extension / uninstall-gnome-extension / pack"
	@echo "  install-systemd / enable-systemd / disable-systemd / status-systemd"
	@echo "  deb / clean / clean-all"

# ─── Build ───────────────────────────────────────────────────────────────────

all: build

build: build-rust build-frontends
	@echo "✅ UNIM 전체 빌드 완료!"

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

install: install-core install-gui-gtk install-gui-qt install-frontends install-icons install-gnome-extension
	@echo "✅ UNIM 설치 완료! (PREFIX=$(PREFIX))"

install-core:
	@echo "Installing core components..."
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(REAL_LIBDIR) $(DESTDIR)$(LIBEXECDIR) \
	           $(DESTDIR)$(INCLUDEDIR) $(DESTDIR)$(IM_CONFIG_DATA_DIR) $(DESTDIR)$(DBUS_SERVICES_DIR)
	install -m 755 target/release/libunim_capi.so $(DESTDIR)$(REAL_LIBDIR)/
	install -m 644 unim-capi/include/unim.h $(DESTDIR)$(INCLUDEDIR)/
	install -m 755 target/release/unim-cli target/release/unim-config $(DESTDIR)$(BINDIR)/
	install -m 755 target/release/unim-daemon target/release/unim-xim target/release/unim-wayland $(DESTDIR)$(LIBEXECDIR)/
	install -m 644 im-config/25_unim.conf $(DESTDIR)$(IM_CONFIG_DATA_DIR)/
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" im-config/25_unim.rc > $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc && chmod 644 $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc
	sed "s|@LIBEXECDIR@|$(LIBEXECDIR)|g" scripts/org.atit.unim.InputMethod.service > $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service && chmod 644 $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service

install-frontends:
	@echo "Installing IM modules..."
	install -d $(DESTDIR)$(GTK3_IMMODULE_DIR) $(DESTDIR)$(GTK4_IMMODULE_DIR) \
	           $(DESTDIR)$(QT5_PLUGIN_DIR) $(DESTDIR)$(QT6_PLUGIN_DIR)
	install -m 755 unim-frontends/gtk3/build/libim-unim.so $(DESTDIR)$(GTK3_IMMODULE_DIR)/
	install -m 755 unim-frontends/gtk4/build/libim-unim.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/
	install -m 755 unim-frontends/qt5/build/libunim.so $(DESTDIR)$(QT5_PLUGIN_DIR)/
	install -m 755 unim-frontends/qt6/build/libunim.so $(DESTDIR)$(QT6_PLUGIN_DIR)/

install-icons:
	install -d $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps
	install -m 644 data/icons/unim-korean.svg data/icons/unim-english.svg $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/

install-gui-gtk:
	install -d $(DESTDIR)$(BINDIR) $(DESTDIR)$(SYSCONFDIR)/xdg/autostart
	-install -m 755 target/release/unim-gui-gtk $(DESTDIR)$(BINDIR)/ 2>/dev/null || true
	-install -m 644 unim-gui-gtk/data/unim-gui-gtk.desktop $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/ 2>/dev/null || true

install-gui-qt:
	install -d $(DESTDIR)$(BINDIR)
	-install -m 755 target/release/unim-gui-qt $(DESTDIR)$(BINDIR)/ 2>/dev/null || true

# ─── Uninstall ───────────────────────────────────────────────────────────────

uninstall: uninstall-core uninstall-gui-gtk uninstall-gui-qt uninstall-frontends uninstall-icons uninstall-gnome-extension
	@echo "✅ UNIM 제거 완료!"

uninstall-core:
	rm -f $(DESTDIR)$(REAL_LIBDIR)/libunim_capi.so $(DESTDIR)$(INCLUDEDIR)/unim.h \
	      $(DESTDIR)$(BINDIR)/unim-cli $(DESTDIR)$(BINDIR)/unim-config \
	      $(DESTDIR)$(LIBEXECDIR)/unim-daemon $(DESTDIR)$(LIBEXECDIR)/unim-xim $(DESTDIR)$(LIBEXECDIR)/unim-wayland \
	      $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.conf $(DESTDIR)$(IM_CONFIG_DATA_DIR)/25_unim.rc \
	      $(DESTDIR)$(DBUS_SERVICES_DIR)/org.atit.unim.InputMethod.service

uninstall-frontends:
	rm -f $(DESTDIR)$(GTK3_IMMODULE_DIR)/libim-unim.so $(DESTDIR)$(GTK4_IMMODULE_DIR)/libim-unim.so \
	      $(DESTDIR)$(QT5_PLUGIN_DIR)/libunim.so $(DESTDIR)$(QT6_PLUGIN_DIR)/libunim.so

uninstall-icons:
	rm -f $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-korean.svg \
	      $(DESTDIR)$(DATADIR)/icons/hicolor/scalable/apps/unim-english.svg

uninstall-gui-gtk:
	rm -f $(DESTDIR)$(BINDIR)/unim-gui-gtk $(DESTDIR)$(SYSCONFDIR)/xdg/autostart/unim-gui-gtk.desktop

uninstall-gui-qt:
	rm -f $(DESTDIR)$(BINDIR)/unim-gui-qt

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
	@cp data/icons/unim-korean.svg data/icons/unim-english.svg unim-gnome-extension/icons/
	@glib-compile-schemas unim-gnome-extension/schemas 2>/dev/null || true
	@if command -v msgfmt >/dev/null 2>&1; then \
		for po in unim-gnome-extension/po/*.po; do \
			lang=$$(basename $$po .po); \
			mkdir -p unim-gnome-extension/locale/$$lang/LC_MESSAGES; \
			msgfmt $$po -o unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo; \
		done; \
	fi

pack: gnome-extension
	@rm -f $(ZIP_FILE) && cd unim-gnome-extension && zip -r ../$(ZIP_FILE) .

install-gnome-extension: gnome-extension
	@install -d "$(DESTDIR)$(GNOME_EXTENSION_DIR)"
	@cp -rf unim-gnome-extension/* "$(DESTDIR)$(GNOME_EXTENSION_DIR)/"
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

deb: build gnome-extension
	@mkdir -p $(DEB_DIR) && dpkg-buildpackage -us -uc -b -jauto
	@mv -f ../*.deb ../*.ddeb ../unim*.changes ../unim*.buildinfo $(DEB_DIR)/ 2>/dev/null || true
	@echo "✅ Debian packages: $(DEB_DIR)/" && ls -la $(DEB_DIR)/

clean-deb:
	@rm -rf $(DEB_DIR)
	@rm -f ../*.deb ../*.ddeb ../unim*.changes ../unim*.buildinfo ../unim*.tar.gz ../unim*.dsc

# ─── Test & Verification ─────────────────────────────────────────────────────

test:
	@echo "UNIM 설치 상태 확인"
	@for f in $(REAL_LIBDIR)/libunim_capi.so \
	          $(GTK3_IMMODULE_DIR)/libim-unim.so $(GTK4_IMMODULE_DIR)/libim-unim.so \
	          $(QT5_PLUGIN_DIR)/libunim.so $(QT6_PLUGIN_DIR)/libunim.so; do \
		printf "  %-55s %s\n" "$$f" "$$([ -f $(DESTDIR)$$f ] && echo '✓' || echo '✗')"; \
	done
	@for cmd in unim-cli unim-config unim-gui-gtk; do \
		printf "  %-55s %s\n" "$(BINDIR)/$$cmd" "$$([ -f $(DESTDIR)$(BINDIR)/$$cmd ] && echo '✓' || echo '✗')"; \
	done
	@for cmd in unim-daemon unim-xim unim-wayland; do \
		printf "  %-55s %s\n" "$(LIBEXECDIR)/$$cmd" "$$([ -f $(DESTDIR)$(LIBEXECDIR)/$$cmd ] && echo '✓' || echo '✗')"; \
	done

# CMake-based test apps (static pattern rule)
test-gtk3 test-gtk4 test-qt5 test-qt6 test-xim test-gnome: test-%:
	$(call cmake_build,tests/unim-test-$*,$* test app)
	@echo "✅ Run: ./tests/unim-test-$*/build/unim-test-$*"

test-wayland: build-rust
	@$(CARGO) build --release -p unim-test-wayland
	@echo "✅ Run: ./target/release/unim-test-wayland"

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
	        tests/unim-test-qt5/build tests/unim-test-qt6/build tests/unim-test-gnome/build

clean-all: clean clean-deb
	@$(CARGO) clean

# ─── Quick Dev (requires initial: make build && sudo make install PREFIX=/usr)

dev-gtk3:
	@cd unim-frontends/gtk3/build && $(MAKE) --no-print-directory
	@sudo cp unim-frontends/gtk3/build/libim-unim.so $(GTK3_IMMODULE_DIR)/
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

dev-daemon:
	@$(CARGO) build --release -p unim-daemon
	@sudo cp target/release/unim-daemon $(LIBEXECDIR)/
	@pkill -f unim-daemon 2>/dev/null || true
	@echo "✅ 데몬 배포 완료! (DBus 자동활성화)"

dev-xim:
	@$(CARGO) build --release -p unim-xim
	@pkill -x unim-xim 2>/dev/null || true
	@sleep 0.5
	@sudo cp target/release/unim-xim $(LIBEXECDIR)/
	@echo "✅ XIM 서버 배포 완료!"

dev-wayland:
	@$(CARGO) build --release -p unim-wayland
	@sudo cp target/release/unim-wayland $(LIBEXECDIR)/
	@pkill -f unim-wayland 2>/dev/null || true
	@echo "✅ Wayland IM 배포 완료!"

dev-gui-gtk:
	@$(CARGO) build --release -p unim-gui-gtk
	@sudo cp target/release/unim-gui-gtk $(BINDIR)/
	@pkill -f unim-gui-gtk 2>/dev/null || true
	@echo "✅ unim-gui-gtk 배포 완료!"

dev-gui-qt:
	@$(CARGO) build --release -p unim-gui-qt
	@sudo cp target/release/unim-gui-qt $(BINDIR)/
	@pkill -f unim-gui-qt 2>/dev/null || true
	@echo "✅ unim-gui-qt 배포 완료!"

dev-extension: gnome-extension
	@mkdir -p ~/.local/share/gnome-shell/extensions/$(UUID)
	@cp -rf unim-gnome-extension/* ~/.local/share/gnome-shell/extensions/$(UUID)/
	@glib-compile-schemas ~/.local/share/gnome-shell/extensions/$(UUID)/schemas 2>/dev/null || true
	@echo "✅ Extension 배포 완료! GNOME Shell 재시작 필요."

dev-restart:
	@pkill -f unim-daemon 2>/dev/null; pkill -f unim-xim 2>/dev/null; \
	 pkill -f unim-wayland 2>/dev/null; pkill -f unim-gui-gtk 2>/dev/null; \
	 pkill -f unim-gui-qt 2>/dev/null; sleep 1
	@echo "✅ 모든 UNIM 프로세스 종료 (DBus 자동활성화)"
