#include "parse.h"



ArgsNode* make_args(Token *token_list_ptr, int *pos, int call_args) {
    ArgsNode *args = (ArgsNode *)malloc(sizeof(ArgsNode) * 2);
    int arg_count = 0;
    //  引数の長さを設定
    args[arg_count].length = 0;
    arg_count = 1;

    if (token_list_ptr[*pos].type == TypeLparen) {
        (*pos)++;
        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeRparen) {
                break;
            }

            if (call_args) {
                //  関数の引数を設定
                if (token_list_ptr[*pos].type == TypeNormal) {
                    //  引数の長さをインクリメント
                    args[0].length++;
                    //  引数の名前を代入
                    args[arg_count].name = (char *)malloc((int)strlen(token_list_ptr[*pos].token) + 1);
                    strcpy(args[arg_count].name, token_list_ptr[*pos].token);
                }
            } else {
                //  呼び出す元の引数の価
                if (token_list_ptr[*pos].type == TypeString) {
                    char *string = make_string_token(token_list_ptr, pos);
                    args[arg_count].value = (CalculNode *)malloc(sizeof(CalculNode));
                    args[arg_count].value->value = (char *)malloc((int)strlen(string) + 1);
                    strcpy(args[arg_count].value->value, string);
                    args[arg_count].value->type = String;
                    args[0].length++;
                } else if (token_list_ptr[*pos].type == TypeNumber) {
                    args[arg_count].value = (CalculNode *)malloc(sizeof(CalculNode));
                    args[arg_count].value->value = (char *)malloc((int)strlen(token_list_ptr[*pos].token) + 1);
                    strcpy(args[arg_count].value->value, token_list_ptr[*pos].token);
                    args[arg_count].value->type = Num;
                    args[0].length++;
                } else {
                    //  引数に変数の結果を代入
                    if (token_list_ptr[*pos].type == TypeNormal) {
                        args[arg_count].value = (CalculNode *)malloc(sizeof(CalculNode));
                        args[arg_count].value->value = (char *)malloc((int)strlen(token_list_ptr[*pos].token) + 1);
                        strcpy(args[arg_count].value->value, token_list_ptr[*pos].token);
                        args[arg_count].value->type = CallVar;
                        args[0].length++;
                    }
                }
            }

            if (token_list_ptr[*pos].type == TypePeriod) {
                ArgsNode *tmp = (ArgsNode *)realloc(args, sizeof(ArgsNode) * (arg_count + 2));
                //  メモリの確保に失敗
                if (tmp == NULL) {
                    puts("[realloc err]: func: make_args_setting_node");
                    puts("func args ralloc failed");
                    exit(1);
                }
                args = tmp;
                // 新しく確保したメモリを初期化
                arg_count++;
                memset(&args[arg_count], 0, sizeof(ArgsNode));
            }

            (*pos)++;
        }
    }

    return args;
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
                    //  第一引数は、add_funcの戻り値，ArgsNodeのポインタ
                    //  このポインタに引数のデータを代入する
                    *add_func(token_list_ptr[func_name_pos].token) = make_args(token_list_ptr, pos, 1);

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

//  関数を呼び出すときに呼び出すデータを作成
CalculNode* make_call_func_node(Token *token_list_ptr, int *pos) {
    CalculNode *node = (CalculNode *)malloc(sizeof(CalculNode));
    node->type = CallFunc;

    if (token_list_ptr[*pos].type == TypeNormal) {
        node->call_data = (CallFuncNode *)malloc(sizeof(CallFuncNode));

        // 呼び出す関数の名前を設定
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

        //  引数を設定
        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeLparen) {
                node->args = make_args(token_list_ptr, pos, 0);
                continue;
            }

            (*pos)++;
        }
    }

    return node;
}
