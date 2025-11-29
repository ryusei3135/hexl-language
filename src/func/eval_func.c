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

// 反復処理を実行
// int loop_expr_eval(FuncBlock *data, int *count) {
//     (*count)++;
//     int start_pos;
//     if (data->process[*count - 1].process_ptr->type == OpLoop) {
// loop_cond: // loop文が終わったら、ここに戻り条件がまだtrueか調べる
//         if (calcul_eval(data->process[*count - 1].process_ptr)) {
//             manage_indent_len(data->process[*count - 1].process_ptr->indent_len);
//             start_pos = *count;
//
//             while (data->process_length > start_pos) {
//                 eval_func_block(data, &start_pos);
//                 //  もし、loop文の外に出たり、関数の最後になった場合、loop_condに戻る
//                 if (data->process[start_pos].process_ptr->indent_len == manage_indent_len(-1)) {
//                     goto loop_cond;
//                 }
//                 if (data->process_length == start_pos + 1) {
//                     goto loop_cond;
//                 }
//                 start_pos++;
//             }
//         }
//     }
//     (*count) = start_pos - 1;
//     return 1;
// }

// //  関数の処理を実行
// int eval_func_block(FuncBlock *data, int *count) {
//     if (data->process[*count].process_ptr->type == OpLoop) {
//         loop_expr_eval(data, count);
//         (*count)++;
//     } else if (data->process[*count].process_ptr->type == OpIf \
//             || data->process[*count].process_ptr->type == OpIfElse \
//             || data->process[*count].process_ptr->type == OpElse) {
//         cond_expr_eval(data, count);
//         (*count)++;
//     }
//
//     if (get_skip_indent_len(-1) != data->process[*count].process_ptr->indent_len) {
//         calcul_eval(data->process[*count].process_ptr);
//     } else if (data->process[*count].process_ptr->type == OpRet) {
//         //  もし戻り値を返す処理なら、1を返す
//         return 1;
//     } else {
//         (*count)++;
//     }
//     return 0;
// }

//  rustから、呼び出される
void execute_one_line(ProcessList *node, int pos) {
    calcul_eval(node[pos].process_ptr);
}
