# parse/ の NOTE

`lex/preproc.rs` が返すトークン列を読み、`node.zip` のノード(`Group1Node` / `Group2Info` / `Expr`)を作る。

```
トークン --(parser.rs)--> CAST(cast.rs) --(lower.rs)--> node
```

* `parser.rs` … C の構文解析。typedef 名・列挙定数・構造体の定義を読みながら進める。
* `lower.rs`  … CAST から node への変換。node に無い構文はここで書き換えるか、エラーにする。
* `prelude.rs` … node / lexer など外部への依存を 1 か所に集めたファイル。
* `error.rs`  … `ParseError`。`kind` が `Syntax`(Cとして誤り)か `Unsupported`(node で表せない)かで区別できる。
* `tests.rs`  … 36 件のテスト(`cargo test`)。

入口は `parse::parse_c(tokens) -> Result<ParseOutput, ParseError>`。
`ParseOutput.nodes` が `Vec<Group1Node>`、`warnings` が読み飛ばし・近似の一覧。

---

## 0. 前提(node.zip から読み取れなかったので仮定したもの)

node.zip は `crate::compiler::{models::Body, parse::VarMutAttr, err::ErrKind}` を使っているが、zip に入っていない。次のように仮定した。違うなら `prelude.rs` と `lower.rs` の該当箇所だけ直せばよい。

| 仮定 | 理由 |
|---|---|
| `Body` は `Vec<Group2Info>` と同じ(`Vec<Group2Info>` をそのまま渡せる) | `FuncDefine.body` が `Vec<Group2Info>`、`MatchArm.body` / `Loop.body` が `Body` |
| `VarMutAttr` に `Invar`(不変) と `Var`(可変) がある。この 2 つしか使っていない | `DefineVar` のコメントとテストで確認できた名前 |
| `crate::lexer::{Spanned, Token}` で lexer を参照できる | `preproc.rs` が既にこの書き方 |
| パーサは `crate::compiler` の子孫モジュールに置く | `node/mod.rs` の再エクスポートが `pub(in crate::compiler)` |

**`lex/mod.rs` は `mod lexer; mod preproc;`(非公開) のままだが、`preproc.rs` は `crate::lexer` を参照している。** 今回は lex には手を入れていない。組み込むときは lexer / preproc が `crate::lexer` / `crate::preproc` から見えるようにすること。

動作確認は、`lexer.rs` `preproc.rs` `node/` `parse/` を `#[path]` で束ねた別クレートで行った(`models` / `VarMutAttr` / `ErrKind` は上の仮定どおりの仮実装)。

---

## 1. node に無いので、エラー(`Unsupported`)にしているもの

node に表す方法が無いか、意味を保ったまま書き換えられないもの。**node 側にノードが増えれば対応できる。**

### グローバルな宣言
* **グローバル変数**(`static` も含む)。`Group1Node` に変数のノードが無い。`extern` 宣言と関数プロトタイプは、無害なので警告つきで読み飛ばす。
* 関数ポインタ、関数型(`TyNode` に対応するものが無い)。仮引数・戻り値・メンバー・変数のどれでも。
* 可変長引数の関数定義(`int f(int n, ...)`)。
* ビットフィールド、C11 の無名メンバー(無名の struct/union メンバー)。
* **enum の明示的な値**(`enum { A = 1, B = 2 }`)。`VariantField` が値を持てず、タグは定義順(0,1,2…)に固定されているため。定義順と一致する指定(`A = 0, B, C = 2`)は通す。

### 式
* **`sizeof`**(型・式とも)。node に `sizeof` が無く、型の大きさはこの段階で決まらない。
* **関数ポインタ経由の呼び出し**(`f(x)` の `f` が名前でないもの)。`CallInfo.name` は名前のみ。
* **`->` の連鎖・名前以外への `->`**(`p->next->val`、`f()->x`、`a[i]->x`、`(*pp)->x`)。`PtrMember.name` が変数名しか持てない。`p->a.b`、`a[i].m`、`(*p).m` は対応している。
* 変数名・配列要素・`*p` 以外への `.`(関数の戻り値のメンバー、`a[i][j].m` など)。
* 値を使っている**後置 `++` / `--`**(`a[i++] = x`、`y = i++`)。node の `Inc` / `Dec` は前置と後置を区別できない。文として単独で使う場合(`i++;`、`for` の更新式)は対応している。
* 複合リテラル `(T){...}`、GNU の文式 `({ ... })`、`_Generic`、`_Alignof`。
* 式の途中のカンマ演算子(`x = (a, b)`)。文の中と `for` の初期化・更新式のカンマは分解して対応している。
* 副作用のある左辺への複合代入(`a[i++] += 1`)。左辺が 2 回評価されてしまうため。

### 文
* **`goto` とラベル**。
* **switch の途中の `break`**(case 本体の末尾以外。`if (x) break;` など)。`Match` に switch を抜ける手段が無い。
* switch の外側に置かれた case/default ラベル(Duff's device など)、`{}` で囲まれていない switch 本体、GNU の `case a ... b:`。

### その他
* 配列の添字指示子 `{ [2] = 5 }`、入れ子の指示子 `{ .a.b = 1 }`、GNU の範囲指示子。
* 可変長配列、大きさを決められない配列(`int a[];` で初期化子なし)。
* ワイド文字・UTF 文字(`L'a'` `u"x"` など)。
* K&R 形式(旧式)の関数定義。
* `_Thread_local` `_Atomic` `_Complex` `_Alignas`。
* インラインアセンブリ `asm(...)`(`CompleSyntax` の使い方が分からないので未対応。`__asm__` は宣言の後ろにあるものだけ読み飛ばす)。

---

## 2. node に無いが、書き換えて対応しているもの

どれも node の既存ノードで書ける。**node 側の意味づけが私の想定と違うと壊れる**ので、確認してほしい(3 章)。

| C | node |
|---|---|
| `if / else if / else` | `Match { pattern: None, arms: [条件 → 本体, ...], arm_else }`(`else if` は平らな arm に並べる) |
| `switch` | `Match { pattern: Some(式), arms, arm_else }`。`default` が `arm_else`。末尾の `break` は外す。**fallthrough は、落ちた先の本体を後ろにコピーして再現する**(`case 1: case 2:` は同じ本体を持つ arm が 2 つ) |
| `while (c) B` | `Loop { pattern: None, body: [c の否定なら break, B...] }` |
| `do B while (c)` | `Loop { pattern: None, body: [B..., c の否定なら break] }` |
| `for (i; c; s) B` | `i; Loop { pattern: None, body: [c の否定なら break, B..., s] }` |
| `continue` | ループ内で `for` なら更新式 `s`、`do-while` なら判定をコピーしてから `Continue` |
| `while (1)` / `for (;;)` | 判定の `break` を付けない |
| `c ? a : b` | `Match { pattern: None, arms: [c → [a]], arm_else: Some([b]) }`(**`Match` が値を返す前提**) |
| `!a` | `a == 0`(`a` が比較なら反転: `<` → `>=`、`==` → `!=` など) |
| `-a` | `0 - a`(整数・実数リテラルなら `Number("-5")` / `Float("-1.5")`) |
| `+a` | `a` |
| `~a` | `a ^ -1` |
| `a && b` / `a \|\| b` | 左右を `!= 0` で 0/1 にしたうえで、**右辺に副作用・メモリ参照・割り算が無ければ** `AndCalcul` / `OrCalcul`。あれば短絡評価を保つため `Match`(`a ? b : 0` / `a ? 1 : b`)。 |
| `a += b`(複合代入) | `a = a + b`(左辺に副作用が無いときだけ) |
| `if (x)` の `x` が比較でない | `x != 0` |
| 型指定子 | `unsigned long long` のように **空白区切りの正規名** を `TyNode::Ty` に入れる(`long int` → `long`、`_Bool` → `bool`、`unsigned` → `unsigned int`) |
| `typedef` | その場で展開する(node に typedef が無い)。`typedef struct {..} Name;` は `Name` を構造体の名前にする |
| 無名の struct / union / enum | `__anon_struct_N` などの名前を付ける |
| 関数内で定義した struct / union / enum | 関数の前へ移して、トップレベルの定義にする |
| `struct S` / `union U` / `enum E` | それぞれ `StructDefine` / `VariantDefine`(union は全フィールド `Unsafe`、enum は型なしフィールド) |
| 文字定数 `'a'` | `Number("97")` |
| 文字列リテラル | エスケープを展開した `Str`。隣り合う文字列は連結する |
| 整数リテラル | 10 進にした `Number`(`0x10` → `"16"`)。**サフィックス(`u` `l` `ll`)は捨てる** |
| `{ }` で囲んだ複文 | 親の本体へ平らに展開する(node に「ブロック」が無い)。`for (int i..)` を 2 つ並べるなど、同じ本体の中で名前が重なったら **後ろの変数を `i__1` のように改名**し、その後の参照も追従させる |
| 初期化子の無い宣言 | スカラ・ポインタ・enum は `Number("0")`、配列は `Array([])`、構造体は空の `InitStruct`、共用体は空の `InitVariant`(`DefineVar.value` が必須のため。C では不定値なので 0 にしても矛盾しない) |
| 配列の初期化子 | `{1,2,3}` → `Array`。**宣言の大きさより短くても 0 で埋めない** |
| `static` な局所変数 | `TyNode::Static { name, len }`(スカラは `len: 1`、配列は要素数) |
| 配列の局所変数 | `TyNode::Stack { name: 要素型, len }`(多次元は入れ子) |
| 関数の `static` | `FuncDefine.public = false`。それ以外は `true` |
| 仮引数 | 配列型はポインタに読み替える。`const` 付きは `Invar`、そのほかは `Var` |
| 変数の `const` | `VarMutAttr::Invar`(`const int x` / `char *const p`)。`const char *p` は変数が `Var` でポインタが `is_const: true` |

---

## 3. node の意味が分からず、推測で決めたもの(要確認)

| 箇所 | 今の対応 | 違っていたら |
|---|---|---|
| `Match.pattern: None` | 条件分岐(各 arm の `pattern` は真偽を表す式、いわゆる `cond`) | `if` / 三項演算子 / 短絡評価 / ループの判定すべてに影響 |
| `Match` は値を返す式 | 三項演算子と `&&` / `\|\|` の短絡で、arm の最後の式が値になる前提 | 値を返さないなら三項と短絡評価のところ(`lower_expr_ex` の `Cond` と `lower_binary`)を直す |
| `Loop.pattern: None` | 無限ループ。条件は本体の先頭で `break` して表す | `pattern` に条件を渡せるなら `while` を `Some(cond)` にする方が素直 |
| `Loop` / `Match` の本体の `break` / `continue` | 一番内側の `Loop` を指す | |
| `return;` | `Return(Number("0"))`(`Return` が式を必須としているため)。void 関数の**末尾**の `return;` は消す | `lower.rs` の `void_value()` だけ変えればよい |
| `TyNode::Pointer.is_const` | **指す先が const**(`const char *`)なら `true`。ポインタ自身の const は変数の `var_attr` に出す | |
| `Member { scope, target }` | `a.b.c` → `scope: ["a","b"], target: Var("c")` | |
| `PtrMember` | `p->m` → `name: "p", index: None`。`a[i].m` は `index: Some(i)` で、`a` が配列でもポインタでも `PtrMember` | 配列要素のメンバーは別の表現かもしれない |
| `PtrMember.target` が入れ子 | `p->a.b` → `target: Member { scope: ["a"], target: Var("b") }` | |
| `RefArray.name` | 添字を取られる式の根になっている変数名(`a[i][j]` の外側も `"a"`)。根が変数でなければ空文字列 | |
| `RefArray.else_idx` | 常に `None`(C は範囲外の添字を扱わない) | `ir/checker/access_mem.rs` によると、`const` でも即値でもない添字には `else_idx` が必要。**そのままだと検査で落ちる可能性が高い** |
| `AssignVar.name` | 代入先の根になっている変数名 | |
| `Inc` / `Dec` | 値の意味は不明のまま、文として使うときと前置の `++i` だけ使う | |
| 整数の型 | 型の名前は C のまま(`"int"` `"unsigned long"` ...)。node 側の型名が違うなら `parser.rs` の `base_name` で一括変更 | |
| `Number` の負数 | `Number("-5")` と文字列に `-` を含める | |
| ポインタ演算 | `p + 1` は `Add` のまま。C の「要素の大きさ倍」を node が行うかは不明 | |
| `!(a < b)` を `a >= b` に反転 | 浮動小数点の NaN では結果が変わる | |

---

## 4. 情報が落ちる・読み飛ばすもの

警告(`ParseOutput.warnings`)に出るのは、プロトタイプ・`extern` 宣言・`_Static_assert`・暗黙の `int`。下の項目は警告なしで落ちる。

* `volatile` `restrict` `inline` `register` `auto` `_Noreturn`、`__attribute__((..))`、`__extension__`。
* 整数リテラルのサフィックス(`10UL` → `10`)。
* 関数プロトタイプ、`extern` 変数宣言(型情報は捨てるので、呼び出しの引数・戻り値は検査されない)。
* `#include` / `#line` は preproc が処理済みなので `Group1Node::Include` / `Line` は作らない。
* `Group2Node::CompleSyntax` は作らない。

---

## 5. メモ

* typedef 名と変数名の衝突(ローカル変数が typedef 名を隠す)、ローカル変数が列挙定数を隠すケースは扱っていない。
* 構造体の初期化子は、その構造体の定義が先に見えている必要がある(`InitStruct` がフィールド名で持つため)。波括弧を省略した入れ子の初期化子(brace elision)は未対応で、余分な要素がエラーになるか位置がずれる。
* `node/data_node.rs` の doc コメント(`variant A { ... }`)が rustdoc にコードブロックと解釈されるため、`cargo test` の doctest が失敗する。`cargo test --lib` なら通る(node 側の既存の問題で、今回は触っていない)。
