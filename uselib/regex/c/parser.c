#include "all.h"



CharOpt peek(Parser *this) {
    OpKind kind = Some;
    if (this->chars_len >= this->pos)
        kind = None;

    CharOpt opt = {
        this->chars[this->pos],
        kind
    };
    return opt;
}

CharOpt peek2(Parser *this) {
    OpKind kind = Some;
    if (this->chars_len >= this->pos + 1)
        kind = None;

    CharOpt opt = {
        this->chars[this->pos + 1],
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