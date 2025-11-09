#include "func.h"


int func_eval(CallFuncNode *call_data, ArgsNode *args) {
    if (!strcmp(call_data->lib_header, "[local]")) {
        FuncBlock *data = get_func_data(call_data->func_name);
        int skip_indent_len = 0;
        int skip_indent_process_count = 0;
        int indent_len = 0;

        for (int count = 0; data->process_length > count; count++) {
            if (!indent_len) {
                indent_len = data->process[count].process_ptr->indent_len;
            }

            if (indent_len != data->process[count].process_ptr->indent_len) {
                if (skip_indent_len && indent_len != data->process[count].process_ptr->indent_len) {
                    continue;
                }
            }

            if (data->process[count].process_ptr->indent_len == skip_indent_len) {
                skip_indent_process_count++;
                continue;
            } else {
                if (skip_indent_process_count) {
                    skip_indent_len = 0;
                    skip_indent_process_count = 0;
                }
            }

            if (data->process[count].process_ptr->type == OpIf) {
                if (!calcul_eval(data->process[count].process_ptr->left)) {
                    skip_indent_len = data->process[count + 1].process_ptr->indent_len;
                }
            }
            printf("%d value\n", calcul_eval(data->process[count].process_ptr));
        }
    } else {
        eval_lib_func(call_data->func_name, call_data->lib_header, args);
    }

    return 1;
}
