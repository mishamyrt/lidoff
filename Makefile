# lidoff - MacBook lid angle brightness daemon
VERSION = 0.3.2

# Compilation variables
ifeq ($(origin CC), default)
CC = clang
endif
FRAMEWORKS = -framework IOKit -framework Foundation -framework CoreFoundation -framework CoreGraphics
CFLAGS = \
	-Wall \
	-Wextra \
	-Os \
	-flto \
	-fobjc-arc \
	-DNDEBUG \
	-DVERSION=\"$(VERSION)\"
LDFLAGS = -Wl,-dead_strip
BREW ?= $(shell command -v brew 2>/dev/null)
LLVM_PREFIX ?= $(shell if [ -n "$(BREW)" ]; then brew --prefix llvm 2>/dev/null; fi)
LLVM_BIN ?= $(if $(strip $(LLVM_PREFIX)),$(LLVM_PREFIX)/bin,)
CLANG_TIDY ?= $(shell command -v clang-tidy 2>/dev/null)
SCAN_BUILD ?= $(shell command -v scan-build 2>/dev/null)
SDK_ROOT ?= $(shell xcrun --show-sdk-path 2>/dev/null)

ifeq ($(strip $(CLANG_TIDY)),)
ifneq ($(strip $(LLVM_BIN)),)
CLANG_TIDY := $(LLVM_BIN)/clang-tidy
endif
endif

ifeq ($(strip $(SCAN_BUILD)),)
ifneq ($(strip $(LLVM_BIN)),)
SCAN_BUILD := $(LLVM_BIN)/scan-build
endif
endif

# Source and build directories
SRC_DIR = src
BUILD_DIR = build
SOURCES = \
	$(SRC_DIR)/main.m \
	$(SRC_DIR)/launch_agent.m \
	$(SRC_DIR)/logging.m \
	$(SRC_DIR)/monitor.m \
	$(SRC_DIR)/recovery_state.m \
	$(SRC_DIR)/external_display.m \
	$(SRC_DIR)/lid_sensor.m \
	$(SRC_DIR)/brightness.m \
	$(SRC_DIR)/caffeinate.m \
	$(SRC_DIR)/external_display_gamma.m \
	$(SRC_DIR)/external_display_mirroring.m \
	$(SRC_DIR)/external_display_skylight.m \
	$(SRC_DIR)/power_observer.m
SOURCE_HEADERS = \
	$(SRC_DIR)/launch_agent.h \
	$(SRC_DIR)/logging.h \
	$(SRC_DIR)/monitor.h \
	$(SRC_DIR)/recovery_state.h \
	$(SRC_DIR)/lid_sensor.h \
	$(SRC_DIR)/brightness.h \
	$(SRC_DIR)/caffeinate.h \
	$(SRC_DIR)/external_display_backend.h \
	$(SRC_DIR)/external_display.h \
	$(SRC_DIR)/power_observer.h
TARGET = $(BUILD_DIR)/lidoff
LINT_SOURCES = $(filter %.m,$(SOURCES))
CLANG_TIDY_FLAGS = $(CFLAGS) -I$(SRC_DIR) -isysroot $(SDK_ROOT) -x objective-c
SCAN_BUILD_DIR = $(BUILD_DIR)/scan-build

.PHONY: all
all: $(TARGET) ## build the daemon

.PHONY: clean
clean: ## clean build directory
	rm -rf $(BUILD_DIR)

.PHONY: install
install: $(TARGET) ## install the daemon
	rm -f "$(HOME)/.local/bin/lidoff"
	cp $(TARGET) "$(HOME)/.local/bin/lidoff"

.PHONY: lint
lint: ## run clang-tidy for Objective-C sources
	@if ! command -v "$(CLANG_TIDY)" >/dev/null 2>&1; then \
		echo "clang-tidy not found. Install LLVM with Homebrew: brew install llvm" >&2; \
		exit 1; \
	fi
	@if [ -z "$(SDK_ROOT)" ]; then \
		echo "macOS SDK not found. Install Xcode Command Line Tools." >&2; \
		exit 1; \
	fi
	@for file in $(LINT_SOURCES); do \
		echo "clang-tidy $$file"; \
		"$(CLANG_TIDY)" "$$file" -- $(CLANG_TIDY_FLAGS); \
	done

.PHONY: analyze
analyze: ## run Clang Static Analyzer via scan-build
	@if ! command -v "$(SCAN_BUILD)" >/dev/null 2>&1; then \
		echo "scan-build not found. Install LLVM with Homebrew: brew install llvm" >&2; \
		exit 1; \
	fi
	@rm -rf "$(SCAN_BUILD_DIR)"
	@$(MAKE) clean
	@"$(SCAN_BUILD)" --status-bugs -o "$(SCAN_BUILD_DIR)" --use-analyzer "$$(xcrun --find clang)" $(MAKE) all

.PHONY: check
check: ## run lint and static analysis
	@$(MAKE) lint
	@$(MAKE) analyze

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

$(TARGET): $(SOURCES) $(SOURCE_HEADERS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) $(FRAMEWORKS) -o $@ \
	$(SOURCES)
