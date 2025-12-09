#include "temp_data.hpp"

#include "stdio.h"

#include <vector>

void BlockTokens::add_block() {
    this->blocks.block_count++;
    this->blocks.appeared = 0;
}
//  インデントを一つ下げる
void BlockTokens::end_block() {
    this->blocks.block_count--;
}

int BlockTokens::check_reserve() {
    return this->blocks.appeared;
}
//  次のトークンが、インデントを一つ上げる
//  ことを代入
void BlockTokens::reserve_block() {
    if (!this->blocks.appeared) {
        this->blocks.appeared = 1;
    } else {
      puts("[syntax err]");
    }
}
BlockTokens::BlockTokens() {
    this->blocks.block_count = 0;
    this->blocks.appeared = 0;
}

BlockTokens block_token_data() {
    static BlockTokens block;
    return block;
}
