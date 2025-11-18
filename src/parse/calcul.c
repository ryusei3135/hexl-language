#include "parse.h"


static CalculNode *make_num_node(char *value, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));

    //  ノードに値を代入し初期化
    node->value = (char *)malloc(strlen(value));
    strcpy(node->value, value);
    node->left = NULL;
    node->right = NULL;
    //  ノードのタイプを代入
    node->type = Num;

    (*pos)++;
    return node;
}

static CalculNode *make_op_node(OpType type, CalculNode *left, CalculNode *right) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    node->value = NULL;
    node->left = left;
    node->right = right;
    node->type = type;
    return node;
}


static CalculNode *parse_expr(Token *token_list_ptr, int *pos);

static CalculNode *parse_factor(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeNumber) {
        return make_num_node(token_list_ptr[*pos].token, pos);
    } else if (token_list_ptr[*pos].type == TypeNormal) {
        CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
        node->value = (char *)malloc((int)strlen(token_list_ptr[*pos].token));
        strcpy(node->value, token_list_ptr[*pos].token);
        (*pos)++;
        node->type = CallVar;
        node->left = NULL;
        node->right = NULL;
        return node;
    }

    if (token_list_ptr[*pos].type == TypeLparen) {
        (*pos)++;
        CalculNode *node = parse_expr(token_list_ptr, pos);

        //  エラー
        if (token_list_ptr[*pos].type != TypeRparen) {
            puts("[err paren]");
            exit(1);
        }

        (*pos)++;
        return node;
    }

    if (token_list_ptr[*pos].type == TypeSpace) {
        (*pos)++;
        return parse_factor(token_list_ptr, pos);
    }
    puts("err");
    exit(1);
}


static CalculNode *parse_term(Token *token_list_ptr, int *pos) {
    CalculNode *node = parse_factor(token_list_ptr, pos);

    while (TokenEndCond(token_list_ptr[*pos].type)) {
        if (token_list_ptr[*pos].type == TypeOpMul) {
            (*pos)++;
            node = make_op_node(Mul, node, parse_factor(token_list_ptr, pos));
        } else if (token_list_ptr[*pos].type == TypeOpDiv) {
            (*pos)++;
            node = make_op_node(Div, node, parse_factor(token_list_ptr, pos));
        } else if (token_list_ptr[*pos].type == TypeSpace) {
            (*pos)++;
        } else {
            break;
        }
    }

    return node;
}

static CalculNode *parse_expr(Token *token_list_ptr, int *pos) {
    CalculNode *node = parse_term(token_list_ptr, pos);

    while (TokenEndCond(token_list_ptr[*pos].type)) {
        if (token_list_ptr[*pos].type == TypeOpAdd) {
            (*pos)++;
            node = make_op_node(Add, node, parse_term(token_list_ptr, pos));
        } else if (token_list_ptr[*pos].type == TypeOpSub) {
            (*pos)++;
            node = make_op_node(Sub, node, parse_term(token_list_ptr, pos));
        } else if (token_list_ptr[*pos].type == TypeSpace) {
            (*pos)++;
        } else {
            break;
        }
    }

    return node;
}

//  === 関係演算子や論理演算 ===
CalculNode *parse_operator(Token *token_list_ptr, int *pos) {
    CalculNode *node = parse_expr(token_list_ptr, pos);

    while (TokenEndCond(token_list_ptr[*pos].type)) {
        OpType type = soring_operator_token_type(token_list_ptr[*pos].token);

        if (type != -1) {
            (*pos)++;
            node = make_op_node(type, node, parse_expr(token_list_ptr, pos));
        } else if (token_list_ptr[*pos].type == TypeSpace) {
            (*pos)++;
        } else {
            break;
        }
    }

    return node;
}


//  ===  変数を作成 ===
CalculNode *parse_assign_var(Token *token_list_ptr, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    int variable_node_status = 0;
    int var_name_pos = -1;

    while (TokenEndCond(token_list_ptr[*pos].type)) {
        if (token_list_ptr[*pos].type == TypeNormal && !variable_node_status) {
            if (token_list_ptr[(*pos) + 1].type == TypeLparen || token_list_ptr[(*pos) + 1].type == TypeComma) {
                // parse/statement.cで定義
                return make_call_func_node(token_list_ptr, pos);
            }
            var_name_pos = (*pos);
            (*pos)++;
        }

        // もし、変数に代入する式なら、実行
        if (token_list_ptr[*pos].type == TypeOpAssign && var_name_pos >= 0) {
            (*pos)++;
            node->value = (char *)malloc((int)strlen(token_list_ptr[var_name_pos].token));
            strcpy(node->value, token_list_ptr[var_name_pos].token);
            node->type = AssignVar;
            node->left = parse_operator(token_list_ptr, pos);
        }

        if (token_list_ptr[*pos].type == TypeSpace) {
            (*pos)++;
        }
    }

    return node;
}

// 数列を文字列に変換
static char* change_number_for_string(int target) {
    char *str;
    int len = sprintf(str, "%d", target);
    char *number_text = (char *)malloc(len + 1);
    sprintf(number_text, "%d", target);
    return number_text;
}

//  変数に価を代入する前に、代入する変数の価を代入
static CalculNode* make_assign_calcul_node(CalculNode *left) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    node->value = change_number_for_string(calcul_eval(left));
    node->type = Num;
    return node;
}

int calcul_eval(CalculNode* n) {
    switch (n->type) {
        case Num: return atoi(n->value);
        case Add: return calcul_eval(n->left) + calcul_eval(n->right);
        case Sub: return calcul_eval(n->left) - calcul_eval(n->right);
        case Mul: return calcul_eval(n->left) * calcul_eval(n->right);
        case Div: return calcul_eval(n->left) / calcul_eval(n->right);
        case AssignVar: return add_variable_value(n->value, current_func_name("[null]"), make_assign_calcul_node(n->left));
        case CallVar: return calcul_eval(get_variable_value(n->value, current_func_name("[null]")));
        case TypeOpEpual: return calcul_eval(n->left) == calcul_eval(n->right);
        case TypeOpBigger: return calcul_eval(n->left) > calcul_eval(n->right);
        case TypeOpSmallerThen: return calcul_eval(n->left) < calcul_eval(n->right);
        case TypeOpHigher: return calcul_eval(n->left) >= calcul_eval(n->right);
        case TypeOpBelow: return calcul_eval(n->left) <= calcul_eval(n->right);
        case TypeOpIsNot: return calcul_eval(n->left) != calcul_eval(n->right);
        case TypeOpAnd: return calcul_eval(n->left) && calcul_eval(n->right);
        case TypeOpOr: return calcul_eval(n->left) ||  calcul_eval(n->right);
        case CallFunc: return func_eval(n->call_data, n->args, current_func_name("[null]"));
        case OpIf: return calcul_eval(n->left);
        case OpIfElse: return calcul_eval(n->left);
        case OpElse: return 1;
        case OpLoop: return calcul_eval(n->left);
        case OpRet: return calcul_eval(n->left);
        default: printf("%d <- err type\n", n->type);
    }

    return 0;
}

void free_all_calcul_node(CalculNode *n) {
    if (n->type == CallFunc) {
        free(n->call_data->func_name);
        free(n->call_data->lib_header);

        for (int count = 0; n->args[0].length > count; count++) {
            if (count != 0) {
                if (n->args[count].name) {
                    free(n->args[count].name);
                } else if (n->args[count].value) {
                    free_all_calcul_node(n->args[count].value);
                }
            }
        }
        free(n->args);
        return;
    }

    if (n->value) {
        free(n->value);
    }

    if (n->left) {
        free_all_calcul_node(n->left);
        free(n->left);
    } else if (n->right) {
        free_all_calcul_node(n->right);
        free(n->right);
    }
}
