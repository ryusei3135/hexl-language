#ifndef BANANA_SRC_EXTERN_LIB_LOAD_H
#define BANANA_SRC_EXTERN_LIB_LOAD_H


#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

void load();


#endif