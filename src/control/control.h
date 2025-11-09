#ifndef BANANA_SRC_CONTROL_CONTROL_H
#define BANANA_SRC_CONTROL_CONTROL_H


#include "../banana.h"


CalculNode* make_cond_expr_node(Token *token_list_ptr, int *pos);

CalculNode* make_for_expr_node(Token *token_list_ptr, int *pos);


#endif
