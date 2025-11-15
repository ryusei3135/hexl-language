#include "banana.h"


void load_file(char *file_name) {
    FILE *file = fopen(file_name, "r");

    char buffer[80];

    while (fgets(buffer, sizeof(buffer), file) != NULL) {
        buffer[strcspn(buffer, "\n") - 1] = '\0';
        Token *token_list_ptr = make_token_list_ptr(buffer);
        make_process_data(token_list_ptr);
        free_all_token_ptr(token_list_ptr);
    }
    CallFuncNode *call_node = (CallFuncNode *)malloc(sizeof(CallFuncNode));
    call_node->func_name = (char *)malloc(5);
    call_node->lib_header = (char *)malloc(9);
    strcpy(call_node->func_name, "main");
    strcpy(call_node->lib_header, "[local]");
    func_eval(call_node, NULL, "[start]");

    fclose(file);
    //  メモリを解放
    current_func_name("[free]");
}

int main(int argc, char *argv[]) {
    if (strstr(argv[1], ".bnn")) {
        load_file(argv[1]);
    }
    return 0;
}
