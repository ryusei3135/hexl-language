#ifndef BANANA_SRC_CONTROL_CONTROL_H
#define BANANA_SRC_CONTROL_CONTROL_H


#include "../banana.h"

#define GetCondStatus -1
#define NextCondExpr 1
#define EndCondExpr 0

int current_cond(int value);

//  control.cpp
void assign_left_brace_token(TokenType type);
void end_block();
int check_block();
//  cond_branch.c
CalculNode* make_cond_expr_node(Token *token_list_ptr, int *pos);
CalculNode* make_else_expr(Token *token_list_ptr, int *pos);
void make_if_else_expr(Token *token_list_ptr, int *pos, int space_len);
//  loop.c
CalculNode* make_loop_expr_node(Token *token_list_ptr, int *pos);


#endif
