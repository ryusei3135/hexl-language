extern "C" {
    #include "load.h"
}

// === 文法が正しいか、調べる ===
int judge_this_expr_is_import(Token *token_list_ptr, int *pos) {
    if (token_list_ptr[*pos].type == TypeImport) {
        while (token_list_ptr[*pos].type != TypeNormal) {
            (*pos)++;

            if (token_list_ptr[*pos].type == TypeEnd) {
                return 0;
            }
        }
    }

    return 1;
}

// === 読み込む関数やファイルの場所などを処理 ===
int setting_load_lib(Token *token_list_ptr, int *pos, char *dir_stack) {
    if (token_list_ptr[*pos].type == TypeLibSpace) {
        dir_stack = (char *)realloc(
                dir_stack, 
                (int)strlen(dir_stack) + 1);
        strcat(dir_stack, "/");
    } else if (token_list_ptr[*pos].type == TypeComma) {
        return 1;
    }

    return 0;
}


class ManageLibs {
public:
    void import_lib(Token *token_list_ptr, int *pos) {
        char *dir_stack = (char *)malloc(1);

        if (!judge_this_expr_is_import(token_list_ptr, pos)) {
            exit(1);//err
        }

        while (token_list_ptr[*pos].type != TypeEnd) {
            if (token_list_ptr[*pos].type == TypeNormal) {
                this->setting_lib_dir(token_list_ptr, pos, dir_stack);
            }
            if (setting_load_lib(token_list_ptr, pos, dir_stack)) {
                // === ライブラリをロード ===
                (*pos)++;
                load_lib_func(dir_stack, token_list_ptr[*pos].token, this->last_lib_dir);
            }

            (*pos)++;
        }

        printf("%s\n", dir_stack);
        free(dir_stack);
    }

    ManageLibs() {
        this->now_lib_type = LibNull;
    }

    ~ManageLibs() {
        if (this->last_lib_dir) {
            free(this->last_lib_dir);
        }
    }
private:
    // === ライブラリの名前の処理をする ===
    void setting_lib_dir(Token *token_list_ptr, int *pos, char *dir_stack) {
        // === 一番最初の名前が、"std"なら、標準ライブラリなので、ディレクトリを指定する ===
        if (!strcmp(token_list_ptr[*pos].token, "std") && !(*dir_stack)) {
            token_list_ptr[*pos].token = (char *)realloc(token_list_ptr[*pos].token, 9);
            strcpy(token_list_ptr[*pos].token, "build/lib");
            this->now_lib_type = LibStd;
        } else {
            dir_stack = (char *)realloc(
                    dir_stack, 
                    (int)strlen(dir_stack)
                    + (int)strlen(token_list_ptr[*pos].token));
        }

        this->setting_last_lib_name(token_list_ptr[*pos].token);
        strcat(dir_stack, token_list_ptr[*pos].token);
    }

    // === 前回指定された、ライブラリの名前を代入 ===
    void setting_last_lib_name(char *name) {
        if (this->last_lib_dir) {
            free(last_lib_dir);
        }
        this->last_lib_dir = (char *)malloc((int)strlen(name));
        strcpy(this->last_lib_dir, name);
    }

    typedef enum {
        LibNull,
        LibNormal,
        LibStd,
    } LibType;

    LibType now_lib_type;
    char *last_lib_dir;
};


ManageLibs* access_manage_libs() {
    static ManageLibs libs;
    return &libs;
}

void import_lib(Token *token_list_ptr, int *pos) {
    access_manage_libs()->import_lib(token_list_ptr, pos);
}