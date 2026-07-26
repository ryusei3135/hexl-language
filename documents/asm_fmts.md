# アセンブリ言語のフォーマットの設定ファイルについて

## asm.json
``` json
{
  "settings": [
     {
	"file": "x64.json",
	"name": "x64"
     },
     {
	"file": "gcc_x64.json",
	"name": "gcc_x64"
     }
  ],
  "default": 1,
  "entry": "_start"
}
```
1. settingsの中に、フォーマットファイルの一覧を書く
    - file
        - asm_fmts/..にファイルの名前を書く.jsonまで含める
    - name
        - inline アセンブラでどの入力で使うか
        - オプションで動指定するかの名前
2. default
    - デフォルトで使うフォーマッとの設定のindex
3. entry
    - ゆつりょくする際に、エントリーポイントを指定

## asm_fmts/
この中にアセンブリ言語のフォーマッとファイルを入れる
### 書き方
1. reg
    - レジスタを記述
    - すべて、配列として記述する
    1. db: 8bit
    2. dw: 16bit
    3. dd: 32bit
    4. dq: 64bit
    ```
    "reg": {
        "db": [
            "al", "cl", "dl", "bl"
        ],
        "dw": [..],
        "dd": [..],
        "dq": [..]
      },
    ```
2. args/fmt
    - OSごとに引数のレジスタを指定する
    linux: []
    win: []
3. section
    - セクションを定義する際のフォーマットを記述
4. fmt
5. op
6. func
