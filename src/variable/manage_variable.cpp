extern "C" {
    #include "variable.h"
}


typedef struct {
    char *name;
    CalculNode *value;
} VariableBlock;


//  === 変数のデータを管理する ===
class ManageVariables {
public:
    void add_variable_data(char *name, CalculNode *value) {
        //  変数の場所を代入
        int variable_pos = 0;
        //  もし変数が存在するなら、上書きする
        if ((variable_pos = this->search_variable(name)) >= 0) {
            this->blocks[variable_pos].value = value;
            return;
        }

        this->blocks[this->blocks_length - 1].name = (char *)malloc((int)strlen(name));
        strcpy(this->blocks[this->blocks_length - 1].name, name);
        this->blocks[this->blocks_length - 1].value = value;
        this->blocks_length++;
        this->blocks = (VariableBlock *)realloc(
                this->blocks,
                sizeof(VariableBlock)
                * this->blocks_length);
    }

    CalculNode* get_variable_value(char *name) {
        int variable_pos = 0;

        if ((variable_pos = this->search_variable(name)) >= 0) {
            return this->blocks[variable_pos].value;
        }

        //err
        exit(1);
    }

    ManageVariables() {
        this->blocks = (VariableBlock *)malloc(sizeof(VariableBlock));
        this->blocks_length = 1;
    }

    ~ManageVariables() {
        for (int count = 0; this->blocks_length - 1 > count; count++) {
            free(this->blocks[count].name);
        }

        free(this->blocks);
    }
private:
    int search_variable(char *name) {
        //  変数が存在しないので、-1
        if (this->blocks_length == 1) {
            return -1;
        }

        for (int count = 0; count < this->blocks_length - 1; count++) {
            if (!strcmp(this->blocks[count].name, name)) {
                return count;
            }
        }
        //  変数が存在しない場合 "-1"を返す
        return -1;
    }
    // === 変数のデータの塊 ===
    VariableBlock *blocks;
    int blocks_length;
};


ManageVariables* access_variable_obj() {
    static ManageVariables data;
    return &data;
}

// =============================
// === c言語からアクセスできる ===
// =============================

// === 変数を新しく作成 & 上書き ===
void add_variable_value(char *name, CalculNode *value) {
    access_variable_obj()->add_variable_data(name, value);
}

// === 変数の値をゲット ===
CalculNode* get_variable_value(char *name) {
    return access_variable_obj()->get_variable_value(name);
}
