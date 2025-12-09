#include "control.h"


int current_cond(int value) {
    static int result = EndCondExpr;
    if (value != -1) {
        result = value;
    }
    return result;
}


CalculNode* make_cond_expr_node(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeCondExpr) {
        (*pos)+=2;
    } else {
        exit(1);//err
    }

    CalculNode *if_node = (CalculNode *)malloc(sizeof(CalculNode));

    if_node->left = parse_operator(token_list_ptr, pos);
    if_node->type = OpIf;

    assign_left_brace_token(token_list_ptr[*pos].type);

    return if_node;
}

CalculNode* make_else_expr(Token *token_list_ptr, int *pos) {
    if (current_cond(GetCondStatus) == 1) {
        if (token_list_ptr[*pos].type == TypeRbrace) {
            CalculNode *else_node = (CalculNode *)malloc(sizeof(CalculNode));

            else_node->left = (CalculNode *)malloc(sizeof(CalculNode));
            else_node->left->value = (char *)malloc(2);
            strcpy(else_node->left->value, "1");
            else_node->type = OpElse;

            while (token_list_ptr[*pos].type != TypeEnd) {
                (*pos)++;
            }

            return else_node;
        }
        puts("else syntax err");
        exit(1);
    }

    current_cond(EndCondExpr);

    exit(1);
}

void make_if_else_expr(Token *token_list_ptr, int *pos, int space_len) {
    if (current_cond(GetCondStatus)) {
        if (token_list_ptr[*pos].type == TypeRbrace) {
            end_block();
            // else文
            if (token_list_ptr[*pos + 2].type == TypeCondElse) {
                assign_process_for_func(make_else_expr(token_list_ptr, pos), space_len);
                return;
            } else if (token_list_ptr[*pos + 1].type == TypeEnd) {
                (*pos)++;
                return;
            }

            CalculNode *if_else_node = (CalculNode *)malloc(sizeof(CalculNode));
            (*pos)++;
            if_else_node->left = parse_operator(token_list_ptr, pos);
            if_else_node->type = OpIfElse;

            assign_left_brace_token(token_list_ptr[*pos].type);
            assign_process_for_func(if_else_node, space_len);
            return;
        }
        puts("if else syntax err");
        exit(1);
    }
    exit(1);//err
}
