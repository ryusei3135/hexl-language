extern "C" {
    #include "func.h"
}
#include <vector>

class ManageIndent {
public:
    void add_indent(int len) {
        this->indent_data.push_back(len);
    }

    int now_indent_len() {
        return this->indent_data.back();
    }

    void del_indent() {
        this->indent_data.pop_back();
    }
private:
    std::vector<int> indent_data;
};


//  スキップするインデントの長さを保持
int get_skip_indent_len(int next_skip_indent) {
    static int skip_indent = 0;
    //  -1 は、インデントの長さを取得したいとき
    if (next_skip_indent == -1) {
        return skip_indent;
    } else {
        skip_indent = next_skip_indent;
        return skip_indent;
    }
}


//  インデントの長さを管理
int manage_indent_len(int update_indent) {
    static ManageIndent indent;

    switch (update_indent) {
        case -1:
            return indent.now_indent_len();
        case -2:
            indent.del_indent();
            return 1;
        default:
            indent.add_indent(update_indent);
            return 1;
    }
}
