#ifndef BANANA_SRC_PARSE_PARSE_H
#define BANANA_SRC_PARSE_PARSE_H


#include "../banana.h"


#define TokenEndCond(type) (type != TypeEnd && type != TypeLbrace)


typedef enum {
    Add,
    Sub,
    Mul,
    Div,

    Num,
    Value,
    CallFunc,
    AssignVar,
    CallVar,
    //  ===  関係演算子 ===
    TypeOpEpual,
    TypeOpBigger,
    TypeOpSmallerThen,
    TypeOpHigher,
    TypeOpBelow,
    TypeOpIsNot,
    //  === 論理演算子 ===
    TypeOpAnd,
    TypeOpOr,
    // === 制御構文 ===
    OpIf,
    OpIfElse,
    OpElse,
    OpLoop,
} OpType;

typedef struct ArgsNode {
    char *arg_value;
    OpType op_type;
} ArgsNode;

typedef struct CallFuncNode {
    char *func_name;
    char *lib_header;
} CallFuncNode;

typedef struct CalculNode {
    int indent_len;
    union {
        char *value;
        CallFuncNode *call_data;
    };
    OpType type;
    union {
        ArgsNode *args;
        struct CalculNode *left;
    };
    struct CalculNode *right;
} CalculNode;


//  === statement.c ===
char* make_func_header(Token *token_list_ptr, int *pos);
CalculNode* make_call_func_node(Token *token_list_ptr, int *pos);


//  === calcul.c ===
CalculNode *parse_operator(Token *token_list_ptr, int *pos);
CalculNode *parse_assign_var(Token *token_list_ptr, int *pos);
int calcul_eval(CalculNode* n);
void free_all_calcul_node(CalculNode *n);

//  === parse.c
OpType soring_operator_token_type(char *token_text);
void make_process_data(Token *token_list_ptr);


#endif
