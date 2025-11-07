#include "control.h"


void make_cond_expr(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeCondExpr) {
        (*pos)++;
    }

    while (token_list_ptr[*pos].type != TypeEnd) {
        CalculNode *expr = parse_operator(token_list_ptr, pos);
        printf("%d if value\n", calcul_eval(expr));
        free_all_calcul_node(expr);
    }
}
