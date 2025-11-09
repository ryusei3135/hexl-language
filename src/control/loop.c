#include "control.h"


CalculNode* make_for_expr_node(Token *token_list_ptr, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    node->type = OpFor;
    int for_expr = 0;

    while (token_list_ptr[*pos].type != TypeEnd) {
        if (token_list_ptr[*pos].type == TypeLoopFor) {
            for_expr = 1;
        }

        if (token_list_ptr[*pos].type == TypeSpace) {
            (*pos)++;
        } else {
            node->left = parse_operator(token_list_ptr, pos);
        }
    }

    return node;
}
