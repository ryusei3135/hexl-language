#pragma once


struct Parser {
    char* chars;
    long chars_len;
    long pos;
    long group_count;
};


enum OpKind {
    None,
    Some,
};

typedef enum OpKind OpKind;

struct CharOpt
{
    char value;
    OpKind kind;
};


typedef struct Parser Parser;
typedef struct CharOpt CharOpt;

// asm/chr.s
char change_byte_chr(volatile Parser *);
int is_byte_digit(char);