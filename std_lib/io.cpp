#include <iostream>
#include "vm.h"


extern "C" VmArgsValue print(VmArgsValue *args, size_t len) {
    if (len >= 1) {
        if (args[0].arg_type == Str) {
            std::cout << args[0].value.str_value << std::endl;
        }
    }

    return VmArgsValue {
        Void,
        CValue { .void_value = 0 }
    };
}
