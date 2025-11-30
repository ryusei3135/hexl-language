extern "C" {
    #include "func.h"
}
#include <vector>


class ManageIndentDatas {
public:
    void new_indent(int indent_len) {
        this->indent_len.insert(this->indent_len.begin(), indent_len);
        this->manage_indent_data.insert(this->manage_indent_data.begin(), 0);
    }
    void update_now_status(int status) {
        this->manage_indent_data.front() = status;
    }
    void update_indent_len(int len) {
        this->indent_len.front() = len;
    }
    int get_last_indent_len() {
        if (this->indent_len.size() > 1) {
            return this->indent_len[1];
        } else {
            puts("[err]system err");
            puts("... file indent.cpp");
            puts("... objs ManageIndentDatas");
            puts("... func get_last_indent_len");
            exit(1);
        }
    }
    int get_now_status() {
        if (!this->manage_indent_data.empty()) {
            return this->manage_indent_data.front();
        }
        // err
        return -1;
    }
    int get_now_indent() {
        if (!this->indent_len.empty()) {
            return this->indent_len.front();
        }
        // err
        return -1;
    }
    void end_now_indent() {
        this->manage_indent_data.erase(this->manage_indent_data.begin());
        this->indent_len.erase(this->indent_len.begin());
    }
private:
    std::vector<int> indent_len = {0};
    std::vector<int> manage_indent_data = {0};
};


ManageIndentDatas* indent_datas() {
    static ManageIndentDatas data;
    return &data;
}

void assign_indent_value(int value, int process_num) {
    switch (process_num) {
        case 1:
            //  現在のインデントの長さを代入
            indent_datas()->new_indent(value);
            break;
        case 2:
            //  インデントの情報を代入
            indent_datas()->update_now_status(value);
            break;
        case 3:
            indent_datas()->end_now_indent();
            break;
        case 4:
            indent_datas()->update_indent_len(value);
        default:
            puts("[err]system err: indent.cpp");
            puts("... func assign_indent_value");
            break;
    }
}

int get_now_indent_len() {
    return indent_datas()->get_now_indent();
}

int get_now_indent_status() {
    return indent_datas()->get_now_status();
}

int get_last_indent_len() {
    return indent_datas()->get_last_indent_len();
}
