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


//  manage_func.cpp
ArgsNode** add_func(char *name);
void add_func_process(CalculNode *process);
FuncBlock* get_func_data(char *func_name);

char* current_func_name(char *func_name);

//eval_func.c
//  === 関数を実行することができる ===
int func_eval(CallFuncNode *call_data, ArgsNode *args, char *caller_func);


#endif
