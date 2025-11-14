extern "C" {
    #include "func.h"
}

//  変数などのアクセス特権を代入する際に
//  今どの関数の変数か調べるための構造体
typedef struct {
    char *name;
} current_func;

//  関数を管理するメモリの処理
class FuncsMemoryManage {
public:
    //  関数のデータを代入する add_funcの名前を構造体に代入する
    //  メモリを確保
    static void add_func_name_ptr(char *name, char **target) {
        char *name_ptr = (char *)malloc((int)strlen(name));
        if (name_ptr == NULL) {
            puts("[   type   ]: [  filename  ]: [   func name   ]");
            puts("[malloc err]: manage_func.cpp: ManageFuncs::add_func");
            puts("    [err]:assign func name");
            puts("func args ralloc failed");
            exit(1);
        }
        (*target) = name_ptr;
    }

    static void realloc_func_block_ptr(FuncBlock **target, int len) {
        FuncBlock *block_ptr = (FuncBlock *)realloc(
                (*target),
                (len + 1) * sizeof(FuncBlock));
        //  メモリが確保されているか確認
        if (block_ptr == NULL) {
            puts("[   type   ]: [  filename  ]: [   func name   ]");
            puts("[realloc err]: manage_func.cpp: ManageFuncs::add_func");
            puts("    [err]:realloc func data memory");
            puts("func args ralloc failed");
        }
        (*target) =  block_ptr;
    }

    static void realloc_add_process_mem(ProcessList **target, int len) {
        ProcessList *ptr \
                = (ProcessList *)realloc(
                        (*target),
                        sizeof(ProcessList)
                        * (len + 1));
        //   メモリが確保されているか確認
        if (ptr == NULL) {
            puts("[   type   ]: [  filename  ]: [   func name   ]");
            puts("[realloc err]: manage_func.cpp: ManageFuncs::add_func_process");
            puts("func args ralloc failed");
            exit(1);
        }

        (*target) = ptr;
    }
};

//  関数のデータを管理
class ManageFuncs {
public:
    //  === 関数を新しく作成 ===
    ArgsNode** add_func(char *name) {
        //  関数の名前を代入
        FuncsMemoryManage::add_func_name_ptr(name, &this->blocks[this->block_length].name);

        strcpy(this->blocks[this->block_length].name, name);
        //  関数の引数を設定するメモリを初期化
        this->blocks[this->block_length].args = (ArgsNode *)malloc(sizeof(ArgsNode));

        // 関数の処理内容を格納する変数を初期化
        this->blocks[this->block_length].process_length = 0;
        this->blocks[this->block_length].process \
                = (ProcessList *)malloc(sizeof(ProcessList));

        this->block_length++;
        //  関数の情報を入れる変数のメモリを拡張
        FuncsMemoryManage::realloc_func_block_ptr(&this->blocks, this->block_length);
        return &this->blocks[this->block_length - 1].args;
    }
    //  === 関数に処理内容を追加 ===
    void add_func_process(CalculNode *process) {
        this->blocks[this->block_length - 1].process[
                this->blocks[this->block_length - 1].process_length
            ].process_ptr \
                = process;

        this->blocks[this->block_length - 1].process_length++;
        FuncsMemoryManage::realloc_add_process_mem(
              &this->blocks[this->block_length - 1].process,
              this->blocks[this->block_length - 1].process_length);
    }

    FuncBlock* get_func_data(char *func_name) {
        for (int count = 0; this->block_length - 1 >= count; count++) {
            if (!strcmp(this->blocks[count].name, func_name)) {
                return &this->blocks[count];
            }
        }

        printf("[err] this func is not found -> %s\n", func_name);
        exit(1);//err
    }

    ManageFuncs() {
        this->blocks = (FuncBlock *)malloc(sizeof(FuncBlock));
        this->block_length = 0;
    }

    ~ManageFuncs() {
        for (int count = 0; this->block_length - 1 > count; count++) {
            free(this->blocks[count].name);
            for (int process = 0; this->blocks[count].process_length > process; process++) {
                if (this->blocks[count].process[process].process_ptr) {
                    free_all_calcul_node(this->blocks[count].process[process].process_ptr);
                }
            }
            //  引数の設定を代入するメモリを解放
            for (int arg_count = 0; this->blocks[count].args[arg_count].arg_value; arg_count++) {
                free(this->blocks[count].args[arg_count].arg_value);
            }
            free(this->blocks[count].args);

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

ArgsNode** add_func(char *name) {
    return access_func_block()->add_func(name);
}

void add_func_process(CalculNode *process) {
    access_func_block()->add_func_process(process);
}

FuncBlock* get_func_data(char *func_name) {
    return access_func_block()->get_func_data(func_name);
}


//  処理を関数のデータに代入中に変数のアクセス特権
//  を代入するときに関数の名前をゲットする
char* current_func_name(char *func_name) {
    static char setting_func_log[6] = "[set]";
    static current_func *current = (current_func *)malloc(sizeof(current_func));
    //  構造体が初期化されていないときに実行
    if (!current->name) {
        current->name = (char *)malloc(1);
    }

    if (!strcmp(func_name, "[null]")) {
        return current->name;
    } else {
        free(current->name);
        char *ptr = (char *)malloc((int)strlen(func_name) + 1);
        if (ptr == NULL) {
            puts("[   type   ]: [  filename  ]: [   func name   ]");
            puts("[malloc err]: manage_func.cpp: current_func_name");
            puts("func args malloc failed");
            exit(1);
        }
        current->name = ptr;
        strcpy(current->name, func_name);

        return setting_func_log;
    }
}
