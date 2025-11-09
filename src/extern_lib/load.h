#ifndef BANANA_SRC_EXTERN_LIB_LOAD_H
#define BANANA_SRC_EXTERN_LIB_LOAD_H


#include "../banana.h"


typedef struct Token Token;
typedef struct ArgsNode ArgsNode;


// === bin_lib.cpp ===
// bananaファイルでは、なくc言語などで作った
// 関数を実行、読み込みする
void load_lib_func(char *path, char *name, char *lib_header);
int eval_lib_func(char *name, char *lib_header, ArgsNode *args);

// === manage_lib.cpp ===
void import_lib(Token *token_list_ptr, int *pos);


#endif
