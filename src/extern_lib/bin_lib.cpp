extern "C" {
    #include "load.h"
}

#if defined(_WIN32)
#include <windows.h>
#else
#include <dlfcn.h>
#endif

typedef struct {
    void *ptr;
    char *header_name;
    char *func_name;
} LibFuncBlock;


void* load(char *path, char *name) {
#if defined(_WIN32)
    HMODULE handle = LoadLibraryA(path);
    if (!handle) {
        printf("this lib is not found %s\n", path);
        exit(1);
    }
    void *ptr = GetProcAddress(handle, name);
#else
    void *handle = dlopen(path, RTLD_LAZY);
    if (!handle) {
        printf("this lib is not found %s\n", path);
        exit(1);
    }
    void *ptr = dlsym(handle, name);
#endif

#if defined(_WIN32)
    FreeLibrary(handle);
#else
    dlclose(handle);
#endif
    return ptr;
}


class ManageBinLibFuncs {
public:
    void add_func(char *path, char *name, char *lib_header) {
        path = (char *)realloc(path, (int)strlen(path) + 3);
        strcat(path, ".so");
        this->blocks[this->block_length - 1].ptr = load(path, name);
        this->blocks[this->block_length - 1].func_name \
                = (char *)malloc((int)strlen(name));

        this->blocks[this->block_length - 1].header_name \
                = (char *)malloc((int)strlen(lib_header));

        strcpy(this->blocks[this->block_length - 1].header_name, lib_header);
        strcpy(this->blocks[this->block_length - 1].func_name, name);

        this->block_length++;
        this->blocks = (LibFuncBlock *)realloc(
                this->blocks,
                sizeof(LibFuncBlock)
                * this->block_length);
    }

    void* get_lib_func_ptr(char *name, char *lib_header) {
        int func_pos = 0;

        if ((func_pos = this->search_lib_func(name, lib_header)) >= 0) {
            return this->blocks[func_pos].ptr;
        }

        printf("this lib func is not found %s\n", name);
        exit(1);
    }

    ManageBinLibFuncs() {
        this->blocks = (LibFuncBlock *)malloc(sizeof(LibFuncBlock));
        this->block_length = 1;
    }

    ~ManageBinLibFuncs() {
        for (int count = 0; this->block_length > count; count++) {
            free(this->blocks[count].func_name);
        }

        free(this->blocks);
    }
private:
    int search_lib_func(char *name, char *lib_header) {
        for (int count = 0; this->block_length > count; count++) {
            if (
                    !strcmp(this->blocks[count].func_name, name)
                    && !strcmp(this->blocks[count].header_name, lib_header)
                ) {
                return count;
            }
        }
        return -1;
    }
    LibFuncBlock *blocks;
    int block_length;
};


ManageBinLibFuncs* access_lib_func() {
    static ManageBinLibFuncs lib_funcs;
    return &lib_funcs;
}

void load_lib_func(char *path, char *name, char *lib_header) {
    access_lib_func()->add_func(path, name, lib_header);
}

// === 外部関数を呼び出す ===
extern "C" int eval_lib_func(char *name, char *lib_header, ArgsNode *args) {
    void *ptr = access_lib_func()->get_lib_func_ptr(name, lib_header);
    int result;
    
#if defined(_WIN32)
    asm volatile(
        "mov %[text], %%rcx\n\t"
        "call *%[fptr]\n\t"
        "mov %%eax, %[res]\n\t"
        : [res] "=r"(result)
        : [text] "r"(args[1].value->value), [fptr] "r"(ptr)
        : "rax", "rcx"
    );
#else
    asm volatile(
        "mov %[text], %%rdi\n\t"
        "call *%[fptr]\n\t"
        "mov %%eax, %[res]\n\t"
        : [res] "=r"(result)
        : [text] "r"(args[1].value->value), [fptr] "r"(ptr)
        : "rax", "rdi"
    );
#endif
    return result;
}
