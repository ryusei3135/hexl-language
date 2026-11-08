//! 関数のラベル(シンボル名)を生成するモジュール
//!
//! ラベルは、Rustの`v0`マングリング(`-C symbol-mangling-version=v0`)と同じ規則で作る。
//!
//! ```text
//! a::b::foo           -> _RNvNtC4hexl1a1b3foo
//! a::b::foo<int, u8>  -> _RINvNtC4hexl1a1b3foolhE
//! a::Foo::bar         -> _RNvMNtC4hexl1aNtNtC4hexl1a3Foo3bar   (構造体`Foo`のメソッド)
//! ```
//!
//! - `_R`: v0のシンボルの先頭
//! - `C4hexl`: クレートのルート。このコンパイラの出力は、すべて`hexl`クレートとして扱う
//! - `Nt`: モジュール(`::`で区切った、最後より前の要素)。`Nt`が階層の数だけ入れ子になる
//! - `Nv`: 値の名前空間(関数)。最後の要素
//! - `M<モジュールのパス><型のパス>`: 構造体などの型のメソッド(Rustの`impl Foo { fn bar }`と同じ)。
//!   型のパスは`Nt<モジュールのパス><型名>`
//! - `<長さ><名前>`: 識別子。名前が数字か`_`で始まるときは、長さの後に`_`を入れる
//! - `I<パス><型引数>E`: ジェネリクス。型引数は、組み込みの型なら1文字、
//!   それ以外は`hexl`クレートの型のパス(`NtC4hexl<長さ><名前>`)
//!
//! クレートの識別子(`Cs<hash>_`)と、ジェネリクスを実体化したクレート名の
//! 末尾(`B6_`)は省略する(v0の仕様で、どちらも省略できる)。
//! `std::`で始まる標準ライブラリの関数も、特別扱いせず同じ規則で作る。

/// 出力するすべての関数が属するクレートの名前
const CRATE_NAME: &str = "hexl";

/// v0の識別子(`<長さ>[_]<名前>`)にする。
/// 英字・数字・`_`以外は無視する(アセンブラのラベルに使える文字だけを残す)
fn encode_ident(part: &str) -> String {
    let mut ident: String = part
        .chars()
        .filter(|&ch| matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_'))
        .collect();
    if ident.is_empty() {
        ident.push('a');
    }
    // 名前が数字か`_`で始まるときは、長さとの境目が分からなくなるので`_`で区切る
    let sep = if ident.starts_with(|c: char| c.is_ascii_digit() || c == '_') {
        "_"
    } else {
        ""
    };
    format!("{}{}{}", ident.len(), sep, ident)
}

/// クレートのルートのパス(`C4hexl`)
fn crate_root() -> String {
    format!("C{}", encode_ident(CRATE_NAME))
}

/// 型引数1つ分。組み込みの型は1文字、それ以外は`hexl`クレートの型のパスにする。
///
/// 組み込みの型は、Rustのv0の`<basic-type>`と同じ文字を使う
/// (`i32`→`l`、`u8`→`h`、`u64`→`y`、`bool`→`b`など)
fn encode_type_arg(ty: &str) -> String {
    // `TyNode::Ty("int")`の`{:?}`(`Ty("int")`)は、中の型名だけを使う
    let ty = ty
        .trim()
        .strip_prefix("Ty(\"")
        .and_then(|rest| rest.strip_suffix("\")"))
        .unwrap_or(ty.trim());
    let basic = match ty {
        "i8" => "a",
        "bool" => "b",
        "char" => "c",
        "f64" => "d",
        "str" => "e",
        "f32" | "float" => "f",
        "u8" | "byte" => "h",
        "isize" => "i",
        "usize" => "j",
        "i32" | "int" => "l",
        "u32" => "m",
        "i128" => "n",
        "u128" => "o",
        "i16" => "s",
        "u16" => "t",
        "i64" => "x",
        "u64" => "y",
        _ => "",
    };
    if basic.is_empty() {
        format!("Nt{}{}", crate_root(), encode_ident(ty))
    } else {
        basic.to_string()
    }
}

/// モジュールのパス(`C4hexl`に、階層ごとの`Nt`を入れ子にしたもの)
fn encode_module_path(modules: &[&str]) -> String {
    modules.iter().fold(crate_root(), |path, module| {
        format!("Nt{}{}", path, encode_ident(module))
    })
}

/// 関数のパス
///
/// - 型を持たない関数: `Nv<モジュールのパス><関数名>`
/// - 型(構造体など)のメソッド: `NvM<モジュールのパス><型のパス><関数名>`
///   (型のパスは`Nt<モジュールのパス><型名>`)
///
/// `self_ty`は、メソッドが属する型の名前。`::`を含むとき(`a::Foo`)は、
/// 最後の要素が型名、それより前がその型のモジュールになる。含まないときは、
/// 型のモジュールは関数名(`base`)の階層と同じになる。
/// `base`が`a::Foo::bar`のように型名を含んでいても、`a::bar`のように含んでいなくても同じラベルになる
fn encode_fn_path(base: &str, self_ty: Option<&str>) -> String {
    let mut parts: Vec<&str> = base.split("::").filter(|part| !part.is_empty()).collect();
    // 最後の要素が関数名、それより前の階層がモジュール
    let fn_part = parts.pop().unwrap_or_default();

    let ty_parts: Vec<&str> = self_ty
        .map(|ty| ty.split("::").filter(|part| !part.is_empty()).collect())
        .unwrap_or_default();
    let Some((ty_name, ty_modules)) = ty_parts.split_last() else {
        return format!("Nv{}{}", encode_module_path(&parts), encode_ident(fn_part));
    };

    // 型の階層が書かれていなければ、関数名の階層を使う(末尾が型名なら取り除く)
    let modules: &[&str] = if !ty_modules.is_empty() {
        ty_modules
    } else {
        if parts.last() == Some(ty_name) {
            parts.pop();
        }
        &parts
    };
    let module_path = encode_module_path(modules);
    format!(
        "NvM{}Nt{}{}{}",
        module_path,
        module_path,
        encode_ident(ty_name),
        encode_ident(fn_part)
    )
}

/// 通常の関数ラベルを生成する。
///
/// `self_ty`は、構造体などのメソッドのときの型名(`FnDefInfo::self_ty`)。
///
/// - `app::util::add` -> `_RNvNtC4hexl3app4util3add`
/// - `app::Foo::bar`(`self_ty`は`Foo`) -> `_RNvMNtC4hexl3appNtNtC4hexl3app3Foo3bar`
pub(in crate::compiler::asm_gen) fn emit_fn_name_id(base: &str, self_ty: Option<&str>) -> String {
    if base == "_start" {
        base.to_string()
    } else {
        format!("_R{}", encode_fn_path(base, self_ty))
    }
}

/// ジェネリクス関数のラベルを生成する。
///
/// `app::map<int, u8>` -> `_RINvC4hexl...3maplhE`(`I<関数のパス><型引数...>E`)。
/// 型引数が無いときは、ジェネリクスではないので[`emit_fn_name_id`]と同じラベルになる。
pub(in crate::compiler::asm_gen) fn emit_generic_fn_name_id(
    base: &str,
    self_ty: Option<&str>,
    generic_args: &[String],
) -> String {
    if generic_args.is_empty() {
        return emit_fn_name_id(base, self_ty);
    }
    let args: String = generic_args.iter().map(|a| encode_type_arg(a)).collect();
    format!("_RI{}{}E", encode_fn_path(base, self_ty), args)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn start_is_unchanged() {
        assert_eq!(emit_fn_name_id("_start", None), "_start");
    }

    /// 実際の`rustc -C symbol-mangling-version=v0`の出力
    /// (`hexl`クレートの`a::b::foo`、`_under`)から、クレートの識別子
    /// (`s20rihVQmJrB_`)と、末尾の`B6_`を除いたものと同じになる
    #[test]
    fn same_as_rustc_v0() {
        assert_eq!(emit_fn_name_id("a::b::foo", None), "_RNvNtNtC4hexl1a1b3foo");
        assert_eq!(emit_fn_name_id("root", None), "_RNvC4hexl4root");
        assert_eq!(emit_fn_name_id("_under", None), "_RNvC4hexl6__under");
    }

    #[test]
    fn generic_labels_same_as_rustc_v0() {
        let args = |v: &[&str]| v.iter().map(|s| s.to_string()).collect::<Vec<_>>();
        // `a::b::gen::<i32, u8>` / `a::b::gen::<u64, bool>`
        assert_eq!(
            emit_generic_fn_name_id("a::b::gen", None, &args(&["i32", "u8"])),
            "_RINvNtNtC4hexl1a1b3genlhE"
        );
        assert_eq!(
            emit_generic_fn_name_id("a::b::gen", None, &args(&["u64", "bool"])),
            "_RINvNtNtC4hexl1a1b3genybE"
        );
        // hexlの型名も同じ文字になる
        assert_eq!(
            emit_generic_fn_name_id("a::b::gen", None, &args(&["int", "byte"])),
            "_RINvNtNtC4hexl1a1b3genlhE"
        );
    }

    #[test]
    fn debug_form_of_ty_node_is_unwrapped() {
        let args = vec!["Ty(\"int\")".to_string(), "Ty(\"Foo\")".to_string()];
        assert_eq!(
            emit_generic_fn_name_id("f", None, &args),
            "_RINvC4hexl1flNtC4hexl3FooE"
        );
    }

    #[test]
    fn user_types_are_paths_in_the_crate() {
        let args = vec!["Vec<u8>".to_string()];
        assert_eq!(
            emit_generic_fn_name_id("f", None, &args),
            "_RINvC4hexl1fNtC4hexl5Vecu8E"
        );
    }

    #[test]
    fn std_is_not_special() {
        assert_eq!(
            emit_fn_name_id("std::io::print", None),
            "_RNvNtNtC4hexl3std2io5print"
        );
    }

    #[test]
    fn no_type_args_is_a_plain_label() {
        assert_eq!(
            emit_generic_fn_name_id("app::f", None, &[]),
            emit_fn_name_id("app::f", None)
        );
    }

    /// rustcの`v0`の出力(`mod a { struct Foo; impl Foo { fn bar, fn newf, fn g<T> } }`)から、
    /// クレートの識別子と、パスの後方参照(`NtB2_3Foo`)を展開したものと同じになる
    #[test]
    fn methods_same_as_rustc_v0() {
        let expect = "_RNvMNtC4hexl1aNtNtC4hexl1a3Foo3bar";
        // `name`が型名を含んでいても、含んでいなくても同じ
        assert_eq!(emit_fn_name_id("a::Foo::bar", Some("Foo")), expect);
        assert_eq!(emit_fn_name_id("a::bar", Some("Foo")), expect);
        assert_eq!(emit_fn_name_id("bar", Some("a::Foo")), expect);
        assert_eq!(
            emit_fn_name_id("a::Foo::newf", Some("Foo")),
            "_RNvMNtC4hexl1aNtNtC4hexl1a3Foo4newf"
        );
        // クレート直下の型
        assert_eq!(
            emit_fn_name_id("Foo::bar", Some("Foo")),
            "_RNvMC4hexlNtC4hexl3Foo3bar"
        );
    }

    #[test]
    fn generic_methods_same_as_rustc_v0() {
        // rustc: `a::Foo::g::<u8>`
        assert_eq!(
            emit_generic_fn_name_id("a::Foo::g", Some("Foo"), &["u8".to_string()]),
            "_RINvMNtC4hexl1aNtNtC4hexl1a3Foo1ghE"
        );
    }

    #[test]
    fn same_name_in_different_types_differs() {
        assert_ne!(
            emit_fn_name_id("new", Some("A")),
            emit_fn_name_id("new", Some("B"))
        );
        assert_ne!(
            emit_fn_name_id("new", Some("A")),
            emit_fn_name_id("new", None)
        );
    }
}
