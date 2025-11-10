#include "func.h"


int func_eval(CallFuncNode *call_data, ArgsNode *args) {
    if (!strcmp(call_data->lib_header, "[local]")) {
        FuncBlock *data = get_func_data(call_data->func_name);
        int skip_indent = 0;
        int skip_indent_process_count = 0;
        int indent_len = 0;
        int loop_start = -1;

        int hoge = 0;

        for (int count = 0; data->process_length > count; count++) {
            if (!indent_len) {
                indent_len = data->process[count].process_ptr->indent_len;
            }

            if (loop_start != -1) {
                if (indent_len != data->process[count].process_ptr->indent_len) {
                    calcul_eval(data->process[count].process_ptr);
                    continue;
                } else {
                    //  もし、loop文の条件がtrueなら、処理を最初に戻す
                    if (calcul_eval(data->process[loop_start].process_ptr)) {
                        count = loop_start;
                        continue;
                    } else {
                        loop_start = -1;
                    }
                }
            }

            if (indent_len != data->process[count].process_ptr->indent_len) {
                if (skip_indent && indent_len != data->process[count].process_ptr->indent_len) {
                    continue;
                }
            }

            if (data->process[count].process_ptr->indent_len == skip_indent) {
                skip_indent_process_count++;
                continue;
            } else {
                if (skip_indent_process_count) {
                    skip_indent = 0;
                    skip_indent_process_count = 0;
                }
            }

            if (data->process[count].process_ptr->type == OpIf) {
                if (!calcul_eval(data->process[count].process_ptr->left)) {
                    skip_indent = data->process[count + 1].process_ptr->indent_len;
                }
            }

            //  反復処理
            if (data->process[count].process_ptr->type == OpLoop) {
                if (calcul_eval(data->process[count].process_ptr)) {
                    loop_start = count;
                    continue;
                } else if (data->process[count].process_ptr->indent_len != indent_len){
                    skip_indent = data->process[count].process_ptr->indent_len;
                }
            }
            //  処理を実行
            calcul_eval(data->process[count].process_ptr);
        }
    } else {
        eval_lib_func(call_data->func_name, call_data->lib_header, args);
    }

    return 1;
}
