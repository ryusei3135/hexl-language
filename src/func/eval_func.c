#include "func.h"

static int eval_func_block(FuncBlock *data, int *count);
// 条件分岐の処理
static int cond_expr_eval(FuncBlock *data, int *count) {
    //  条件分岐で条件がtrueになったときに、1になる、
    //  次に、if文が来たら、0になる
    static int skip_next_cond_expr = 0;
    //  if文が来たので、条件分岐をスキップする処理をやめる
    if (data->process[*count].process_ptr->type == OpIf) {
        skip_next_cond_expr = 0;
    }

    if (!skip_next_cond_expr) {
        if (data->process[*count].process_ptr->type == OpIf) {
            if (calcul_eval(data->process[*count].process_ptr)) {
                //  下にあるつながっている条件分岐をすべてスキップするために
                //  1にする。
                skip_next_cond_expr = 1;
                get_skip_indent_len(0);
                return 1;
            }
            get_skip_indent_len(data->process[*count + 1].process_ptr->indent_len);
        } else if (data->process[*count].process_ptr->type == OpIfElse) {
            if (calcul_eval(data->process[*count].process_ptr)) {
                skip_next_cond_expr = 1;
                get_skip_indent_len(0);
                return 1;
            }
            get_skip_indent_len(data->process[*count + 1].process_ptr->indent_len);
        } else if (data->process[*count].process_ptr->type == OpElse) {
            get_skip_indent_len(0);
            return 1;
        }
    } else { //  もしif else分が続いているときに、後ろにある条件をすべてスキップ
        get_skip_indent_len(data->process[*count + 1].process_ptr->indent_len);
    }

    return 0;
}

//  反復処理を実行
static int loop_expr_eval(FuncBlock *data, int *count) {
    (*count)++;
    int start_pos;
    if (data->process[*count - 1].process_ptr->type == OpLoop) {
loop_cond: // loop文が終わったら、ここに戻り条件がまだtrueか調べる
        if (calcul_eval(data->process[*count - 1].process_ptr)) {
            manage_indent_len(data->process[*count - 1].process_ptr->indent_len);
            start_pos = *count;

            while (data->process_length > start_pos) {
                eval_func_block(data, &start_pos);
                //  もし、loop文の外に出たり、関数の最後になった場合、loop_condに戻る
                if (data->process[start_pos].process_ptr->indent_len == manage_indent_len(-1)) {
                    goto loop_cond;
                }
                if (data->process_length == start_pos + 1) {
                    goto loop_cond;
                }
                start_pos++;
            }
        }
    }
    (*count) = start_pos - 1;
    return 1;
}

//  関数の処理を実行
static int eval_func_block(FuncBlock *data, int *count) {
    if (data->process[*count].process_ptr->type == OpLoop) {
        loop_expr_eval(data, count);
        (*count)++;
    } else if (data->process[*count].process_ptr->type == OpIf \
            || data->process[*count].process_ptr->type == OpIfElse \
            || data->process[*count].process_ptr->type == OpElse) {
        cond_expr_eval(data, count);
        (*count)++;
    }

    if (get_skip_indent_len(-1) != data->process[*count].process_ptr->indent_len) {
        calcul_eval(data->process[*count].process_ptr);
    } else if (data->process[*count].process_ptr->type == OpRet) {
        //  もし戻り値を返す処理なら、1を返す
        return 1;
    } else {
        (*count)++;
    }
    return 0;
}

void expand_args(ArgsNode *def, ArgsNode *value) {
    for (int count = 0; def[0].length >= count; count++) {
        if (count != 0) {
            add_variable_value(def[count].name, current_func_name("[null]"), value[count].value);
        }
    }
}

//  rustから、呼び出される
void execute_one_line(ProcessList *node, int pos) {
    calcul_eval(node[pos].process_ptr);
}

int func_eval(CallFuncNode *call_data, ArgsNode *args, char *caller_func) {
    //  変数のアクセス特権を現在実行中の関数にする
    current_func_name(call_data->func_name);

    if (!strcmp(call_data->lib_header, "[local]")) {
        FuncBlock *data = get_func_data(call_data->func_name);
        expand_args(data->args, args);

        for (int count = 0; data->process_length > count; count++) {
            if (eval_func_block(data, &count)) {
                current_func_name(caller_func);
                return calcul_eval(data->process[count].process_ptr);
            }
        }
    } else {
        //  外部の関数を呼び出す
        current_func_name(caller_func);
        return eval_lib_func(call_data->func_name, call_data->lib_header, args);
    }
    //  変数のアクセス特権を戻す
    current_func_name(caller_func);
    return 1;
}
