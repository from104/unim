SHELL := /bin/bash
UUID := unim-typefix@from104.github.io
VERSION := $(shell sed -n 's/.*"version": "\([^"]*\)".*/\1/p' unim-gnome-extension/metadata.json)
ZIP_FILE := $(UUID)-$(VERSION).zip

.PHONY: all clean build pack install uninstall enable disable log test

all: build

CARGO := cargo

build:
	@echo "Building unim-cli binary..."
	@$(CARGO) build -p unim-cli --release
	@echo "Copying binary to extension directory..."
	@mkdir -p unim-gnome-extension/bin
	@cp target/release/unim-cli unim-gnome-extension/bin/
	@echo "Compiling GSettings schema..."
	@glib-compile-schemas unim-gnome-extension/schemas 2>/dev/null || echo "Note: glib-compile-schemas not available in this environment"
	@echo "Compiling translations..."
	@if command -v msgfmt >/dev/null 2>&1; then \
		for po in unim-gnome-extension/po/*.po; do \
			lang=$$(basename $$po .po); \
			mkdir -p unim-gnome-extension/locale/$$lang/LC_MESSAGES; \
			msgfmt $$po -o unim-gnome-extension/locale/$$lang/LC_MESSAGES/$(UUID).mo; \
		done; \
	else \
		echo "Warning: msgfmt not found, skipping translation compilation. Please install gettext."; \
	fi


pack: build
	@echo "Packing extension into $(ZIP_FILE)..."
	@rm -f $(ZIP_FILE)
	@cd unim-gnome-extension && zip -r ../$(ZIP_FILE) .

install: build
	@echo "Installing extension manually..."
	@INSTALL_DIR="$(HOME)/.local/share/gnome-shell/extensions/$(UUID)"; \
	mkdir -p "$$INSTALL_DIR"; \
	cp -rf unim-gnome-extension/* "$$INSTALL_DIR/"; \
	echo "Compiling schemas in target directory..."; \
	glib-compile-schemas "$$INSTALL_DIR/schemas" || (echo "Error: Failed to compile schemas"; exit 1)

uninstall:
	@echo "Uninstalling extension..."
	@rm -rf "$(HOME)/.local/share/gnome-shell/extensions/$(UUID)"

enable:
	@echo "Enabling extension..."
	@gnome-extensions enable $(UUID)

disable:
	@echo "Disabling extension..."
	@gnome-extensions disable $(UUID)

log:
	@echo "Showing logs..."
	@journalctl -f -o cat /usr/bin/gnome-shell

test:
	@echo "Running extension tests..."
	@echo "════════════════════════════════════════════════════════════"
	@echo "🔍 unim-typefix GNOME Extension 테스트"
	@echo "════════════════════════════════════════════════════════════"
	@echo
	@echo "✅ 1. 설치 상태"
	@if [ -f ~/.local/share/gnome-shell/extensions/$(UUID)/extension.js ]; then \
		echo "   ✓ extension.js 설치됨"; \
	else \
		echo "   ✗ extension.js 미설치"; \
	fi
	@if [ -f ~/.local/share/gnome-shell/extensions/$(UUID)/prefs.js ]; then \
		echo "   ✓ prefs.js (설정 UI) 설치됨"; \
	else \
		echo "   ✗ prefs.js 미설치"; \
	fi
	@if [ -f ~/.local/share/gnome-shell/extensions/$(UUID)/schemas/gschemas.compiled ]; then \
		echo "   ✓ GSettings 스키마 컴파일됨"; \
	else \
		echo "   ✗ GSettings 스키마 미컴파일"; \
	fi
	@echo
	@echo "✅ 2. 활성화 상태"
	@if gnome-extensions list 2>/dev/null | grep -q "$(UUID)"; then \
		echo "   ✓ 익스텐션 활성화됨"; \
	else \
		echo "   ✗ 익스텐션 미활성화"; \
	fi
	@echo
	@echo "✅ 3. GSettings 스키마"
	@SCHEMA_DIR="$(HOME)/.local/share/gnome-shell/extensions/$(UUID)/schemas"; \
	export GSETTINGS_SCHEMA_DIR="$$SCHEMA_DIR:$$GSETTINGS_SCHEMA_DIR"; \
	if gsettings list-schemas 2>/dev/null | grep -q "org.gnome.shell.extensions.unim-typefix"; then \
		echo "   ✓ 스키마 등록됨"; \
		echo "   설정 값:"; \
		echo "   - enable-extension: $$(gsettings get org.gnome.shell.extensions.unim-typefix enable-extension 2>/dev/null || echo 'N/A')"; \
		echo "   - korean-layout: $$(gsettings get org.gnome.shell.extensions.unim-typefix korean-layout 2>/dev/null || echo 'N/A')"; \
	else \
		echo "   ✗ 스키마 미등록"; \
	fi
	@echo
	@echo "════════════════════════════════════════════════════════════"
	@echo "✅ 설치 및 활성화 완료!"
	@echo "════════════════════════════════════════════════════════════"

clean:
	@echo "Cleaning up..."
	@rm -f $(ZIP_FILE)
	@rm -rf unim-gnome-extension/bin
	@rm -f unim-gnome-extension/schemas/gschemas.compiled
	@rm -rf unim-gnome-extension/locale