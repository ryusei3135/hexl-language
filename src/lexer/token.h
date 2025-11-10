#ifndef BANANA_SRC_LEXER_TOKEN_H
#define BANANA_SRC_LEXER_TOKEN_H


typedef enum {
    TypeNormal,
    TypeNumber,
    TypeSpace,
    TypeSymbol,
    TypeString,
    TypeEnd,
    //  ===  算術演算子 ===
    TypeOpAdd,
    TypeOpSub,
    TypeOpMul,
    TypeOpDiv,

    TypeOpAssign,

    TypeRparen,
    TypeLparen,
    TypeComma,

    TypeFunc,
    TypeImport,
    TypeLibSpace,
    // === 制御構文 ===
    TypeCondExpr,
    TypeLoopExpr,

    TypeNull,
} TokenType;

typedef struct Token {
    char *token;
    TokenType type;
} Token;


#include "../banana.h"


char last_token_chr(char text);
TokenType last_token_type(TokenType type);
int _is_normal(char chr);
int _is_number(char chr);
int _is_space(char chr);
int _is_symbol(char chr);
TokenType is_token_type(char chr);
int is_token(char chr, TokenType type);

TokenType change_op_symbol(char *value);
TokenType statement_sorting(char *token);


void free_all_token_ptr(Token *token_list_ptr);
Token* make_token_list_ptr(char *buffer);
char* make_string_token(Token *token_list_ptr, int *pos);

#endif
