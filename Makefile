# lidoff - MacBook lid angle brightness daemon
VERSION = 0.3.0

# Compilation variables
CC = clang
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

# Source and build directories
SRC_DIR = src
BUILD_DIR = build
SOURCES = \
	$(SRC_DIR)/main.m \
	$(SRC_DIR)/lid_sensor.m \
	$(SRC_DIR)/brightness.m \
	$(SRC_DIR)/caffeinate.m \
	$(SRC_DIR)/external_display_gamma.m \
	$(SRC_DIR)/external_display_mirroring.m \
	$(SRC_DIR)/external_display_skylight.m
SOURCE_HEADERS = \
	$(SRC_DIR)/lid_sensor.h \
	$(SRC_DIR)/brightness.h \
	$(SRC_DIR)/caffeinate.h \
	$(SRC_DIR)/external_display.h
TARGET = $(BUILD_DIR)/lidoff

.PHONY: all
all: $(TARGET) ## build the daemon

.PHONY: clean
clean: ## clean build directory
	rm -rf $(BUILD_DIR)

.PHONY: install
install: $(TARGET) ## install the daemon
	rm -f "$(HOME)/.local/bin/lidoff"
	cp $(TARGET) "$(HOME)/.local/bin/lidoff"

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