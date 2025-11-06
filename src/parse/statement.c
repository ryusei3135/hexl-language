#include "parse.h"


void make_func_header(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeFunc) {
        (*pos)++;
        int func_name = 0;

        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeNormal) {
                if (func_name) {
                    //
                }
                func_name = (*pos);
                (*pos)++;
                if (token_list_ptr[*pos].type == TypeLparen) {
                    add_func(token_list_ptr[func_name].token);

                    while (token_list_ptr[*pos].type != TypeEnd) {
                        (*pos)++;
                    }
                    break;
                }
            }

            if (token_list_ptr[*pos].type == TypeSpace) {
                (*pos)++;
            }
        }
    }
}

CalculNode* make_call_func_node(Token *token_list_ptr, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));

    if (token_list_ptr[*pos].type == TypeNormal) {
        node->call_data = (CallFuncNode *)malloc(sizeof(CallFuncNode));
        int start_paren = 0;

        if (token_list_ptr[(*pos) + 1].type == TypeComma && token_list_ptr[(*pos) + 2].type == TypeNormal) {
            //  関数の場所を指定
            node->call_data->lib_header = (char *)malloc(sizeof(token_list_ptr[*pos].token));
            strcpy(node->call_data->lib_header, token_list_ptr[*pos].token);
            // コンマを飛ばす
            (*pos)+=2;
            node->call_data->func_name = (char *)malloc(sizeof(token_list_ptr[*pos].token));
            strcpy(node->call_data->func_name, token_list_ptr[*pos].token);
        } else {
            node->call_data->func_name \
                = (char *)malloc((int)strlen(token_list_ptr[*pos].token));
            strcpy(node->call_data->func_name, token_list_ptr[*pos].token);
        }

        (*pos)++;
        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeLparen && !start_paren) {
                start_paren = 1;
                (*pos)++;
                continue;
            }

            if (start_paren) {
                if (token_list_ptr[*pos].type == TypeRparen) {
                    node->type = CallFunc;
                    (*pos)++;
                    break;
                } else {
                    if (token_list_ptr[*pos].type == TypeString) {
                        char *string = make_string_token(token_list_ptr, pos);

                        ArgsNode *arg = (ArgsNode *)malloc(sizeof(ArgsNode));
                        arg->arg_value = (char *)malloc((int)strlen(string));
                        strcpy(arg->arg_value, string);
                        node->args = arg;
                        continue;
                    }
                }
                (*pos)++;
            } else {
                free(node->value);
                free(node);
                //err
                exit(1);
            }
        }
    }

    return node;
}
