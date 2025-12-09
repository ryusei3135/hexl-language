extern "C" {
    #include "control.h"
}

#include "../temp_data/temp_data.hpp"


void assign_left_brace_token(TokenType type) {
    if (type == TypeLbrace) {
        current_cond(NextCondExpr);
        block_token_data().add_block();
    } else {
        //  "{"がないので、予約する
        block_token_data().reserve_block();
        current_cond(NextCondExpr);
    }
}

void end_block() {
    block_token_data().end_block();
}

int check_block() {
    return block_token_data().check_reserve();
}
