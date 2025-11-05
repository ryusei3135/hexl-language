#ifndef BANANA_SRC_EXTERN_LIB_LOAD_H
#define BANANA_SRC_EXTERN_LIB_LOAD_H


#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

#include "../banana.h"


// === manage_lib.cpp ===
// bananaファイルでは、なくc言語などで作った
// 関数を実行、読み込みする
void load_lib_func(char *path, char *name);
void eval_lib_func(char *name);


#endif