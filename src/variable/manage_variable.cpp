extern "C" {
    #include "variable.h"
}


typedef struct {
    char *name;
    CalculNode *value;
    char *access_priv;
} VariableBlock;


//  === 変数のデータを管理する ===
class ManageVariables {
public:
    void add_variable_data(char *name, char *access_priv, CalculNode *value) {
        //  変数の場所を代入
        int variable_pos = 0;
        //  もし変数が存在するなら、上書きする
        if ((variable_pos = this->search_variable(name, access_priv)) >= 0) {
            this->blocks[variable_pos].value = value;
            return;
        }
        //  変数の名前を代入
        this->blocks[this->blocks_length - 1].name = (char *)malloc((int)strlen(name));
        strcpy(this->blocks[this->blocks_length - 1].name, name);
        //  変数のアクセス特権を代入
        this->blocks[this->blocks_length - 1].access_priv = (char *)malloc((int)strlen(access_priv) + 1);
        strcpy(this->blocks[this->blocks_length - 1].access_priv, access_priv);
        //  変数の値を代入
        this->blocks[this->blocks_length - 1].value = value;
        this->blocks_length++;
        this->blocks = (VariableBlock *)realloc(
                this->blocks,
                sizeof(VariableBlock)
                * this->blocks_length);
    }

    CalculNode* get_variable_value(char *name, char *access_priv) {
        int variable_pos = 0;

        if ((variable_pos = this->search_variable(name, access_priv)) >= 0) {
            return this->blocks[variable_pos].value;
        }

        //err
        printf("this variable is not found -> %s\n", name);
        exit(1);
    }

    ManageVariables() {
        this->blocks = (VariableBlock *)malloc(sizeof(VariableBlock));
        this->blocks_length = 1;
    }

    ~ManageVariables() {
        for (int count = 0; this->blocks_length - 1 > count; count++) {
            free(this->blocks[count].name);
            free(this->blocks[count].access_priv);
        }

        free(this->blocks);
    }
private:
    int search_variable(char *name, char *access_priv) {
        //  変数が存在しないので、-1
        if (this->blocks_length == 1) {
            return -1;
        }

        for (int count = 0; count < this->blocks_length - 1; count++) {
            if (!strcmp(this->blocks[count].name, name) && !strcmp(this->blocks[count].access_priv, access_priv)) {
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
int add_variable_value(char *name, char *access_priv, CalculNode *value) {
    access_variable_obj()->add_variable_data(name, access_priv, value);
    return 1;
}

// === 変数の値をゲット ===
CalculNode* get_variable_value(char *name, char *access_priv) {
    return access_variable_obj()->get_variable_value(name, access_priv);
}
