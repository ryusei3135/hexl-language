#include "parse.h"


OpType soring_operator_token_type(char *token_text) {
    //  === 関係演算子 ===
    if (!strcmp(token_text, "==")) {
        return TypeOpEpual;
    } else if (!strcmp(token_text, ">")) {
        return TypeOpBigger;
    } else if (!strcmp(token_text, "<")) {
        return TypeOpSmallerThen;
    } else if (!strcmp(token_text, ">=")) {
        return TypeOpHigher;
    } else if (!strcmp(token_text, "<=")) {
        return TypeOpBelow;
    } else if (!strcmp(token_text, "!=")) {
        return TypeOpIsNot;
    }
    //   ===  論理演算  ===
    if (!strcmp(token_text, "&&")) {
        return TypeOpAnd;
    } else if (!strcmp(token_text, "||")) {
        return TypeOpOr;
    }

    return -1;
}


void assign_process_for_func(CalculNode *process, int indent_len) {
    process->indent_len = indent_len;
    add_func_process(process);
}

void make_process_data(Token *token_list_ptr) {
  char *in_func_name;
    int pos = 0;
    int space_len;

    while (TokenEndCond(token_list_ptr[pos].type)) {
        if (token_list_ptr[pos].type == TypeSpace) {
            space_len = (int)strlen(token_list_ptr[pos].token);
        }

        if (token_list_ptr[pos].type == TypeNumber || token_list_ptr[pos].type == TypeNormal) {
            assign_process_for_func(parse_assign_var(token_list_ptr, &pos), space_len);
        } else if (token_list_ptr[pos].type == TypeFunc) {
            in_func_name = make_func_header(token_list_ptr, &pos);
            if (!strcmp(in_func_name, "[err]")) {
                exit(1);//err
            }
            current_func_name(in_func_name);
        } else if (token_list_ptr[pos].type == TypeImport) {
            // extern_lib/manage_lib.cppで定義
            import_lib(token_list_ptr, &pos);
        } else if (token_list_ptr[pos].type == TypeCondExpr) {
            assign_process_for_func(make_cond_expr_node(token_list_ptr, &pos), space_len);
        } else if (token_list_ptr[pos].type == TypeLoopExpr) {
            assign_process_for_func(make_loop_expr_node(token_list_ptr, &pos), space_len);
        } else if (token_list_ptr[pos].type == TypeRbrace) {
            //  "}"から始まる、物は大体"if else"文
            assign_process_for_func(make_if_else_expr(token_list_ptr, &pos), space_len);
        } else if (token_list_ptr[pos].type == TypeReturnExpr) {
            assign_process_for_func(make_ret_expr(token_list_ptr, &pos), space_len);//
        }

        if (token_list_ptr[pos].type == TypeSpace) {
            pos++;
        }
    }
}
