#ifndef BANANA_SRC_VARIABLE_VARIABLE_H
#define BANANA_SRC_VARIABLE_VARIABLE_H

#include "../banana.h"


//  === parse/parse.hで定義 ===
typedef struct CalculNode CalculNode;


// =============================
// === c言語からアクセスできる ===
// =============================

// === 変数を新しく作成 & 上書き ===
int add_variable_value(char *name, CalculNode *value);
// === 変数の値をゲット ===
CalculNode* get_variable_value(char *name);

#endif
