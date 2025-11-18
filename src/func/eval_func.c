#include "func.h"


//  スキップするインデントの長さを保持
static int get_skip_indent_len(int next_skip_indent) {
    static int skip_indent = 0;
    //  -1 は、インデントの長さを取得したいとき
    if (next_skip_indent == -1) {
        return skip_indent;
    } else {
        skip_indent = next_skip_indent;
        return skip_indent;
    }
}

//  インデントの長さを管理
static int manage_indent_len(int update_indent) {
    static int indent = 0;

    if (update_indent == -1) {
        return indent;
    } else {
        indent = update_indent;
        return indent;
    }
}


static void eval_func_block(FuncBlock *data, int *count);

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
    if (data->process[*count].process_ptr->type == OpLoop) {
    loop_cond: // loop文が終わったら、ここに戻り条件がまだtrueか調べる
        if (calcul_eval(data->process[*count].process_ptr)) {
            manage_indent_len(data->process[*count].process_ptr->indent_len);
            int start_pos = *count + 1;

            while (data->process_length > start_pos) {
                // loop 分の外に出たら、最初に戻る
                if (data->process[start_pos].process_ptr->indent_len == manage_indent_len(-1)) {
                    goto loop_cond;
                }
                eval_func_block(data, &start_pos);
                start_pos++;
            }
        }
    }

    return 1;
}

static void eval_func_block(FuncBlock *data, int *count) {
runing:
    if (data->process[*count].process_ptr->type == OpLoop) {
        loop_expr_eval(data, count);
        (*count)++;
        goto runing;
    } else if (data->process[*count].process_ptr->type == OpIf \
            || data->process[*count].process_ptr->type == OpIfElse \
            || data->process[*count].process_ptr->type == OpElse) {
        cond_expr_eval(data, count);
        (*count)++;
        goto runing;
    }

    if (get_skip_indent_len(-1) != data->process[*count].process_ptr->indent_len) {
        calcul_eval(data->process[*count].process_ptr);
    }
}

void expand_args(ArgsNode *def, ArgsNode *value) {
    for (int count = 0; def[0].length >= count; count++) {
        if (count != 0) {
            add_variable_value(def[count].name, current_func_name("[null]"), value[count].value);
        }
    }
}

int func_eval(CallFuncNode *call_data, ArgsNode *args, char *caller_func) {
    //  変数のアクセス特権を現在実行中の関数にする
    current_func_name(call_data->func_name);

    if (!strcmp(call_data->lib_header, "[local]")) {
        FuncBlock *data = get_func_data(call_data->func_name);
        expand_args(data->args, args);

        for (int count = 0; data->process_length > count; count++) {
            eval_func_block(data, &count);
        }
    } else {
        //  外部の関数を呼び出す
        eval_lib_func(call_data->func_name, call_data->lib_header, args);
    }
    //  変数のアクセス特権を戻す
    current_func_name(caller_func);
    return 1;
}
