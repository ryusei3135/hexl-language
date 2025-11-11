#ifndef BANANA_SRC_CONTROL_CONTROL_H
#define BANANA_SRC_CONTROL_CONTROL_H


#include "../banana.h"


CalculNode* make_cond_expr_node(Token *token_list_ptr, int *pos);
CalculNode* make_else_expr(Token *token_list_ptr, int *pos);
CalculNode* make_if_else_expr(Token *token_list_ptr, int *pos);

CalculNode* make_loop_expr_node(Token *token_list_ptr, int *pos);


#endif
