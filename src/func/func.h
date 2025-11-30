#ifndef BANANA_SRC_FUNC_FUNC_H
#define BANANA_SRC_FUNC_FUNC_H


#include "../banana.h"


//  === parse/parse.hで定義 ===
typedef struct CallFuncNode CallFuncNode;
typedef struct ArgsNode ArgsNode;
typedef struct CalculNode CalculNode;


typedef struct {
    CalculNode *process_ptr;
} ProcessList;

typedef struct {
    char *name;
    ProcessList *process;
    int process_length;
    ArgsNode *args;
} FuncBlock;

//  indent.cpp
#define MakeNewIndent 1
#define UpdateIndentValue 2
#define DelIndetValue 3

void assign_indent_value(int value, int process_num);
int get_now_indent_len();
int get_last_indent_len();
int get_now_indent_status();

//  manage_func.cpp
ArgsNode** add_func(char *name);
void add_func_process(CalculNode *process);
FuncBlock* get_func_data(char *func_name);

char* current_func_name(char *func_name);

//eval_func.c
//  条件分の結果を調べて実行可能か調べる
int check_cond_expr(FuncBlock *data, int process_num);
//  === 関数を実行することができる ===
int execute_func(CallFuncNode *call_data, ArgsNode *args, char *caller_func);


#endif
