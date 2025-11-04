#include "func.h"


void func_eval(char *func_name) {
    FuncBlock *data = get_func_data(func_name);

    for (int count = 0; data->process_length >= count; count++) {
        printf("%d value\n", calcul_eval(data->process[count].process_ptr));
    }
}