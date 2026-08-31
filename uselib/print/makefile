CC = gcc
AR = ar

CFLAGS = -O2 -Wall -Wextra

C_SRC = c/print.c
ASM_SRC = asm/linux_write.s

C_OBJ = print.o
ASM_OBJ = linux_write.o

OBJ = $(C_OBJ) $(ASM_OBJ)

TARGET = libruntime.a

all: $(TARGET)

print.o: $(C_SRC)
	$(CC) $(CFLAGS) -c $< -o $@

linux_write.o: $(ASM_SRC)
	$(CC) -c $< -o $@

$(TARGET): $(OBJ)
	$(AR) rcs $@ $(OBJ)

clean:
	rm -f $(OBJ) $(TARGET)

.PHONY: all clean