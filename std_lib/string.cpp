#include "vm.h"
#include <vector>
#include <cstdlib>
#include <string>
#include <cstring>


extern "C" VmArgsValue int_to_str(VmArgsValue *args, size_t len) {
    if (len == 1) {
        if (args[0].arg_type == Int32) {
            VmArgsValue ret;
            ret.arg_type = Str;
            ret.value.str_value = std::to_string(args[0].value.i32_value).data();
            return ret;
        }
    }
}

extern "C" VmArgsValue float_to_str(VmArgsValue *args, size_t len) {
    if (len == 1) {
        if (args[0].arg_type == Float32) {
            VmArgsValue ret;
            ret.arg_type = Str;
            ret.value.str_value = std::to_string(args[0].value.f32_value).data();
            return ret;
        }
    }
}

extern "C" VmArgsValue str_len(VmArgsValue *args, size_t len) {
    if (len == 1) {
        if (args[0].arg_type == Str) {
            VmArgsValue ret;
            ret.arg_type = Int32;
            ret.value.i32_value = std::strlen(s);
            return ret;
        }
    }
}
