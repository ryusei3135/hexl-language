extern "C" {
    #include "func.h"
}

//  もし、帰ってきた数が、-1なら実行しない行はない
static int* manage_skip_indent() {
    static int data;
    return &data;
}

void setting_skip_indent(int skip_indent) {
    (*manage_skip_indent()) = skip_indent;
}

int next_eval_indent_len() {
    return *manage_skip_indent();
}
