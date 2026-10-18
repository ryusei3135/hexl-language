/* 1. プログラムの全体構造 */
<translation-unit>        ::= <external-declaration> | <translation-unit> <external-declaration>
<external-declaration>   ::= <function-definition> | <declaration>
<function-definition>   ::= <declaration-specifiers>? <declarator> <declaration-list>? <compound-statement>

/* 2. 宣言（変数宣言など） */
<declaration>            ::= <declaration-specifiers> <init-declarator-list>? ";"
<declaration-specifiers> ::= <storage-class-specifier> <declaration-specifiers>?
                           | <type-specifier> <declaration-specifiers>?
                           | <type-qualifier> <declaration-specifiers>?
<type-specifier>         ::= "void" | "char" | "short" | "int" | "long" | "float" | "double" | "signed" | "unsigned" 

                           | <struct-or-union-specifier> | <enum-specifier> | <typedef-name>

/* 3. 文（ステートメント） */
<statement>              ::= <labeled-statement>
                           | <compound-statement>
                           | <expression-statement>
                           | <selection-statement>
                           | <iteration-statement>
                           | <jump-statement>

<compound-statement>     ::= "{" <declaration-list>? <statement-list>? "}"
<expression-statement>   ::= <expression>? ";"

/* 条件分岐とループ（if, whileなど） */
<selection-statement>    ::= "if" "(" <expression> ")" <statement>
                           | "if" "(" <expression> ")" <statement> "else" <statement>
                           | "switch" "(" <expression> ")" <statement>

<iteration-statement>    ::= "while" "(" <expression> ")" <statement>
                           | "do" <statement> "while" "(" <expression> ")" ";"
                           | "for" "(" <expression-statement> <expression-statement> <expression>? ")" <statement>

<jump-statement>         ::= "goto" <identifier> ";"
                           | "continue" ";"
                           | "break" ";"
                           | "return" <expression>? ";"

/* 4. 式（演算子の優先順位の定義） */
<expression>             ::= <assignment-expression> | <expression> "," <assignment-expression>
<assignment-expression>  ::= <conditional-expression>
                           | <unary-expression> <assignment-operator> <assignment-expression>

<conditional-expression> ::= <logical-or-expression>
                           | <logical-or-expression> "?" <expression> ":" <conditional-expression>

<logical-or-expression>  ::= <logical-and-expression> | <logical-or-expression> "||" <logical-and-expression>
<logical-and-expression> ::= <inclusive-or-expression> | <logical-and-expression> "&&" <inclusive-or-expression>

/* 代数演算（足し算・掛け算など） */
<additive-expression>    ::= <multiplicative-expression>
                           | <additive-expression> "+" <multiplicative-expression>
                           | <additive-expression> "-" <multiplicative-expression>

<multiplicative-expression> ::= <cast-expression>
                               | <multiplicative-expression> "*" <cast-expression>
                               | <multiplicative-expression> "/" <cast-expression>
                               | <multiplicative-expression> "%" <cast-expression>

/* 基本要素（変数名や数値） */
<primary-expression>     ::= <identifier>
                           | <constant>
                           | <string-literal>
                           | "(" <expression> ")"
