# lidoff - MacBook lid angle brightness daemon

CC = clang
FRAMEWORKS = -framework IOKit -framework Foundation -framework CoreFoundation -framework CoreGraphics
CFLAGS = -Wall -Wextra -Os -flto -fobjc-arc -DNDEBUG
LDFLAGS = -Wl,-dead_strip

SRC_DIR = src
BUILD_DIR = build
SOURCES = \
	$(shell find src/ -type f -name '*.m')
SOURCE_HEADERS = \
	$(shell find src/ -type f -name '*.h')
TARGET = $(BUILD_DIR)/lidoff

.PHONY: all clean install uninstall

all: $(TARGET)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(TARGET): $(SOURCES) $(SOURCE_HEADERS) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) $(FRAMEWORKS) -o $@ \
	$(SOURCES)
clean:
	rm -rf $(BUILD_DIR)

install: $(TARGET)
	./$(TARGET) --install

uninstall:
	./$(TARGET) --uninstall
