# アセンブリ言語のフォーマットの設定ファイルについて

## asm.yaml
``` yaml
settings:
  - file: x64.yaml
    name: x64
  - file: gcc_x64.yaml
    name: gcc_x64
default: 1
entry: _start
```
1. settingsの中に、フォーマットファイルの一覧を書く
    - file
        - asm_fmts/..にファイルの名前を書く.yamlまで含める
    - name
        - inline アセンブラでどの入力で使うか
        - オプションで動指定するかの名前
2. default
    - デフォルトで使うフォーマッとの設定のindex
3. entry
    - ゆつりょくする際に、エントリーポイントを指定

## asm_fmts/
この中にアセンブリ言語のフォーマッとファイルを入れる(拡張子は`.yaml`)
### 書き方
1. reg
    - レジスタを記述
    - すべて、配列として記述する
    1. db: 8bit
    2. dw: 16bit
    3. dd: 32bit
    4. dq: 64bit
    ```yaml
    reg:
      db:
        - al
        - cl
        - dl
        - bl
      dw: [..]
      dd: [..]
      dq: [..]
    ```
2. args/fmt
    - OSごとに引数のレジスタを指定する
    ```yaml
    args:
      fmt:
        linux: [4, 3, 2, 1, 5, 6]
        win: [1, 2, 5, 6]
    ```
3. section
    - セクションを定義する際のフォーマットを記述
    ```yaml
    section: ".{name}\n"
    ```
4. fmt
    - 値やメモリ参照などをオペランドとして文字列化する際のフォーマットを記述する
    - `{}`や`{name}`のようなプレースホルダーを、実際の値で置換して使う

    | キー | 説明 | プレースホルダー |
    | --- | --- | --- |
    | reg | レジスタ参照のフォーマット | `{}`: レジスタ名(`reg`セクションの値) |
    | num | 数値リテラルのフォーマット | `{}`: 数値 |
    | static_var | 静的領域の変数の参照フォーマット(`%rip`相対など) | `{name}`: ラベル名 |
    | string | 文字列リテラルをデータセクションへ配置するフォーマット | `{name}`: ラベル名、`{}`: 文字列の中身 |
    | global | シンボルを公開する(`.global`)フォーマット | `{name}`: 公開するシンボル名 |
    | ref_stack | スタック上の値を`%rbp`からのオフセットで参照するフォーマット | `{src}`: 基準となるレジスタ(通常`%rbp`)、`{size}`: オフセット(バイト数) |
    | get_ptr | スタック上に置かれた値へのポインタを、変数へ代入する際に使う専用フォーマット | `{size}`: `%rbp`からのオフセット(バイト数)。基準レジスタは常に`%rbp`固定のため`{src}`は無い |
    | frame | 関数の先頭で、スタックフレームを構築するフォーマット(`push %rbp` / `mov %rsp, %rbp`) | `{space}`: インデント用の空白 |
    | frame_end | 関数の末尾で、スタックフレームを解放するフォーマット(`leave`) | `{space}`: インデント用の空白 |
    | data.head | ローカル変数用にスタック領域を確保する、関数先頭のフォーマット | `{space}`: インデント用の空白、`{size}`: 確保するバイト数(8バイト境界にアライメント済み) |
    | data.fmt | スタック上の構造体メンバーなどへ値を書き込むフォーマット | `{space}`: インデント用の空白、`{dst}`: 書き込む値、`{size}`: `%rbp`からのオフセット |
    | op_size.db/dw/dd/dq | ニーモニックへ付けるサイズ接尾辞(`movl`の`l`など) | 接尾辞の文字列そのもの(プレースホルダーなし) |

    ```yaml
    fmt:
      reg: "%{}"
      num: "${}"
      static_var: "{name}(%rip)"
      string: "{name}: .ascii \"{}\"\n"
      global: ".global {name}\n"
      data:
        head: "{space}push %rbp\n{space}mov %rsp, %rbp\n{space}sub ${size}, %rsp\n"
        fmt: "{space}mov {dst}, -{size}(%rbp)\n"
      op_size:
        db: b
        dw: w
        dd: l
        dq: q
      ref_stack: "-{size}({src})"
      get_ptr: "-{size}(%rbp)"
      frame: "{space}push %rbp\n{space}mov %rsp, %rbp\n"
      frame_end: "{space}leave\n"
    ```

5. op
    - 各命令(オペコード)ごとのテンプレートを記述する
    - キーは命令の種類を表す名前(以下)で、値は`len`と`template`を持つオブジェクト

    | キー | 対応する演算/命令 |
    | --- | --- |
    | push | スタックへの`push` |
    | pop | スタックからの`pop` |
    | add | 加算(`+`) |
    | sub | 減算(`-`) |
    | mul | 乗算(`*`) |
    | div | 除算(`/`) |
    | mov | 代入・値の移動 |
    | cmp_l | `<`比較とジャンプ |
    | cmp_g | `>`比較とジャンプ |
    | cmp_e | `==`比較とジャンプ |
    | cmp_ne | `!=`比較とジャンプ |
    | ret | 関数からの復帰(`ret`) |
    | address | アドレスの取得(`lea`) |

    - `len`
        - このテンプレートが必要とするオペランドの数(現状は生成側の情報として保持しているのみで、テンプレート中のプレースホルダーの数と対応する)
    - `template`
        - 実際に出力するアセンブリの行(複数行の場合は`\n`で連結する)
        - 以下のプレースホルダーが使える
            - `{space}`: インデント用の空白
            - `{dst}`: 書き込み先のオペランド
            - `{src1}` / `{src2}`: 読み込み元のオペランド
            - `{label}`: `cmp_*`系命令のジャンプ先ラベル

    ```yaml
    op:
      push:
        len: 1
        template: "{space}push {dst}\n"
      add:
        len: 2
        template: "{space}mov {src1}, {dst}\n{space}add {src2}, {dst}\n"
      cmp_l:
        len: 2
        template: "{space}mov {src1}, {dst}\n{space}cmp {src2}, {dst}\n{space}jl {label}\n"
      address:
        len: 1
        template: "{space}lea {src1}, {dst}\n"
    ```

    - 注意点
        - `cmp_l` / `cmp_g` / `cmp_e` / `cmp_ne` はテンプレートを引くためのキーであり、実際に出力される比較命令自体のニーモニックは共通して`cmp`になる(ニーモニックへサイズの接尾辞を付ける際は、`cmp_l`などのキーではなく`cmp`という文字列を対象にする必要がある)
        - `address`もテンプレートを引くためのキーで、実際のニーモニックは`lea`になる。`lea`は常にポインタサイズ(64bit)のレジスタ/接尾辞を使う

6. func
    - 関数の定義や呼び出しに関するフォーマットを記述する

    | キー | 説明 | プレースホルダー |
    | --- | --- | --- |
    | extern_def | 外部で定義された関数を宣言するフォーマット(`.extern`) | `{name}`: 関数名 |
    | ret | 関数の戻り値が置かれるレジスタの番号(`reg`の`dd`配列のindex) | プレースホルダーなし(数値をそのまま指定する) |
    | call | 関数を呼び出すフォーマット(`call`) | `{name}`: 呼び出す関数名 |

    ```yaml
    func:
      extern_def: ".extern {name}\n"
      ret: 0
      call: "call {name}\n"
    ```
