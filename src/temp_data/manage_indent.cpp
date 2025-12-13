#include "temp_data.hpp"

#include "stdio.h"

#include <vector>

//  インデントを一つ上げる
void BlockTokens::add_block() {
    this->blocks.block_count++;
    this->blocks.appeared = 0;
}
//  インデントを一つ下げる
void BlockTokens::end_block() {
    this->blocks.block_count--;
}
//  "{"が前回のトークンになくこのトークンが来ること
//  が予約されているか確認
int BlockTokens::check_reserve() {
    return this->blocks.appeared;
}

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
