extern "C" {
    #include "load.h"
}


typedef struct {
    void *ptr;
    char *func_name;
} LibFuncBlock;


void *load(char *path, char *name) {
#if defined(_WIN32)
    HMODULE handle = LoadLibraryA(path);
    if (!handle) {
        exit(1);
    }
    void *ptr = GetProcAddress(handle, name);
#else
    void *handle = dlopen(path, RTLD_LAZY);
    if (!handle) {
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


class ManageLibFuncs {
public:
    void add_func(char *path, char *name) {
        this->blocks[this->block_length - 1].ptr = load(path, name);
        this->blocks[this->block_length - 1].func_name \
                = (char *)malloc((int)strlen(name));
        
        strcpy(this->blocks[this->block_length - 1].func_name, name);

        this->block_length++;
        this->blocks = (LibFuncBlock *)realloc(
                this->blocks, 
                sizeof(LibFuncBlock) 
                * this->block_length);
    }

    void* get_lib_func_ptr(char *name) {
        int func_pos = 0;

        if ((func_pos = this->search_lib_func(name)) >= 0) {
            return this->blocks[func_pos].ptr;
        }

        exit(1);
    }

    ManageLibFuncs() {
        this->blocks = (LibFuncBlock *)malloc(sizeof(LibFuncBlock));
        this->block_length = 1;
    }

    ~ManageLibFuncs() {
        for (int count = 0; this->block_length > count; count++) {
            free(this->blocks[count].func_name);
        }

        free(this->blocks);
    }
private:
    int search_lib_func(char *name) {
        for (int count = 0; this->block_length > count; count++) {
            if (!strcmp(this->blocks[count].func_name, name)) {
                return count;
            }
        }
        return -1;
    }
    LibFuncBlock *blocks;
    int block_length;
};


ManageLibFuncs* access_lib_func() {
    static ManageLibFuncs lib_funcs;
    return &lib_funcs;
}

void load_lib_func(char *path, char *name) {
    access_lib_func()->add_func(path, name);
}

// === 外部関数を呼び出す ===
extern "C" void eval_lib_func(char *name) {
    void *ptr = access_lib_func()->get_lib_func_ptr(name);
    char *msg = (char *)malloc(12);
    strcpy(msg, "hello world");
    int result;

#if defined(_WIN32)
    asm volatile(
        "mov %[text], %%rcx\n\t"
        "call *%[fptr]\n\t"
        "mov %%eax, %[res]\n\t"
        : [res] "=r"(result)
        : [text] "r"(msg), [fptr] "r"(ptr)
        : "rax", "rcx"
    );
#else
    asm volatile(
        "mov %[text], %%rdi\n\t"
        "call *%[fptr]\n\t"
        "mov %%eax, %[res]\n\t"
        : [res] "=r"(result)
        : [text] "r"(msg), [fptr] "r"(ptr)
        : "rax", "rdi"
    );
#endif
}