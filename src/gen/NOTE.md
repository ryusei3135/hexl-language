# 重要なAPI

## アセンブリ言語
- アセンブリ言語のテキストのレジスタのサイズを取得する方法
    - [場所] `src/gen/mng_fmt/`
    - [名前] get_reg_size

## 重要
### get_expr_ty
- `asm_emitter.rs`
- irの式をたどって自分の式の型を取得する関数