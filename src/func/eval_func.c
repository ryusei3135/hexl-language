#include "func.h"


int func_eval(CallFuncNode *call_data, ArgsNode *args) {
    if (!strcmp(call_data->lib_header, "[local]")) {
        FuncBlock *data = get_func_data(call_data->func_name);
        for (int count = 0; data->process_length > count; count++) {
            printf("%d value\n", calcul_eval(data->process[count].process_ptr));
        }
    } else {
        eval_lib_func(call_data->func_name, call_data->lib_header, args);
    }

    return 1;
}
