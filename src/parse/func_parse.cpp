extern "C" {
    #include "parse.h"
}
#include "../temp_data/temp_data.hpp"


//   関数の名前と引数を設定
//   add_funcにデータを渡す
char* make_func_header(Token *token_list_ptr, int *pos) {
    static char err_ret_result[6] = "[err]";

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
                        if (token_list_ptr[*pos].type == TypeLbrace) {
                            block_token_data().add_block();
                            break;
                        }
                        (*pos)++;
                    }

                    block_token_data().reserve_block();
                    break;
                }
            }
            if (token_list_ptr[*pos].type == TypeSpace) {
                (*pos)++;
            }
        }

        return token_list_ptr[func_name_pos].token;
    }

    return err_ret_result;
}
