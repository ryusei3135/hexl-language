#include "control.h"


#define GetCondStatus -1
#define NextCondExpr 1
#define EndCondExpr 0


static int current_cond(int value) {
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

    if (token_list_ptr[*pos].type == TypeLbrace) {
        current_cond(NextCondExpr);
    } else {
        current_cond(EndCondExpr);
    }

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

CalculNode* make_if_else_expr(Token *token_list_ptr, int *pos) {
    if (current_cond(GetCondStatus)) {
        if (token_list_ptr[*pos].type == TypeRbrace) {
            // else文
            if (token_list_ptr[*pos + 2].type == TypeCondElse) {
                return make_else_expr(token_list_ptr, pos);
            }

            CalculNode *if_else_node = (CalculNode *)malloc(sizeof(CalculNode));
            (*pos)++;
            if_else_node->left = parse_operator(token_list_ptr, pos);
            if_else_node->type = OpIfElse;

            if (token_list_ptr[*pos].type == TypeLbrace) {
                current_cond(NextCondExpr);
            } else {
                current_cond(EndCondExpr);
            }
            return if_else_node;
        }
        puts("if else syntax err");
        exit(1);
    }
    exit(1);//err
}
