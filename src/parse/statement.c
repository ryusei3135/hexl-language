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