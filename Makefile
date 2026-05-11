# lidoff - MacBook lid angle brightness daemon
VERSION = 0.3.2

LLVM_BIN ?= $(if $(strip $(LLVM_PREFIX)),$(LLVM_PREFIX)/bin,)
CLANG_TIDY ?= $(shell command -v clang-tidy 2>/dev/null)
CLANG_FORMAT ?= $(shell command -v clang-format 2>/dev/null)
SCAN_BUILD ?= $(shell command -v scan-build 2>/dev/null)
SDK_ROOT ?= $(shell xcrun --show-sdk-path 2>/dev/null)

ifeq ($(strip $(CLANG_TIDY)),)
ifneq ($(strip $(LLVM_BIN)),)
CLANG_TIDY := $(LLVM_BIN)/clang-tidy
endif
endif

ifeq ($(strip $(CLANG_FORMAT)),)
ifneq ($(strip $(LLVM_BIN)),)
CLANG_FORMAT := $(LLVM_BIN)/clang-format
endif
endif

ifeq ($(strip $(SCAN_BUILD)),)
ifneq ($(strip $(LLVM_BIN)),)
SCAN_BUILD := $(LLVM_BIN)/scan-build
endif
endif

BUILD_DIR = build
TARGET = $(BUILD_DIR)/lidoff
CARGO_TARGET = target/release/lidoff
SCAN_BUILD_DIR = $(BUILD_DIR)/scan-build
DISPLAY_NATIVE_DIR = crates/lidoff-display/macos
POWER_NATIVE_DIR = crates/lidoff-power/src
C_SOURCES = \
	$(DISPLAY_NATIVE_DIR)/brightness.c \
	$(DISPLAY_NATIVE_DIR)/external_display.c \
	$(DISPLAY_NATIVE_DIR)/external_display_gamma.c \
	$(DISPLAY_NATIVE_DIR)/external_display_skylight.c \
	$(POWER_NATIVE_DIR)/caffeinate.c \
	$(POWER_NATIVE_DIR)/power_observer.c
CLANG_TIDY_FLAGS = \
	-std=c11 \
	-Wall \
	-Wextra \
	-Os \
	-DNDEBUG \
	-isysroot $(SDK_ROOT) \
	-I$(DISPLAY_NATIVE_DIR) \
	-I$(POWER_NATIVE_DIR)
CLANG_TIDY_CHECKS = -*,clang-analyzer-*

.PHONY: all
all: $(TARGET) ## build the daemon

.PHONY: clean
clean: ## clean build artifacts
	cargo clean
	rm -rf $(BUILD_DIR)

.PHONY: install
install: $(TARGET) ## install the daemon
	rm -f "$(HOME)/.local/bin/lidoff"
	cp $(TARGET) "$(HOME)/.local/bin/lidoff"

.PHONY: fmt
fmt: ## run cargo fmt and clang-format
	@cargo fmt --all
	@find \
		crates/ \
		-iname '*.h' -o -iname '*.c' -o -iname '*.m' \
		| xargs clang-format -i

.PHONY: lint
lint: ## run cargo clippy and clang-tidy
	cargo clippy --workspace --all-targets
	@if ! command -v "$(CLANG_TIDY)" >/dev/null 2>&1; then \
		echo "clang-tidy not found. Install LLVM with Homebrew: brew install llvm" >&2; \
		exit 1; \
	fi
	@if [ -z "$(SDK_ROOT)" ]; then \
		echo "macOS SDK not found. Install Xcode Command Line Tools." >&2; \
		exit 1; \
	fi
	@for file in $(C_SOURCES); do \
		echo "clang-tidy $$file"; \
		"$(CLANG_TIDY)" "$$file" --checks='$(CLANG_TIDY_CHECKS)' -- $(CLANG_TIDY_FLAGS); \
	done

.PHONY: test
test: ## run Rust tests
	cargo test --workspace

.PHONY: analyze
analyze: ## run Clang Static Analyzer via scan-build
	@if ! command -v "$(SCAN_BUILD)" >/dev/null 2>&1; then \
		echo "scan-build not found. Install LLVM with Homebrew: brew install llvm" >&2; \
		exit 1; \
	fi
	rm -rf "$(SCAN_BUILD_DIR)"
	"$(SCAN_BUILD)" --status-bugs -o "$(SCAN_BUILD_DIR)" cargo build --release -p lidoff

.PHONY: check
check: ## run formatting, lint, tests, and static analysis
	$(MAKE) fmt
	$(MAKE) lint
	$(MAKE) test
	$(MAKE) analyze

.PHONY: publish
publish: ## publish the daemon
	git tag "v$(VERSION)"
	git-cliff -o CHANGELOG.md
	git tag -d "v$(VERSION)"
	git add Makefile CHANGELOG.md
	git commit -m "chore: release v$(VERSION)"
	git tag "v$(VERSION)"
	git push
	git push --tags

.PHONY: help
help: ## print this message
	@echo "Usage: make <command>"
	@echo "Available commands:"
	@awk \
		'BEGIN {FS = ":.*?## "} \
		/^[a-zA-Z_-]+:.*?## / \
		{printf "\033[36m%-15s\033[0m %s\n", $$1, $$2}' \
		$(MAKEFILE_LIST)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(TARGET): | $(BUILD_DIR)
	cargo build --release -p lidoff
	cp $(CARGO_TARGET) $(TARGET)
