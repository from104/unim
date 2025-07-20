SHELL := /bin/bash
UUID := unim-autocorrect@from104.github.io
VERSION := $(shell sed -n 's/.*"version": "\([^"]*\)".*/\1/p' unim-gnome-extension/metadata.json)
ZIP_FILE := $(UUID)-$(VERSION).zip

.PHONY: all clean build pack install enable disable log

all: build

build:
	@echo "Building Rust core library..."
	@cd unim-core && cargo build --release
	@echo "Copying library to extension directory..."
	@mkdir -p unim-gnome-extension/lib
	@cp unim-core/target/release/libunim_core.so unim-gnome-extension/lib/

pack: build
	@echo "Packing extension into $(ZIP_FILE)..."
	@rm -f $(ZIP_FILE)
	@cd unim-gnome-extension && zip -r ../$(ZIP_FILE) .

install: build
	@echo "Installing extension manually..."
	@INSTALL_DIR="$(HOME)/.local/share/gnome-shell/extensions/$(UUID)"; \
	mkdir -p "$$INSTALL_DIR"; \
	cp -rf unim-gnome-extension/* "$$INSTALL_DIR/"

enable:
	@echo "Enabling extension..."
	@gnome-extensions enable $(UUID)

disable:
	@echo "Disabling extension..."
	@gnome-extensions disable $(UUID)

log:
	@echo "Showing logs..."
	@journalctl -f -o cat /usr/bin/gnome-shell

clean:
	@echo "Cleaning up..."
	@rm -f $(ZIP_FILE)
	@cd unim-core && cargo clean
	@rm -rf unim-gnome-extension/lib 