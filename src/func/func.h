#ifndef BANANA_SRC_FUNC_FUNC_H
#define BANANA_SRC_FUNC_FUNC_H


#include "../banana.h"


//  === parse/parse.hで定義 ===
typedef struct CalculNode CalculNode;


typedef struct {
    CalculNode *process_ptr;
} ProcessList;

typedef struct {
    char *name;
    ProcessList *process;
    int process_length;
} FuncBlock;


void add_func(char *name);
void add_func_process(CalculNode *process);
FuncBlock* get_func_data(char *func_name);

//  === 関数を実行することが、できる ===
void func_eval(char *func_name);


#endif