#include <iostream>
#include <fstream>
#include "vm.h"
#include <stdlib.h>


extern "C" VmArgsValue print(VmArgsValue *args, size_t len) {
    if (len >= 1) {
        if (args[0].arg_type == Str) {
            std::cout << args[0].value.str_value << std::endl;
        }
    }

    VmArgsValue ret;
    ret.arg_type = Void;
    ret.value.void_value = 0;
    return ret;
}

extern "C" VmArgsValue file_open(VmArgsValue *args, size_t len) {
    std::ifstream ifs(args[0].value.str_value);

    if (!ifs) {
        std::cerr << "ファイルを開けませんでした。" << std::endl;
        exit(1);
    }

    std::string line;
    std::string text;
    int count = 0;
    VmArgsValue ret;
    ret.arg_type = Str;
    // 2. 1行ずつ読み込み
    while (std::getline(ifs, line)) {
        if (count) {
            text += line;
        } else {
            text = line;
        }
        count++;
    }
    ret.value.str_value = text.data();

    // 3. ファイルを閉じる (デストラクタで自動的に閉じられるが、明示も可能)
    ifs.close();
    return ret;
}

extern "C" VmArgsValue file_line(VmArgsValue *args, size_t len) {
    std::ifstream ifs(args[0].value.str_value);

    if (!ifs) {
        std::cerr << "ファイルを開けませんでした。" << std::endl;
        exit(1);
    }

    std::string line;
    int count = 0;
    VmArgsValue ret;
    ret.arg_type = Int32;
    // 2. 1行ずつ読み込み
    while (std::getline(ifs, line)) {
        count++;
    }
    ret.value.i32_value = count;

    // 3. ファイルを閉じる (デストラクタで自動的に閉じられるが、明示も可能)
    ifs.close();
    return ret;
}

extern "C" VmArgsValue file_write(VmArgsValue *args, size_t len) {
    std::ofstream ofs(args[0].value.str_value);

    ofs << args[1].value.str_value;

    ofs.close(); // 明示的に閉じる（省略可）
    VmArgsValue ret;
    ret.arg_type = Void;
    ret.value.void_value = 0;
    return ret;
}
