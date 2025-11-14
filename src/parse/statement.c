#include "parse.h"


//  関数の引数を設定
void make_args_setting_node(ArgsNode **args, Token *token_list_ptr, int *pos) {
    int arg_count = 0;

    if (token_list_ptr[*pos].type == TypeLparen) {
        (*pos)++;
        while (token_list_ptr[*pos].type != TypeRparen) {
            if (token_list_ptr[*pos].type == TypeNormal) {
                (*args)[arg_count].arg_value = (char *)malloc((int)strlen(token_list_ptr[*pos].token) + 1);
                strcpy((*args)[arg_count].arg_value, token_list_ptr[*pos].token);
                arg_count++;
            }

            if (token_list_ptr[*pos].type == TypePeriod) {
                ArgsNode *tmp = (ArgsNode *)realloc(*args, sizeof(ArgsNode) * (arg_count + 1));
                //  メモリの確保に失敗
                if (tmp == NULL) {
                    puts("[realloc err]: func: make_args_setting_node");
                    puts("func args ralloc failed");
                    exit(1);
                }
                *args = tmp;
                // 新しく確保したメモリを初期化
                memset(&(*args)[arg_count], 0, sizeof(ArgsNode));
            }

            (*pos)++;
        }
    }
}
//   関数の名前と引数を設定
//   add_funcにデータを渡す
char* make_func_header(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeFunc) {
        (*pos)++;
        int func_name_pos = 0;

        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeNormal) {
                if (func_name_pos) {
                    //
                }
                func_name_pos = (*pos);
                (*pos)++;
                if (token_list_ptr[*pos].type == TypeLparen) {
                    make_args_setting_node(add_func(token_list_ptr[func_name_pos].token), token_list_ptr, pos);

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

        return token_list_ptr[func_name_pos].token;
    }

    return "[err]";
}

//  関数の引数を渡すノードを作成
ArgsNode* make_args_node(Token *token_list_ptr, int *pos) {
    ArgsNode *node = (ArgsNode *)malloc(sizeof(ArgsNode));
    int start_paren = 0;
    int arg_count = 0;

    while (token_list_ptr[*pos].type != TypeEnd) {
        if (token_list_ptr[*pos].type == TypeLparen && !start_paren) {
            start_paren = 1;
            (*pos)++;
            continue;
        }

        if (start_paren) {
            if (token_list_ptr[*pos].type == TypeRparen) {
                (*pos)++;
                break;
            } else {
                arg_count++;

                if (token_list_ptr[*pos].type == TypeString) {
                    char *string = make_string_token(token_list_ptr, pos);

                    node->arg_value = (char *)malloc((int)strlen(string));
                    strcpy(node->arg_value, string);
                    node->op_type = Value;
                } else {
                    if (token_list_ptr[*pos].type == TypeNormal) {
                        node->arg_value = (char *)malloc((int)strlen(token_list_ptr[*pos].token));
                        strcpy(node->arg_value, token_list_ptr[*pos].token);
                        node->op_type = CallVar;
                    } else {
                        node->arg_value = (char *)malloc((int)strlen(token_list_ptr[*pos].token));
                        strcpy(node->arg_value, token_list_ptr[*pos].token);
                        node->op_type = Value;
                    }
                }
            }
            (*pos)++;
        }
    }

    //  引数が何もなかったときに実行
    if (!arg_count) {
        node->arg_value = (char *)malloc(7);
        strcpy(node->arg_value, "[null]");
    }

    return node;
}

CalculNode* make_call_func_node(Token *token_list_ptr, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    node->type = CallFunc;

    if (token_list_ptr[*pos].type == TypeNormal) {
        node->call_data = (CallFuncNode *)malloc(sizeof(CallFuncNode));

        if (token_list_ptr[(*pos) + 1].type == TypeComma && token_list_ptr[(*pos) + 2].type == TypeNormal) {
            //  関数の場所を指定
            node->call_data->lib_header = (char *)malloc(sizeof(token_list_ptr[*pos].token));
            strcpy(node->call_data->lib_header, token_list_ptr[*pos].token);
            // コンマを飛ばす
            (*pos)+=2;
            node->call_data->func_name = (char *)malloc(sizeof(token_list_ptr[*pos].token));
            strcpy(node->call_data->func_name, token_list_ptr[*pos].token);
        } else {
            //  ローカル関数であることを代入
            node->call_data->lib_header = (char *)malloc(9);
            strcpy(node->call_data->lib_header, "[local]");
            // 関数の名前を代入
            node->call_data->func_name \
                = (char *)malloc((int)strlen(token_list_ptr[*pos].token));
            strcpy(node->call_data->func_name, token_list_ptr[*pos].token);
        }

        (*pos)++;
        node->args = make_args_node(token_list_ptr, pos);
    }

    return node;
}
