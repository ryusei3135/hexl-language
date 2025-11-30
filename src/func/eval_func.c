#include "func.h"


// 条件分岐の処理
int check_cond_expr(FuncBlock *data, int process_num) {
    static int skip_if_else = 0;
    int cond_type = 0;

    if (data->process[process_num].process_ptr->type == OpIf) {
        skip_if_else = 0;
    }

    if (!calcul_eval(data->process[process_num].process_ptr)) {
        goto result_false;
    } else {
        if (!skip_if_else && data->process[process_num].process_ptr->type == OpElse) {
            goto result_true;
        }
    }

    if (skip_if_else == 0) {
        if (data->process[process_num].process_ptr->type == OpIf) {
            skip_if_else = 1;
            cond_type = 1;
            goto result_true;
        } else if (data->process[process_num].process_ptr->type == OpIfElse) {
            skip_if_else = 1;
            cond_type = 2;
            goto result_true;
        }
    }

    goto result_false;
result_true:
    return cond_type;
result_false:
    // -2 は、条件分岐の中の処理をスキップする命令
    return -2;
}

//  制御構文の条件が、falseの場合制御構文の中の
//  処理をすべて飛ばすことができる関数
int skip_next_len_indent(FuncBlock *data, int pos) {
    int start_len = data->process[pos].process_ptr->indent_len;
    pos++;

    while (data->process_length >= pos) {
        //  インデントをスキップし終えたら、その場所を返す
        if (data->process[pos].process_ptr->indent_len == start_len) {
            return pos;
        }
        
        pos++;
    }

    return pos - 1;
}

//  rustから、呼び出される
void execute_one_line(ProcessList *node, int pos) {
    calcul_eval(node[pos].process_ptr);
}
