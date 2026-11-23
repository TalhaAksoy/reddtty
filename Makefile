SHELL := /bin/sh

PREFIX ?= /usr/local
BINDIR ?= $(PREFIX)/bin
BINARY := reddtty
TARGET := target/release/$(BINARY)

.PHONY: build run test configs configs-list install uninstall clean help

# Optional non-interactive selection:
#   make configs CONFIG=redlib-zanity.toml
#   make configs CONFIG=2
CONFIG ?=

help:
	@printf '%s\n' \
		'ReddTTY targets:' \
		'  make build                 Build target/release/reddtty' \
		'  make run                   Run the release binary' \
		'  make test                  Run cargo test' \
		'  make install               Install reddtty to $(BINDIR)' \
		'  make uninstall             Remove only the installed binary' \
		'  make configs               Interactively install a config from configs/' \
		'  make configs CONFIG=NAME   Install configs/NAME.toml without prompting' \
		'  make configs CONFIG=2      Install the second listed config' \
		'  make configs-list          Only list available configs' \
		'  make clean                 Remove Cargo build output'

build:
	cargo build --release

run: build
	./$(TARGET) $(ARGS)

test:
	cargo test

install: build
	install -d "$(DESTDIR)$(BINDIR)"
	install -m 755 "$(TARGET)" "$(DESTDIR)$(BINDIR)/$(BINARY)"
	@printf 'Installed %s\n' "$(DESTDIR)$(BINDIR)/$(BINARY)"
	@printf 'Configuration and cache files were not touched.\n'

uninstall:
	rm -f "$(DESTDIR)$(BINDIR)/$(BINARY)"
	@printf 'Removed %s\n' "$(DESTDIR)$(BINDIR)/$(BINARY)"
	@printf 'Configuration, cache and user data were not touched.\n'

clean:
	cargo clean

configs-list:
	@set -eu; \
	files=$$(find configs -maxdepth 1 -type f -name '*.toml' -print | sort); \
	if [ -z "$$files" ]; then \
		printf 'No configs found in configs/.\n' >&2; \
		exit 1; \
	fi; \
	printf 'Available configs:\n'; \
	i=1; \
	for file in $$files; do \
		printf '  %2d) %s\n' "$$i" "$$(basename "$$file")"; \
		i=$$((i + 1)); \
	done

configs:
	@set -eu; \
	: "$${HOME:?HOME is not set}"; \
	files=$$(find configs -maxdepth 1 -type f -name '*.toml' -print | sort); \
	if [ -z "$$files" ]; then \
		printf 'No configs found in configs/.\n' >&2; \
		exit 1; \
	fi; \
	choice=''; \
	if [ -n "$(CONFIG)" ]; then \
		choice='$(CONFIG)'; \
	else \
		printf 'Available configs:\n'; \
		i=1; \
		for file in $$files; do \
			printf '  %2d) %s\n' "$$i" "$$(basename "$$file")"; \
			i=$$((i + 1)); \
		done; \
		printf 'Select config [1-%d]: ' $$((i - 1)); \
		read choice || true; \
	fi; \
	if [ -z "$$choice" ]; then \
		printf 'No config selected.\n' >&2; \
		exit 1; \
	fi; \
	case "$$choice" in \
		*[!0-9]*) selected="$$choice" ;; \
		*) selected=$$(printf '%s\n' $$files | sed -n "$${choice}p") ;; \
	esac; \
	if [ -z "$$selected" ]; then \
		printf 'Invalid selection: %s\n' "$$choice" >&2; \
		exit 1; \
	fi; \
	case "$$selected" in \
		*.toml) : ;; \
		*) selected="$$selected.toml" ;; \
	esac; \
	case "$$selected" in \
		*/*) : ;; \
		*) selected="configs/$$selected" ;; \
	esac; \
	if [ ! -f "$$selected" ]; then \
		printf 'Config not found: %s\n' "$$selected" >&2; \
		exit 1; \
	fi; \
	if [ -n "$${XDG_CONFIG_HOME:-}" ] && [ "$${XDG_CONFIG_HOME#/}" != "$$XDG_CONFIG_HOME" ]; then \
		config_root="$$XDG_CONFIG_HOME"; \
	else \
		config_root="$$HOME/.config"; \
	fi; \
	dest_dir="$$config_root/reddtty"; \
	dest="$$dest_dir/config.toml"; \
	mkdir -p "$$dest_dir"; \
	if [ -f "$$dest" ]; then \
		timestamp=$$(date +%s); \
		backup="$$dest.bckp_$$timestamp"; \
		suffix=1; \
		while [ -e "$$backup" ]; do \
			backup="$$dest.bckp_$${timestamp}_$$suffix"; \
			suffix=$$((suffix + 1)); \
		done; \
		cp -p "$$dest" "$$backup"; \
		chmod 600 "$$backup"; \
		printf 'Backed up existing config: %s\n' "$$backup"; \
	fi; \
	cp "$$selected" "$$dest"; \
	chmod 600 "$$dest"; \
	printf 'Installed: %s\n' "$$selected"; \
	printf 'Destination: %s\n' "$$dest"; \
	printf 'Run: reddtty\n'
