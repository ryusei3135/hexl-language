#include "load.h"

void load() {
#if defined(_WIN32)
    HMODULE handle = LoadLibrary("../../../lib/print.dll");
    if (!handle) {
        return;
    }
    void (*hello)(const char *) = (void (*)(const char *))GetProcAddress(handle, "print");
#else
    void *handle = dlopen("../../../lib/print.so", RTLD_LAZY);
    if (!handle) {
        return;
    }
    void (*hello)(const char *) = (void (*)(const char *))dlsym(handle, "print");
#endif

    if (!hello) {
        return;
    }

    hello("world");

#if defined(_WIN32)
    FreeLibrary(handle);
#else
    dlclose(handle);
#endif
}