# lidoff - MacBook lid angle brightness daemon

CC = clang
FRAMEWORKS = -framework IOKit -framework Foundation -framework CoreFoundation -framework CoreGraphics
CFLAGS = -Wall -Wextra -Os -flto -fobjc-arc -DNDEBUG
LDFLAGS = -Wl,-dead_strip

SRC_DIR = src
BUILD_DIR = build
SOURCES = $(SRC_DIR)/main.m $(SRC_DIR)/lid_sensor.m $(SRC_DIR)/brightness.m
TARGET = $(BUILD_DIR)/lidoff

.PHONY: all clean install uninstall

all: $(TARGET)

$(BUILD_DIR):
	mkdir -p $(BUILD_DIR)

$(TARGET): $(SOURCES) | $(BUILD_DIR)
	$(CC) $(CFLAGS) $(LDFLAGS) $(FRAMEWORKS) -o $@ $(SOURCES)

clean:
	rm -rf $(BUILD_DIR)

install: $(TARGET)
	./$(TARGET) --install

uninstall:
	./$(TARGET) --uninstall
