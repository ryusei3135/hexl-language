#include "token.h"


char last_token_chr(char text) {
    static char t = '\0';
    if (text == '\0') {
        return t;
    } else {
        t = text;
    }
    return t;
}

TokenType last_token_type(TokenType type) {
    static TokenType last_type = TypeNull;
    if (type != TypeNull) {
        if (last_type != type) {
            last_type = type;
        }
        return last_type;
    }
    return last_type;
}


int _is_normal(char chr) {
    if (isalpha(chr) || chr == '_' || last_token_type(TypeNull) == TypeNormal && isdigit(chr)) {
        last_token_type(TypeNormal);
        return 1;
    }
    return 0;
}

int _is_number(char chr) {
    if (isdigit(chr)) {
        last_token_type(TypeNumber);
        return 1;
    }
    return 0;
}

int _is_space(char chr) {
    if (isspace(chr)) {
        last_token_type(TypeSpace);
        return 1;
    }
    return 0;
}

int _is_symbol(char chr) {
    if (chr == '"') {
        return 0;
    }
    if (ispunct(chr)) {
        return 1;
    }
    return 0;
}

TokenType is_token_type(char chr) {
    if (_is_number(chr)) {
        return TypeNumber;
    } else if (_is_normal(chr)) {
        return TypeNormal;
    } else if (_is_space(chr)) {
        return TypeSpace;
    } else if (_is_symbol(chr)) {
        return TypeSymbol;
    } else if (chr == '"') {
        return TypeString;
    }

    return TypeEnd;
}

int is_token(char chr, TokenType type) {
    static int lparen = 0;
    static int rparen = 0;

    int result = 0;
    switch (type) {
        case TypeNormal:
            result = _is_normal(chr);
            break;
        case TypeNumber:
            result = _is_number(chr);
            break;
        case TypeSpace:
            result = _is_space(chr);
            break;
        case TypeSymbol:
            result = _is_symbol(chr);
            break;
        default:
            if (chr == '"') {
                result = 1;
            }
            break;
    }
    last_token_chr(chr);
    return result;
}



TokenType change_op_symbol(char *value) {
    if (!strcmp(value, "(")) {
        return TypeLparen;
    } else if (!strcmp(value, ")")) {
        return TypeRparen;
    } else if (!strcmp(value, ".")) {
        return TypeComma;
    } else if (!strcmp(value, ",")) {
        return TypePeriod;
    } else if (!strcmp(value, "{")) {
        return TypeLbrace;
    } else if (!strcmp(value, "}")) {
        return TypeRbrace;
    }

    //  ===  算術演算子 ===
    if (!strcmp(value, "+")) {
        return TypeOpAdd;
    } else if (!strcmp(value, "-")) {
        return TypeOpSub;
    } else if (!strcmp(value, "*")) {
        return TypeOpMul;
    } else if (!strcmp(value, "/")) {
        return TypeOpDiv;
    } else if (!strcmp(value, "=")) {
        return TypeOpAssign;
    }

    if (!strcmp(value, "::")) {
        return TypeLibSpace;
    }

    return TypeSymbol;
}

TokenType statement_sorting(char *token) {
    if (!strcmp(token, "def")) {
        return TypeFunc;
    } else if (!strcmp(token, "import")) {
        return TypeImport;
    } else if (!strcmp(token, "if")) {
        return TypeCondExpr;
    } else if (!strcmp(token, "else")) {
        return TypeCondElse;
    } else if (!strcmp(token, "loop")) {
        return TypeLoopExpr;
    } else if (!strcmp(token, "ret")) {
        return TypeReturnExpr;
    }

    return TypeNormal;
}
