extern "C" {
    #include "func.h"
}


class ManageFuncs {
public:
    //  === 関数を新しく作成 ===
    void add_func(char *name) {
        //  関数の名前を代入
        this->blocks[this->block_length - 1].name \
                = (char *)malloc((int)strlen(name));
        strcpy(this->blocks[this->block_length - 1].name, name);

        // 関数の処理内容を格納する変数を初期化
        this->blocks[this->block_length - 1].process_length = 0;
        this->blocks[this->block_length - 1].process \
                = (ProcessList *)malloc(sizeof(ProcessList));

        //  関数の情報を入れる変数のメモリを拡張
        this->block_length++;
        this->blocks = (FuncBlock *)realloc(
                this->blocks,
                this->block_length
                * sizeof(FuncBlock));
    }
    //  === 関数に処理内容を追加 ===
    void add_func_process(CalculNode *process) {
        this->blocks[this->block_length - 2].process[
                this->blocks[this->block_length - 2].process_length
            ].process_ptr \
                = process;

        this->blocks[this->block_length - 2].process_length++;
        this->blocks[this->block_length - 2].process \
                = (ProcessList *)realloc(
                        this->blocks[this->block_length - 2].process,
                        sizeof(ProcessList)
                        * (this->blocks[this->block_length - 2].process_length + 1));
    }

    FuncBlock* get_func_data(char *func_name) {
        for (int count = 0; this->block_length - 2 >= count; count++) {
            if (!strcmp(this->blocks[count].name, func_name)) {
                return &this->blocks[count];
            }
        }

        exit(1);//err
    }

    ManageFuncs() {
        this->blocks = (FuncBlock *)malloc(sizeof(FuncBlock));
        this->block_length = 1;
    }

    ~ManageFuncs() {
        for (int count = 0; this->block_length - 1 > count; count++) {
            free(this->blocks[count].name);
            for (int process = 0; this->blocks[count].process_length - 1 > process; process++) {
                if (this->blocks[count].process[process].process_ptr) {
                    free_all_calcul_node(this->blocks[count].process[process].process_ptr);
                }
            }
            free(this->blocks[count].process);
        }
        free(this->blocks);
    }
private:
    FuncBlock *blocks;
    int block_length;
};


ManageFuncs* access_func_block() {
    static ManageFuncs func_block;
    return &func_block;
}

void add_func(char *name) {
    access_func_block()->add_func(name);
}

void add_func_process(CalculNode *process) {
    access_func_block()->add_func_process(process);
}

FuncBlock* get_func_data(char *func_name) {
    return access_func_block()->get_func_data(func_name);
}
