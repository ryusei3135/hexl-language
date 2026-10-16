use super::*;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct CallInfo {
    pub name: String,
    /// `func<int>(..)`の`<>`の中身。呼び出す関数を
    /// `FuncDefine`の`temp_ty`と突き合わせて特定するために使う。
    /// ジェネリクスでない関数の呼び出しでは空
    pub temp_ty: Vec<TyNode>,
    pub args: Vec<Expr>,
}

/// `#include`で指定されたパスの実体
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportPath {
    /// `#include mod::file`のように、`::`で区切られた
    /// セグメントをそのままディレクトリ・ファイル名として使う
    /// (従来の書き方。拡張子は付いていないので`gen_path`側で付与する)
    Segments(Vec<String>),
    /// `#include "mod/file.hexl"`のように、拡張子を含む
    /// ファイルパスをそのまま文字列で指定する書き方
    Literal(String),
}

/// `#include`で取り込んだ関数を、どういう名前で
/// 使えるようにするかを表す
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ImportKind {
    Auto,
    /// 公開されている関数をすべて、モジュール名を付けて取り込む。
    /// `None`の場合はファイル名からモジュール名を自動生成する
    /// - `#include Name="mod/file.hexl"` -> `Some("Name")` (`Name::func()`)
    /// - `#include "mod/file.hexl"` -> `None` (ファイル名から`file::func()`)
    Module(Option<String>),
    /// 指定した関数だけを、モジュール名を付けずに取り込む
    /// (`#include "mod/file.hexl"::func` -> `func()`)
    Func(String),
    /// 公開されている関数をすべて、モジュール名を付けずに取り込む
    /// (`#include "mod/file.hexl"::*` -> `func()`)
    Glob,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ModPath {
    pub path: ImportPath,
    pub kind: ImportKind,
}

impl ModPath {
    /// 従来の書き方(`mod::file`)用の空の`ModPath`を作る
    #[inline(always)]
    pub fn new_segments() -> Self {
        Self {
            path: ImportPath::Segments(Vec::new()),
            kind: ImportKind::Auto,
        }
    }

    /// 新しい書き方(`"mod/file.hexl"`)用の`ModPath`を作る
    #[inline(always)]
    pub fn new_literal(literal: &str, kind: ImportKind) -> Self {
        Self {
            path: ImportPath::Literal(literal.to_string()),
            kind,
        }
    }

    /// 従来の書き方(`Segments`)でのみ使う。
    /// `Literal`に対して呼び出すと`Err`を返す
    #[inline(always)]
    pub fn add_path(&mut self, path_name: &str) -> Result<(), crate::compiler::err::ErrKind> {
        match &mut self.path {
            ImportPath::Segments(segments) => {
                segments.push(path_name.to_string());
                Ok(())
            }
            ImportPath::Literal(_) => Err(crate::compiler::err::ErrKind::UnexpectedToken),
        }
    }

    /// `#include`先のファイルパスを生成する
    pub fn gen_path(&self) -> String {
        match &self.path {
            // 既に拡張子込みの完全なパスなので、そのまま返す
            ImportPath::Literal(literal) => literal.clone(),
            ImportPath::Segments(segments) => {
                // ディレクトリの最初のパスのインデックス
                const PATH_START: usize = 0;

                let mut path = String::new();
                for (index, dir) in segments.iter().enumerate() {
                    if index != PATH_START {
                        path.push('/');
                    }
                    path.push_str(dir.as_str());
                }
                // 最後に拡張子を追加
                path.push_str(".hexl");
                path
            }
        }
    }

    /// パスの最後のセグメントを除いた、
    /// 親のディレクトリのファイルパスを生成する
    /// (例: `mod::file::func` -> `mod/file.hexl`)
    ///
    /// `#include`で指定されたパスがファイルとして
    /// 存在しない場合、最後のセグメントは
    /// 関数名とみなし、その手前までを
    /// ファイルパスとして探すのに使う。
    /// `Segments`形式(従来の書き方)でのみ使う
    /// (`Literal`に対して呼び出すと`Err`を返す)
    pub fn gen_parent_path(&self) -> Result<String, crate::compiler::err::ErrKind> {
        const PATH_START: usize = 0;

        let ImportPath::Segments(segments) = &self.path else {
            return Err(crate::compiler::err::ErrKind::UnexpectedToken);
        };

        let mut path = String::new();
        let parent_len = segments.len().saturating_sub(1);
        for (index, dir) in segments[..parent_len].iter().enumerate() {
            if index != PATH_START {
                path.push('/');
            }
            path.push_str(dir.as_str());
        }
        path.push_str(".hexl");
        Ok(path)
    }

    /// パスの最後のセグメント(`Segments`の場合)や、
    /// ファイル名から拡張子を除いた部分(`Literal`の場合)を返す。
    /// モジュール名を自動生成する際や、`Auto`判定で関数名として
    /// 使う際に使う
    pub fn last_segment(&self) -> String {
        match &self.path {
            ImportPath::Literal(literal) => {
                let file_name = literal.rsplit('/').next().unwrap_or(literal.as_str());
                file_name
                    .strip_suffix(".hexl")
                    .unwrap_or(file_name)
                    .to_string()
            }
            ImportPath::Segments(segments) => {
                // パスが空の場合は空文字列
                segments.last().cloned().unwrap_or_default()
            }
        }
    }
}

/// 変数を定義する
/// ノード
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct DefineVar {
    pub name: String,
    pub value: Box<Expr>,
    pub ty: TyNode,
    pub var_attr: VarMutAttr,
}

impl DefineVar {
    pub fn new(name: &String, value: Expr, ty: &TyNode, var_attr: VarMutAttr) -> Self {
        Self {
            name: name.to_string(),
            value: Box::new(value),
            ty: ty.clone(),
            var_attr: var_attr,
        }
    }

    pub fn wrap(self) -> Expr {
        Expr::DefVar(self)
    }
}

/// `cond`の条件式の中だけで使える、バリアントの
/// パターン束縛(Rustの`if let`に近い)
///
/// - `M(val) := value`      : `val`の型はバリアント`M`の中身の型から推論する(`ty`は`None`)
/// - `M(val): int = value`  : `val`の型を明示する(`ty`は`Some(int)`)
/// - `M(val) mut:= value`   : `val`を可変にする(`var_attr`は`Var`)
///
/// `value`が`M`であるときだけ真になり、その中身が`val`に束縛される
/// (`val`は真のときの本体の中でだけ使える)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VariantBind {
    /// `A::M(val)`の`A`(`M(val)`のように省略されたときは`None`)
    pub variant_ty: Option<String>,
    /// `M(val)`の`M`(バリアントのメンバーの名前)
    pub variant: String,
    /// `M(val)`の`val`(束縛する変数の名前)
    pub bind: String,
    /// `None`なら`:=`(型推論)、`Some`なら`: ty =`(型を明示)
    pub ty: Option<TyNode>,
    /// 判定する対象の式
    pub value: Box<Expr>,
    /// `val`が不変(省略)か、`mut`/`const`か
    /// (`M(val) mut:= value` / `M(val) mut: ty = value`)
    pub var_attr: VarMutAttr,
}

impl VariantBind {
    pub fn new(variant: &str, bind: &str, ty: Option<TyNode>, value: Expr) -> Self {
        Self {
            variant_ty: None,
            variant: variant.to_string(),
            bind: bind.to_string(),
            ty,
            value: Box::new(value),
            var_attr: VarMutAttr::Invar,
        }
    }

    /// `val`の可変/不変を設定する
    pub fn with_var_attr(mut self, var_attr: VarMutAttr) -> Self {
        self.var_attr = var_attr;
        self
    }

    /// `A::M(val)`の`A`を設定する
    pub fn with_variant_ty(mut self, variant_ty: Option<String>) -> Self {
        self.variant_ty = variant_ty;
        self
    }

    pub fn wrap(self) -> Expr {
        Expr::VariantBind(self)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct AssignVar {
    pub name: String,
    // Expr::Varなど
    pub dst: Box<Expr>,
    pub value: Box<Expr>,
}

impl AssignVar {
    #[inline(always)]
    pub fn new(name: &str, dst: Expr, value: Expr) -> Expr {
        Expr::Assign(Self {
            name: name.to_string(),
            dst: Box::new(dst),
            value: Box::new(value),
        })
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Expr {
    RangeNode((String, String)),
    /// ポインタ関係
    GetAddress(Box<Expr>),
    ConnectAddr(Box<Expr>),

    Number(String),
    /// 小数点を含む数値リテラル: `3.14` (ソース上の文字列のまま保持する)
    Float(String),
    Str(String),
    Var(String),
    CallFunc(CallInfo),
    Assign(AssignVar),
    Add((Box<Expr>, Box<Expr>)),
    Sub((Box<Expr>, Box<Expr>)),
    Mul((Box<Expr>, Box<Expr>)),
    Div((Box<Expr>, Box<Expr>)),
    Surplus((Box<Expr>, Box<Expr>)),

    LessThen((Box<Expr>, Box<Expr>)),
    GreaterThen((Box<Expr>, Box<Expr>)),
    LessThenEq((Box<Expr>, Box<Expr>)),
    GreaterThenEq((Box<Expr>, Box<Expr>)),

    AndCalcul((Box<Expr>, Box<Expr>)),
    XorCalcul((Box<Expr>, Box<Expr>)),
    OrCalcul((Box<Expr>, Box<Expr>)),

    LShift((Box<Expr>, Box<Expr>)),
    RShift((Box<Expr>, Box<Expr>)),
    /// `==` 値の等価比較
    Equal((Box<Expr>, Box<Expr>)),
    NotEq((Box<Expr>, Box<Expr>)),
    Match {
        pattern: Option<Box<Expr>>,
        arms: Vec<MatchArm>,
        arm_else: Option<Body>,
    },
    Loop {
        pattern: Option<Box<Expr>>,
        body: Body,
    },
    InitStruct {
        is_self: bool,
        name: String,
        fields: HashMap<String, Box<Expr>>,
    },
    InitVariant {
        is_self: bool,
        name: String,
        fields: HashMap<String, Box<Expr>>,
    },
    /// 列挙型のメンバへのアクセス: `Name::Mem`
    EnumVariant {
        name: String,
        variant: String,
    },
    /// 配列リテラル: `{100, 100, 100, 100}`
    Array(Vec<Expr>),
    /// 配列の要素へのアクセス(参照/代入どちらの対象にもなる)
    /// `[name index]`
    /// - name: 配列変数の名前
    /// - dst: 配列本体を指す式
    /// - index: 添字を表す式(`Number`/`Var`/`Member`(`a.a`)/四則演算など)
    ///   `const`や即値ではない場合は`else_idx`が必要(`ir/checker/access_mem.rs`)
    RefArray {
        name: String,
        dst: Box<Expr>,
        index: Box<Expr>,
        /// 配列の範囲外にアクセスしたときの要素の値
        else_idx: Option<Box<Expr>>,
    },

    DefVar(DefineVar),
    /// `cond`の条件式の中だけで使える: `M(val) := value` / `M(val): ty = value`
    /// (`expr/variant_bind.rs`)
    VariantBind(VariantBind),
    Scope {
        scope: Vec<String>,
        target: Box<Expr>,
    },
    Member {
        scope: Vec<String>,
        target: Box<Expr>,
    },
    /// ポインタが指す構造体のメンバー/メゾットへのアクセス
    /// `[name].member` (フィールドの参照/代入) / `[name].method(..)` (メゾット呼び出し)
    /// - name: ポインタ変数の名前
    /// - target: `Var(メンバー名)`(フィールドアクセス)、または`CallFunc(..)`(メゾット呼び出し)
    PtrMember {
        name: String,
        target: Box<Expr>,
    },
}

impl Expr {
    pub fn wrap(left: Expr, right: Expr) -> (Box<Expr>, Box<Expr>) {
        (Box::new(left), Box::new(right))
    }

    pub fn wrap_group2(self) -> Group2Node {
        Group2Node::Expr(self)
    }
}
