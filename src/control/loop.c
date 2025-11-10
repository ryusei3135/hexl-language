#include "control.h"


CalculNode* make_loop_expr_node(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeLoopExpr) {
        (*pos)+=2;
    } else {
        exit(1);//err
    }

    CalculNode *loop_node = (CalculNode *)malloc(sizeof(CalculNode));

    loop_node->left = parse_operator(token_list_ptr, pos);
    loop_node->type = OpLoop;

    return loop_node;
}
