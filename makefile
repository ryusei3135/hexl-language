CC = clang
CXX = clang++


SRC_DIR = src
LIB_DIR = lib
BUILD_DIR = build

TARGET = $(BUILD_DIR)/banana

UNAME := $(shell uname)

ifeq ($(UNAME), Linux)
	LIB_FLAGS = -shared -fPIC
endif

ifeq ($(UNAME), Darwin)
    LIB_FLAGS = -shared -fPIC
endif

ifeq ($(UNAME), Windows_NT)
    LIB_FLAGS = -shared
endif


# go build -buildmode=c-shared -o libhello.so hello.go


SRC_CC_FILES  := $(shell find $(SRC_DIR) -name '*.c')
SRC_CXX_FILES := $(shell find $(SRC_DIR) -name '*.cpp')
LIB_CC_FILES  := $(shell find $(LIB_DIR) -name '*.c')

# --- 出力するオブジェクト名（build に移動）---
SRC_OBJS := $(patsubst %.c,$(BUILD_DIR)/%.o,$(SRC_CC_FILES)) \
            $(patsubst %.cpp,$(BUILD_DIR)/%.o,$(SRC_CXX_FILES))

LIB_SOS := $(patsubst %.c,$(BUILD_DIR)/%.so,$(LIB_CC_FILES))

all: $(TARGET)

# --- src の .c / .cpp を .o にする ---
$(BUILD_DIR)/%.o: %.c
	mkdir -p $(dir $@)
	$(CC) -c $< -o $@

$(BUILD_DIR)/%.o: %.cpp
	mkdir -p $(dir $@)
	$(CXX) -c $< -o $@

# --- lib の .c を .so にする ---
$(BUILD_DIR)/%.so: %.c
	mkdir -p $(dir $@)
	$(CC) $(LIB_FLAGS) $< -o $@

# Rust静的ライブラリをビルド
# $(RUST_LIB):
# 	@echo "🦀 Building Rust static library..."
# 	cd rustlib && cargo build --release


# 最終リンク
$(TARGET): $(SRC_OBJS) $(LIB_SOS)
	mkdir -p $(BUILD_DIR)
	$(CXX) -o $@ $(SRC_OBJS) $(LIB_SOS)

clean:
	rm -f $(SRC_OBJS) $(LIB_SOS) $(TARGET)
# 	cd rustlib && cargo clean

test: $(TARGET)
	./$(TARGET) test_file/test.bnn

.PHONY: all clean