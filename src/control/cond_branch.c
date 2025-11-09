#include "control.h"


CalculNode* make_cond_expr_node(Token *token_list_ptr, int *pos) {

    if (token_list_ptr[*pos].type == TypeCondExpr) {
        (*pos)+=2;
    } else {
        exit(1);//err
    }

    CalculNode *if_node = (CalculNode *)malloc(sizeof(CalculNode));

    if_node->left = parse_operator(token_list_ptr, pos);
    if_node->type = OpIf;
    
    return if_node;
}
