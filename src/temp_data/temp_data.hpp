#ifndef BANANA_SRC_TEMP_DATA_TEMP_DATA_H
#define BANANA_SRC_TEMP_DATA_TEMP_DATA_H


class BlockTokens {
public:
    void add_block();
    void end_block();
    int check_reserve();
    void reserve_block();
    BlockTokens();
private:
    struct {
        int block_count;
        int appeared;
    } blocks;
};

BlockTokens block_token_data();


#endif
