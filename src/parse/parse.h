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
    String,
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
    OpRet,
} OpType;

//  この構造体のポインタの最初は、必ずargs.lengthが来る
typedef struct ArgsNode {
    union {
        //  長さを格納する
        int length;
        char *name;
        CalculNode *value;
    };
} ArgsNode;

//  関数を呼ぶ際に、関数の名前や、ライブラリの場合
//  どのライブラリなのかを格納
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
CalculNode* make_ret_expr(Token *token_list_ptr, int *pos);
char* make_func_header(Token *token_list_ptr, int *pos);
CalculNode* make_call_func_node(Token *token_list_ptr, int *pos);


//  === calcul.c ===
CalculNode *parse_operator(Token *token_list_ptr, int *pos);
CalculNode *parse_assign_var(Token *token_list_ptr, int *pos);
int calcul_eval(CalculNode* n);
void free_all_calcul_node(CalculNode *n);

//  === parse.c
OpType soring_operator_token_type(char *token_text);
void assign_process_for_func(CalculNode *process, int indent_len);
void make_process_data(Token *token_list_ptr);


#endif
