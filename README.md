# banana
=== clang 推奨 ===

# banana alpha 0.3
外部関数を読み込むことができるようになった
標準ライブラリを作成

int print(char *text)
# banana alpha 0.3.1
外部ライブラリの読み込む文法が完成

import std::io::io.print
このように読み込める
呼び出すときは、
io.print()
# banana alpha 0.3.2
ビルドされたときのディレクトリを変更
build -> bin
# banana alpha 0.3.3
標準ライブラリ io
の print関数の修正
引数を入れることで出力する文字列を変更可能

io.print("Hello World!!") -> Hello World!!
# banana alpha 0.3.4
インデントを追加、インポートしたライブラリ
の名前が出力されるバグを修正
# banana alpha 0.3.5
処理が終了した後に、すべてのメモリを
解放するときに、出るエラーを修正

# banana alpha 0.4.0
if 分を追加
# banana alpha 0.4.1
変数の価を代入する処理を変更
変数を呼び出す処理を変更
# banana alpha 0.5.0
loop分を追加
変数を、例 a = a + 1
のようにすると、処理されなくなるバグを修正
# banana alpha 0.5.1
変数 c が処理終了後に、呼び出す処理を削除
