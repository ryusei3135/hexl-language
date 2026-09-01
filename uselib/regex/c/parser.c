#include "all.h"



CharOpt peek(Parser *this) {
    OpKind kind = Some;
    if (this->pos >= this->chars_len)
        kind = None;

    CharOpt opt = {
        kind == Some ? this->chars[this->pos] : 0,
        kind
    };
    return opt;
}

CharOpt peek2(Parser *this) {
    OpKind kind = Some;
    if (this->pos + 1 >= this->chars_len)
        kind = None;

    CharOpt opt = {
        kind == Some ? this->chars[this->pos + 1] : 0,
        kind
    };
    return opt;
}

CharOpt bump(Parser *this) {
    CharOpt c = peek(this);
    if (c.kind == Some) {
        this->pos += 1;
    }
    return c;
}

char match_chr(Parser *this, char chr) {
    if (peek(this).kind == None)
        return 0;
    if (peek(this).value == chr)
        return 1;
    return 0;
}

char match_chr_2(Parser *this, char chr) {
    if (peek2(this).kind == None)
        return 0;
    if (peek2(this).value == chr)
        return 1;
    return 0;
}

char unmatch_bump(Parser *this, char chr) {
    if (peek2(this).kind == None)
        return 0;
    if (peek2(this).value != chr)
        return 1;
    return 0;
}


#define ResultErrGen(msg)\
    CharResult result = {msg, Err};\
    return result;

#define ResultOkGen(c)\
    CharResult result = {c, Ok};\
    return result;

CharResult parse_class_char(Parser *this) {
    if (bump(this).kind == None) {
        ResultErrGen("'[' に対応する ']' がありません");
    }

    char c = peek(this).value;
    if (c == '\\') {
        if (bump(this).kind == None) {
            ResultErrGen("'[' に対応する ']' がありません");
        }
        ResultOkGen(change_byte_chr(this));
    }
    ResultOkGen(c);
}