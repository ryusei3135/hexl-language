//! トークン列 -> CAST。typedef名の判定のため、宣言を読みながら表を更新する。

use super::cast::*;
use super::error::*;
use super::prelude::{TokenInfo, Token};
use std::collections::{HashMap, HashSet};

pub struct Parser {
    toks: Vec<TokenInfo>,
    pos: usize,
    typedefs: HashMap<String, CType>,
    /// 列挙定数名 -> (列挙型名, メンバー名, 値)
    enum_consts: HashMap<String, (String, String, i64)>,
    items: Vec<CItem>,
    defined_tags: HashSet<String>,
    anon: usize,
    pub warnings: Vec<String>,
}

/// 宣言指定子の読み取り結果
struct Specs {
    storage: Storage,
    is_typedef: bool,
    base: CType,
}

enum Suffix {
    Array(Option<usize>),
    Func(Vec<CParam>, bool),
}

const BIN_OPS: &[(&str, BinOp, u8)] = &[
    ("||", BinOp::LogOr, 1),
    ("&&", BinOp::LogAnd, 2),
    ("|", BinOp::BitOr, 3),
    ("^", BinOp::BitXor, 4),
    ("&", BinOp::BitAnd, 5),
    ("==", BinOp::Eq, 6),
    ("!=", BinOp::Ne, 6),
    ("<", BinOp::Lt, 7),
    (">", BinOp::Gt, 7),
    ("<=", BinOp::Le, 7),
    (">=", BinOp::Ge, 7),
    ("<<", BinOp::Shl, 8),
    (">>", BinOp::Shr, 8),
    ("+", BinOp::Add, 9),
    ("-", BinOp::Sub, 9),
    ("*", BinOp::Mul, 10),
    ("/", BinOp::Div, 10),
    ("%", BinOp::Mod, 10),
];

const ASSIGN_OPS: &[(&str, Option<BinOp>)] = &[
    ("=", None),
    ("+=", Some(BinOp::Add)),
    ("-=", Some(BinOp::Sub)),
    ("*=", Some(BinOp::Mul)),
    ("/=", Some(BinOp::Div)),
    ("%=", Some(BinOp::Mod)),
    ("<<=", Some(BinOp::Shl)),
    (">>=", Some(BinOp::Shr)),
    ("&=", Some(BinOp::BitAnd)),
    ("^=", Some(BinOp::BitXor)),
    ("|=", Some(BinOp::BitOr)),
];

impl Parser {
    pub fn new(mut toks: Vec<TokenInfo>) -> Self {
        if !matches!(toks.last().map(|t| &t.token), Some(Token::Eof)) {
            let (line, col) = toks.last().map(|t| (t.line, t.col)).unwrap_or((1, 1));
            toks.push(TokenInfo {
                token: Token::Eof,
                line,
                col,
                start: 0,
                end: 0,
                bol: true,
                space_before: true,
            });
        }
        Parser {
            toks,
            pos: 0,
            typedefs: HashMap::new(),
            enum_consts: HashMap::new(),
            items: Vec::new(),
            defined_tags: HashSet::new(),
            anon: 0,
            warnings: Vec::new(),
        }
    }

    // ---------------------------------------------------------------
    // トークン操作
    // ---------------------------------------------------------------

    fn cur(&self) -> &TokenInfo {
        &self.toks[self.pos.min(self.toks.len() - 1)]
    }
    fn peek_at(&self, n: usize) -> &Token {
        &self.toks[(self.pos + n).min(self.toks.len() - 1)].token
    }
    fn tok(&self) -> &Token {
        self.peek_at(0)
    }
    fn line(&self) -> usize {
        self.cur().line
    }
    fn bump(&mut self) {
        if self.pos < self.toks.len() - 1 {
            self.pos += 1;
        }
    }
    fn is_punct(&self, p: &str) -> bool {
        matches!(self.tok(), Token::Punct { name } if *name == p)
    }
    fn is_punct_at(&self, n: usize, p: &str) -> bool {
        matches!(self.peek_at(n), Token::Punct { name } if *name == p)
    }
    fn is_kw(&self, k: &str) -> bool {
        matches!(self.tok(), Token::KeyWord { name } if name == k)
    }
    fn eat_punct(&mut self, p: &str) -> bool {
        if self.is_punct(p) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn eat_kw(&mut self, k: &str) -> bool {
        if self.is_kw(k) {
            self.bump();
            true
        } else {
            false
        }
    }
    fn expect_punct(&mut self, p: &str) -> PResult<()> {
        if self.eat_punct(p) {
            Ok(())
        } else {
            Err(self.syntax(format!("`{}`が必要です(実際: {})", p, self.describe())))
        }
    }
    fn describe(&self) -> String {
        match self.tok() {
            Token::Eof => "ファイルの終わり".to_string(),
            t => format!("{:?}", t),
        }
    }
    fn syntax(&self, msg: impl Into<String>) -> ParseError {
        let c = self.cur();
        ParseError::syntax(c.line, c.col, msg)
    }
    fn unsupported(&self, msg: impl Into<String>) -> ParseError {
        let c = self.cur();
        ParseError::unsupported(c.line, c.col, msg)
    }
    fn expect_ident(&mut self) -> PResult<String> {
        match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                Ok(name)
            }
            _ => Err(self.syntax(format!("識別子が必要です(実際: {})", self.describe()))),
        }
    }

    /// `__attribute__((..))` `__extension__` `__asm__(..)` などを読み飛ばす
    fn skip_gnu_noise(&mut self) {
        loop {
            match self.tok() {
                Token::Ident { name } if name == "__extension__" => self.bump(),
                Token::Ident { name }
                    if name == "__attribute__" || name == "__asm__" || name == "__asm" =>
                {
                    self.bump();
                    if self.is_punct("(") {
                        self.skip_balanced_parens();
                    }
                }
                _ => break,
            }
        }
    }

    /// 現在位置の`(`から対応する`)`の次まで進む
    fn skip_balanced_parens(&mut self) {
        let mut depth = 0usize;
        loop {
            match self.tok() {
                Token::Punct { name: "(" } => depth += 1,
                Token::Punct { name: ")" } => {
                    depth -= 1;
                    if depth == 0 {
                        self.bump();
                        return;
                    }
                }
                Token::Eof => return,
                _ => {}
            }
            self.bump();
        }
    }

    // ---------------------------------------------------------------
    // 翻訳単位
    // ---------------------------------------------------------------

    pub fn parse_unit(mut self) -> PResult<(Vec<CItem>, Vec<String>)> {
        while !matches!(self.tok(), Token::Eof) {
            self.parse_external()?;
        }
        Ok((self.items, self.warnings))
    }

    fn parse_external(&mut self) -> PResult<()> {
        self.skip_gnu_noise();
        if self.eat_punct(";") {
            return Ok(());
        }
        if self.is_kw("_Static_assert") {
            self.skip_static_assert();
            return Ok(());
        }
        let line = self.line();
        let specs = self.parse_specs(true)?;
        self.skip_gnu_noise();
        if self.eat_punct(";") {
            return Ok(());
        }
        // 最初の宣言子
        let (name, ty) = self.declarator(specs.base.clone(), false)?;
        let name = name.expect("abstract=falseなので名前はある");
        self.skip_gnu_noise();

        if !specs.is_typedef && self.is_punct("{") {
            if let CType::Func { ret, params, variadic } = ty {
                self.items.push(CItem::Func(CFunc {
                    line,
                    name,
                    ret: *ret,
                    params,
                    variadic,
                    is_static: specs.storage == Storage::Static,
                    body: Vec::new(),
                }));
                let idx = self.items.len() - 1;
                // 本体の中で定義された構造体は、関数より前に入れたいので、関数は最後に並べ直す
                let func = self.items.pop().unwrap();
                let _ = idx;
                let body = self.parse_compound_items()?;
                if let CItem::Func(mut f) = func {
                    f.body = body;
                    self.items.push(CItem::Func(f));
                }
                return Ok(());
            }
            return Err(self.syntax("関数以外の宣言に`{`が続いています"));
        }
        if !specs.is_typedef && !matches!(ty, CType::Func { .. }) && self.is_type_start() {
            return Err(self.unsupported("K&R形式(旧式)の関数定義"));
        }

        let decls = self.finish_declarators(&specs, line, name, ty, false)?;
        for d in decls {
            self.push_global(d);
        }
        Ok(())
    }

    fn push_global(&mut self, d: CDecl) {
        if matches!(d.ty, CType::Func { .. }) {
            self.items.push(CItem::Proto { name: d.name });
        } else if d.storage == Storage::Extern {
            self.items.push(CItem::ExternVar { name: d.name });
        } else {
            self.items.push(CItem::Global(d));
        }
    }

    fn skip_static_assert(&mut self) {
        self.bump();
        if self.is_punct("(") {
            self.skip_balanced_parens();
        }
        self.eat_punct(";");
        self.warnings.push(format!("{}行目: _Static_assertは無視しました", self.line()));
    }

    /// 最初の宣言子を読み終わった後、`= init` `,` 次の宣言子 ... `;` までを読む。
    /// typedefはここで登録して、宣言としては返さない。
    fn finish_declarators(
        &mut self,
        specs: &Specs,
        line: usize,
        first_name: String,
        first_ty: CType,
        _local: bool,
    ) -> PResult<Vec<CDecl>> {
        let mut out = Vec::new();
        let mut name = first_name;
        let mut ty = first_ty;
        loop {
            self.skip_gnu_noise();
            let mut init = None;
            if self.eat_punct("=") {
                if specs.is_typedef {
                    return Err(self.syntax("typedefに初期化子は付けられません"));
                }
                init = Some(self.parse_initializer()?);
            }
            if specs.is_typedef {
                self.typedefs.insert(name, ty);
            } else {
                out.push(CDecl { line, name, ty, storage: specs.storage, init });
            }
            if self.eat_punct(",") {
                let (n, t) = self.declarator(specs.base.clone(), false)?;
                name = n.expect("abstract=falseなので名前はある");
                ty = t;
                continue;
            }
            break;
        }
        self.skip_gnu_noise();
        self.expect_punct(";")?;
        Ok(out)
    }

    // ---------------------------------------------------------------
    // 型・宣言指定子
    // ---------------------------------------------------------------

    fn is_typedef_name(&self, name: &str) -> bool {
        self.typedefs.contains_key(name)
    }

    /// 現在位置から型名(宣言の先頭)が始まるか
    fn is_type_start(&self) -> bool {
        self.is_type_start_at(0)
    }

    fn is_type_start_at(&self, n: usize) -> bool {
        match self.peek_at(n) {
            Token::KeyWord { name } => matches!(
                name.as_str(),
                "void" | "char" | "short" | "int" | "long" | "float" | "double" | "signed"
                    | "unsigned" | "_Bool" | "_Complex" | "struct" | "union" | "enum" | "const"
                    | "volatile" | "restrict" | "typedef" | "extern" | "static" | "auto"
                    | "register" | "inline" | "_Noreturn" | "_Atomic" | "_Alignas"
                    | "_Thread_local"
            ),
            Token::Ident { name } => {
                self.is_typedef_name(name)
                    || matches!(
                        name.as_str(),
                        "__extension__" | "__attribute__" | "__restrict" | "__restrict__"
                            | "__inline" | "__inline__" | "__const" | "__volatile__"
                    )
            }
            _ => false,
        }
    }

    fn anon_name(&mut self, kind: &str) -> String {
        self.anon += 1;
        format!("__anon_{}_{}", kind, self.anon)
    }

    /// 宣言指定子を読む。`allow_storage`が偽なら記憶域クラスを許さない(型名・構造体メンバー用)
    fn parse_specs(&mut self, allow_storage: bool) -> PResult<Specs> {
        let mut storage = Storage::None;
        let mut is_typedef = false;
        let mut konst = false;
        let (mut signed, mut unsigned, mut short) = (false, false, false);
        let mut long = 0u8;
        let (mut c_char, mut c_int, mut c_float, mut c_double, mut c_void, mut c_bool) =
            (false, false, false, false, false, false);
        let mut other: Option<CType> = None;
        let mut any = false;

        loop {
            self.skip_gnu_noise();
            match self.tok().clone() {
                Token::KeyWord { name } => {
                    match name.as_str() {
                        "typedef" | "extern" | "static" | "auto" | "register" => {
                            if !allow_storage {
                                return Err(self.syntax("ここでは記憶域クラス指定子は使えません"));
                            }
                            match name.as_str() {
                                "typedef" => is_typedef = true,
                                "extern" => storage = Storage::Extern,
                                "static" => storage = Storage::Static,
                                _ => {}
                            }
                        }
                        "_Thread_local" => return Err(self.unsupported("_Thread_local")),
                        "_Atomic" => return Err(self.unsupported("_Atomic")),
                        "_Complex" | "_Imaginary" => return Err(self.unsupported("複素数型")),
                        "_Alignas" => return Err(self.unsupported("_Alignas")),
                        "const" => konst = true,
                        "volatile" | "restrict" | "inline" | "_Noreturn" => {}
                        "void" => c_void = true,
                        "char" => c_char = true,
                        "short" => short = true,
                        "int" => c_int = true,
                        "long" => long += 1,
                        "float" => c_float = true,
                        "double" => c_double = true,
                        "signed" => signed = true,
                        "unsigned" => unsigned = true,
                        "_Bool" => c_bool = true,
                        "struct" | "union" => {
                            self.bump();
                            let ty = self.parse_struct_or_union(name == "union", is_typedef)?;
                            other = Some(ty);
                            any = true;
                            continue;
                        }
                        "enum" => {
                            self.bump();
                            let ty = self.parse_enum(is_typedef)?;
                            other = Some(ty);
                            any = true;
                            continue;
                        }
                        _ => break,
                    }
                    any = true;
                    self.bump();
                }
                Token::Ident { name } => {
                    match name.as_str() {
                        "__restrict" | "__restrict__" | "__inline" | "__inline__"
                        | "__volatile__" => {
                            self.bump();
                            continue;
                        }
                        "__const" => {
                            konst = true;
                            self.bump();
                            continue;
                        }
                        _ => {}
                    }
                    let seen_base = other.is_some()
                        || signed
                        || unsigned
                        || short
                        || long > 0
                        || c_char
                        || c_int
                        || c_float
                        || c_double
                        || c_void
                        || c_bool;
                    if !seen_base {
                        if let Some(t) = self.typedefs.get(&name) {
                            other = Some(t.clone());
                            any = true;
                            self.bump();
                            continue;
                        }
                    }
                    break;
                }
                _ => break,
            }
        }

        let has_word = signed
            || unsigned
            || short
            || long > 0
            || c_char
            || c_int
            || c_float
            || c_double
            || c_void
            || c_bool;

        let mut base = if let Some(t) = other {
            if has_word {
                return Err(self.syntax("型指定子の組み合わせが不正です"));
            }
            t
        } else if has_word {
            CType::named(&self.base_name(
                signed, unsigned, short, long, c_char, c_int, c_float, c_double, c_void, c_bool,
            )?)
        } else {
            if !any && !allow_storage {
                return Err(self.syntax("型名が必要です"));
            }
            // 型が無い宣言は暗黙のint(K&R)。`static x;` や `main() {}` など
            if storage != Storage::None || is_typedef || any {
                self.warnings
                    .push(format!("{}行目: 型指定子が無いので`int`として扱います", self.line()));
            }
            CType::named("int")
        };
        if konst {
            base = base.with_const();
        }
        Ok(Specs { storage, is_typedef, base })
    }

    #[allow(clippy::too_many_arguments)]
    fn base_name(
        &self,
        signed: bool,
        unsigned: bool,
        short: bool,
        long: u8,
        c_char: bool,
        c_int: bool,
        c_float: bool,
        c_double: bool,
        c_void: bool,
        c_bool: bool,
    ) -> PResult<String> {
        let bad = || self.syntax("型指定子の組み合わせが不正です");
        if signed && unsigned {
            return Err(bad());
        }
        let n_kinds = [c_char, c_float, c_double, c_void, c_bool].iter().filter(|b| **b).count();
        if n_kinds > 1 || long > 2 {
            return Err(bad());
        }
        let sign_ok_for_int_like = !(c_float || c_double || c_void || c_bool);
        if (signed || unsigned) && !sign_ok_for_int_like {
            return Err(bad());
        }
        let name = if c_void {
            if short || long > 0 || c_int {
                return Err(bad());
            }
            "void"
        } else if c_bool {
            "bool"
        } else if c_float {
            if short || long > 0 || c_int {
                return Err(bad());
            }
            "float"
        } else if c_double {
            if short || c_int || long > 1 {
                return Err(bad());
            }
            if long == 1 {
                "long double"
            } else {
                "double"
            }
        } else if c_char {
            if short || long > 0 || c_int {
                return Err(bad());
            }
            if unsigned {
                "unsigned char"
            } else if signed {
                "signed char"
            } else {
                "char"
            }
        } else if short {
            if long > 0 {
                return Err(bad());
            }
            if unsigned {
                "unsigned short"
            } else {
                "short"
            }
        } else if long == 1 {
            if unsigned {
                "unsigned long"
            } else {
                "long"
            }
        } else if long == 2 {
            if unsigned {
                "unsigned long long"
            } else {
                "long long"
            }
        } else if unsigned {
            "unsigned int"
        } else {
            "int"
        };
        Ok(name.to_string())
    }

    fn parse_struct_or_union(&mut self, is_union: bool, typedef_ctx: bool) -> PResult<CType> {
        self.skip_gnu_noise();
        let kind = if is_union { "union" } else { "struct" };
        let mut tag = match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                Some(name)
            }
            _ => None,
        };
        self.skip_gnu_noise();
        if !self.eat_punct("{") {
            return match tag {
                Some(t) => Ok(CType::named(&t)),
                None => Err(self.syntax(format!("{}のタグ名か`{{`が必要です", kind))),
            };
        }
        // 本体あり
        let mut fields: Vec<CField> = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.tok(), Token::Eof) {
                return Err(self.syntax("構造体の`}`が見つかりません"));
            }
            if self.is_kw("_Static_assert") {
                self.skip_static_assert();
                continue;
            }
            let specs = self.parse_specs(false)?;
            if self.is_punct(";") {
                return Err(self.unsupported("無名の構造体/共用体メンバー(C11 anonymous member)"));
            }
            loop {
                let (name, ty) = self.declarator(specs.base.clone(), false)?;
                if self.is_punct(":") {
                    return Err(self.unsupported("ビットフィールド"));
                }
                self.skip_gnu_noise();
                fields.push(CField { name: name.unwrap(), ty });
                if !self.eat_punct(",") {
                    break;
                }
            }
            self.expect_punct(";")?;
        }
        self.expect_punct("}")?;
        self.skip_gnu_noise();

        if tag.is_none() && typedef_ctx {
            // `typedef struct {..} Name;` はtypedef名を型の名前にする
            if let Token::Ident { name } = self.tok() {
                if !self.is_typedef_name(name) {
                    tag = Some(name.clone());
                }
            }
        }
        let name = match tag {
            Some(t) => t,
            None => self.anon_name(kind),
        };
        if !self.defined_tags.insert(name.clone()) {
            return Err(self.syntax(format!("{} `{}`が再定義されています", kind, name)));
        }
        let def = if is_union {
            CDef::Union { name: name.clone(), fields }
        } else {
            CDef::Struct { name: name.clone(), fields }
        };
        self.items.push(CItem::Def(def));
        Ok(CType::named(&name))
    }

    fn parse_enum(&mut self, typedef_ctx: bool) -> PResult<CType> {
        self.skip_gnu_noise();
        let mut tag = match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                Some(name)
            }
            _ => None,
        };
        if !self.eat_punct("{") {
            return match tag {
                Some(t) => Ok(CType::named(&t)),
                None => Err(self.syntax("enumのタグ名か`{`が必要です")),
            };
        }
        // 定数の登録に型名が要るので、先に名前を決める
        if tag.is_none() && typedef_ctx {
            // `}` の後ろのtypedef名を先読み
            let save = self.pos;
            let mut depth = 1usize;
            while depth > 0 && !matches!(self.tok(), Token::Eof) {
                if self.is_punct("{") {
                    depth += 1;
                } else if self.is_punct("}") {
                    depth -= 1;
                }
                self.bump();
            }
            if let Token::Ident { name } = self.tok() {
                if !self.is_typedef_name(name) {
                    tag = Some(name.clone());
                }
            }
            self.pos = save;
        }
        let ename = match tag {
            Some(t) => t,
            None => self.anon_name("enum"),
        };
        if !self.defined_tags.insert(ename.clone()) {
            return Err(self.syntax(format!("enum `{}`が再定義されています", ename)));
        }
        let mut members: Vec<(String, i64)> = Vec::new();
        let mut next: i64 = 0;
        while !self.is_punct("}") {
            let name = self.expect_ident()?;
            if self.eat_punct("=") {
                let e = self.parse_cond()?;
                next = self.const_eval(&e)?;
            }
            self.enum_consts.insert(name.clone(), (ename.clone(), name.clone(), next));
            members.push((name, next));
            next = next.wrapping_add(1);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("}")?;
        self.items.push(CItem::Def(CDef::Enum { name: ename.clone(), members }));
        Ok(CType::named(&ename))
    }

    // ---------------------------------------------------------------
    // 宣言子
    // ---------------------------------------------------------------

    /// `*`と直接宣言子を読み、`base`に型を適用した結果を返す。
    /// `abstract_ok`なら名前が無くてもよい(型名・無名の仮引数)
    fn declarator(&mut self, base: CType, abstract_ok: bool) -> PResult<(Option<String>, CType)> {
        let mut ty = base;
        loop {
            self.skip_gnu_noise();
            if self.eat_punct("*") {
                let mut konst = false;
                loop {
                    match self.tok() {
                        Token::KeyWord { name } if name == "const" => konst = true,
                        Token::KeyWord { name } if name == "volatile" || name == "restrict" => {}
                        Token::Ident { name }
                            if matches!(
                                name.as_str(),
                                "__restrict" | "__restrict__" | "__volatile__"
                            ) => {}
                        Token::Ident { name } if name == "__const" => konst = true,
                        _ => break,
                    }
                    self.bump();
                }
                ty = CType::Ptr { inner: Box::new(ty), konst };
            } else {
                break;
            }
        }
        self.skip_gnu_noise();

        // `( 宣言子 )` の入れ子
        if self.is_punct("(") && self.paren_is_nested_declarator() {
            let inner_start = self.pos + 1;
            self.skip_balanced_parens();
            let ty2 = self.declarator_suffixes(ty)?;
            let end = self.pos;
            self.pos = inner_start;
            let (name, res) = self.declarator(ty2, abstract_ok)?;
            self.expect_punct(")")?;
            self.pos = end;
            return Ok((name, res));
        }

        let name = match self.tok().clone() {
            Token::Ident { name } if !(abstract_ok && self.is_typedef_name(&name)) => {
                self.bump();
                Some(name)
            }
            _ => {
                if abstract_ok {
                    None
                } else {
                    return Err(self.syntax(format!("宣言する名前が必要です(実際: {})", self.describe())));
                }
            }
        };
        let ty = self.declarator_suffixes(ty)?;
        Ok((name, ty))
    }

    fn paren_is_nested_declarator(&self) -> bool {
        match self.peek_at(1) {
            Token::Punct { name } => *name == "*" || *name == "(" || *name == "[",
            Token::Ident { name } => {
                !self.is_typedef_name(name)
                    && !matches!(name.as_str(), "__restrict" | "__restrict__" | "__const")
            }
            _ => false,
        }
    }

    fn declarator_suffixes(&mut self, ty: CType) -> PResult<CType> {
        let mut suffixes: Vec<Suffix> = Vec::new();
        loop {
            self.skip_gnu_noise();
            if self.eat_punct("[") {
                // `[static 10]` `[const]` などは読み飛ばす
                while matches!(self.tok(), Token::KeyWord { name } if name == "static" || name == "const" || name == "restrict" || name == "volatile")
                {
                    self.bump();
                }
                if self.eat_punct("]") {
                    suffixes.push(Suffix::Array(None));
                } else if self.is_punct("*") && self.is_punct_at(1, "]") {
                    return Err(self.unsupported("可変長配列(`[*]`)"));
                } else {
                    let e = self.parse_assign()?;
                    let n = match self.const_eval(&e) {
                        Ok(n) if n >= 0 => n as usize,
                        Ok(_) => return Err(self.syntax("配列の大きさが負です")),
                        Err(_) => {
                            return Err(self.unsupported(
                                "整数定数式でない配列の大きさ(可変長配列 / sizeofを含む式)",
                            ))
                        }
                    };
                    self.expect_punct("]")?;
                    suffixes.push(Suffix::Array(Some(n)));
                }
            } else if self.is_punct("(") {
                self.bump();
                let (params, variadic) = self.parse_params()?;
                suffixes.push(Suffix::Func(params, variadic));
            } else {
                break;
            }
        }
        let mut ty = ty;
        for s in suffixes.into_iter().rev() {
            ty = match s {
                Suffix::Array(len) => CType::Array { inner: Box::new(ty), len },
                Suffix::Func(params, variadic) => {
                    CType::Func { ret: Box::new(ty), params, variadic }
                }
            };
        }
        Ok(ty)
    }

    /// `(`の次から`)`まで
    fn parse_params(&mut self) -> PResult<(Vec<CParam>, bool)> {
        let mut params = Vec::new();
        let mut variadic = false;
        if self.eat_punct(")") {
            return Ok((params, false));
        }
        // `(void)`
        if self.is_kw("void") && self.is_punct_at(1, ")") {
            self.bump();
            self.bump();
            return Ok((params, false));
        }
        loop {
            if self.eat_punct("...") {
                variadic = true;
                break;
            }
            if !self.is_type_start() {
                if matches!(self.tok(), Token::Ident { .. }) {
                    // K&R の識別子リスト
                    return Err(self.unsupported("K&R形式(旧式)の仮引数リスト"));
                }
                return Err(self.syntax(format!("仮引数の型が必要です(実際: {})", self.describe())));
            }
            let specs = self.parse_specs(true)?;
            let (name, ty) = self.declarator(specs.base, true)?;
            // 配列・関数の仮引数はポインタに読み替える
            let ty = match ty {
                CType::Array { inner, .. } => CType::Ptr { inner, konst: false },
                f @ CType::Func { .. } => CType::Ptr { inner: Box::new(f), konst: false },
                t => t,
            };
            params.push(CParam { name, ty });
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct(")")?;
        Ok((params, variadic))
    }

    fn parse_type_name(&mut self) -> PResult<CType> {
        let specs = self.parse_specs(false)?;
        let (_, ty) = self.declarator(specs.base, true)?;
        Ok(ty)
    }

    // ---------------------------------------------------------------
    // 初期化子
    // ---------------------------------------------------------------

    fn parse_initializer(&mut self) -> PResult<CInit> {
        if !self.eat_punct("{") {
            return Ok(CInit::Expr(self.parse_assign()?));
        }
        let mut list = Vec::new();
        while !self.is_punct("}") {
            let mut desigs = Vec::new();
            loop {
                if self.eat_punct(".") {
                    desigs.push(Designator::Field(self.expect_ident()?));
                } else if self.is_punct("[") {
                    self.bump();
                    let e = self.parse_cond()?;
                    let n = self.const_eval(&e)?;
                    if self.is_punct("...") {
                        return Err(self.unsupported("範囲指示子 `[a ... b]`"));
                    }
                    self.expect_punct("]")?;
                    desigs.push(Designator::Index(n));
                } else {
                    break;
                }
            }
            if !desigs.is_empty() {
                self.expect_punct("=")?;
            }
            let item = self.parse_initializer()?;
            list.push((desigs, item));
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("}")?;
        Ok(CInit::List(list))
    }

    // ---------------------------------------------------------------
    // 文
    // ---------------------------------------------------------------

    /// `{`の次から`}`まで読んで、中の文を返す(`{`は呼び出し側が読んでいる前提ではなく、ここで読む)
    fn parse_compound_items(&mut self) -> PResult<Vec<CStmt>> {
        self.expect_punct("{")?;
        let mut items = Vec::new();
        while !self.is_punct("}") {
            if matches!(self.tok(), Token::Eof) {
                return Err(self.syntax("`}`が見つかりません"));
            }
            items.push(self.parse_stmt()?);
        }
        self.expect_punct("}")?;
        Ok(items)
    }

    fn parse_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let mk = |kind| CStmt { line, kind };
        self.skip_gnu_noise();

        // ラベル
        if let Token::Ident { name } = self.tok().clone() {
            if self.is_punct_at(1, ":") && !self.is_typedef_name(&name) {
                self.bump();
                self.bump();
                return Ok(mk(CStmtKind::Label(name)));
            }
        }

        match self.tok().clone() {
            Token::Punct { name: "{" } => {
                let items = self.parse_compound_items()?;
                Ok(mk(CStmtKind::Block(items)))
            }
            Token::Punct { name: ";" } => {
                self.bump();
                Ok(mk(CStmtKind::Expr(None)))
            }
            Token::KeyWord { name } => match name.as_str() {
                "if" => {
                    self.bump();
                    self.expect_punct("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_punct(")")?;
                    let then = Box::new(self.parse_stmt()?);
                    let els = if self.eat_kw("else") {
                        Some(Box::new(self.parse_stmt()?))
                    } else {
                        None
                    };
                    Ok(mk(CStmtKind::If(cond, then, els)))
                }
                "switch" => {
                    self.bump();
                    self.expect_punct("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_punct(")")?;
                    let body = Box::new(self.parse_stmt()?);
                    Ok(mk(CStmtKind::Switch(cond, body)))
                }
                "while" => {
                    self.bump();
                    self.expect_punct("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_punct(")")?;
                    let body = Box::new(self.parse_stmt()?);
                    Ok(mk(CStmtKind::While(cond, body)))
                }
                "do" => {
                    self.bump();
                    let body = Box::new(self.parse_stmt()?);
                    if !self.eat_kw("while") {
                        return Err(self.syntax("`do`の後ろに`while`が必要です"));
                    }
                    self.expect_punct("(")?;
                    let cond = self.parse_expr()?;
                    self.expect_punct(")")?;
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::DoWhile(body, cond)))
                }
                "for" => {
                    self.bump();
                    self.expect_punct("(")?;
                    let init = if self.eat_punct(";") {
                        None
                    } else if self.is_type_start() {
                        Some(Box::new(self.parse_decl_stmt()?))
                    } else {
                        let l = self.line();
                        let e = self.parse_expr()?;
                        self.expect_punct(";")?;
                        Some(Box::new(CStmt { line: l, kind: CStmtKind::Expr(Some(e)) }))
                    };
                    let cond = if self.is_punct(";") { None } else { Some(self.parse_expr()?) };
                    self.expect_punct(";")?;
                    let step = if self.is_punct(")") { None } else { Some(self.parse_expr()?) };
                    self.expect_punct(")")?;
                    let body = Box::new(self.parse_stmt()?);
                    Ok(mk(CStmtKind::For { init, cond, step, body }))
                }
                "return" => {
                    self.bump();
                    let e = if self.is_punct(";") { None } else { Some(self.parse_expr()?) };
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::Return(e)))
                }
                "break" => {
                    self.bump();
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::Break))
                }
                "continue" => {
                    self.bump();
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::Continue))
                }
                "goto" => {
                    self.bump();
                    let l = self.expect_ident()?;
                    self.expect_punct(";")?;
                    Ok(mk(CStmtKind::Goto(l)))
                }
                "case" => {
                    self.bump();
                    let e = self.parse_cond()?;
                    if self.is_punct("...") {
                        return Err(self.unsupported("case範囲(GNU拡張 `case a ... b:`)"));
                    }
                    self.expect_punct(":")?;
                    Ok(mk(CStmtKind::Case(e)))
                }
                "default" => {
                    self.bump();
                    self.expect_punct(":")?;
                    Ok(mk(CStmtKind::Default))
                }
                "_Static_assert" => {
                    self.skip_static_assert();
                    Ok(mk(CStmtKind::Expr(None)))
                }
                _ if self.is_type_start() => self.parse_decl_stmt(),
                _ => self.parse_expr_stmt(),
            },
            Token::Ident { .. } if self.is_type_start() => self.parse_decl_stmt(),
            _ => self.parse_expr_stmt(),
        }
    }

    fn parse_expr_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let e = self.parse_expr()?;
        self.expect_punct(";")?;
        Ok(CStmt { line, kind: CStmtKind::Expr(Some(e)) })
    }

    /// 関数内の宣言(`;`まで読む)
    fn parse_decl_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let specs = self.parse_specs(true)?;
        if self.eat_punct(";") {
            return Ok(CStmt { line, kind: CStmtKind::Decl(Vec::new()) });
        }
        let (name, ty) = self.declarator(specs.base.clone(), false)?;
        let decls = self.finish_declarators(&specs, line, name.unwrap(), ty, true)?;
        // 関数内の関数プロトタイプ / extern宣言は無視
        let decls: Vec<CDecl> = decls
            .into_iter()
            .filter(|d| !matches!(d.ty, CType::Func { .. }) && d.storage != Storage::Extern)
            .collect();
        Ok(CStmt { line, kind: CStmtKind::Decl(decls) })
    }

    // ---------------------------------------------------------------
    // 式
    // ---------------------------------------------------------------

    fn mk(&self, line: usize, kind: CExprKind) -> CExpr {
        CExpr { line, kind }
    }

    pub fn parse_expr(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let first = self.parse_assign()?;
        if !self.is_punct(",") {
            return Ok(first);
        }
        let mut list = vec![first];
        while self.eat_punct(",") {
            list.push(self.parse_assign()?);
        }
        Ok(self.mk(line, CExprKind::Comma(list)))
    }

    pub fn parse_assign(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let lhs = self.parse_cond()?;
        if let Token::Punct { name } = self.tok() {
            if let Some((_, op)) = ASSIGN_OPS.iter().find(|(s, _)| s == name) {
                let op = *op;
                self.bump();
                let rhs = self.parse_assign()?;
                return Ok(self.mk(line, CExprKind::Assign(op, Box::new(lhs), Box::new(rhs))));
            }
        }
        Ok(lhs)
    }

    fn parse_cond(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let c = self.parse_binary(1)?;
        if self.eat_punct("?") {
            let a = self.parse_expr()?;
            self.expect_punct(":")?;
            let b = self.parse_cond()?;
            return Ok(self.mk(line, CExprKind::Cond(Box::new(c), Box::new(a), Box::new(b))));
        }
        Ok(c)
    }

    fn parse_binary(&mut self, min_prec: u8) -> PResult<CExpr> {
        let line = self.line();
        let mut lhs = self.parse_cast()?;
        loop {
            let (op, prec) = match self.tok() {
                Token::Punct { name } => match BIN_OPS.iter().find(|(s, _, _)| s == name) {
                    Some((_, op, p)) if *p >= min_prec => (*op, *p),
                    _ => break,
                },
                _ => break,
            };
            self.bump();
            let rhs = self.parse_binary(prec + 1)?;
            lhs = self.mk(line, CExprKind::Binary(op, Box::new(lhs), Box::new(rhs)));
        }
        Ok(lhs)
    }

    fn parse_cast(&mut self) -> PResult<CExpr> {
        let line = self.line();
        if self.is_punct("(") && self.is_type_start_at(1) {
            self.bump();
            let ty = self.parse_type_name()?;
            self.expect_punct(")")?;
            if self.is_punct("{") {
                return Err(self.unsupported("複合リテラル `(T){...}`"));
            }
            let e = self.parse_cast()?;
            return Ok(self.mk(line, CExprKind::Cast(ty, Box::new(e))));
        }
        self.parse_unary()
    }

    fn parse_unary(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let op = match self.tok() {
            Token::Punct { name } => match *name {
                "++" => Some(UnOp::PreInc),
                "--" => Some(UnOp::PreDec),
                "&" => Some(UnOp::Addr),
                "*" => Some(UnOp::Deref),
                "+" => Some(UnOp::Plus),
                "-" => Some(UnOp::Neg),
                "~" => Some(UnOp::BitNot),
                "!" => Some(UnOp::Not),
                _ => None,
            },
            _ => None,
        };
        if let Some(op) = op {
            self.bump();
            let e = self.parse_cast()?;
            return Ok(self.mk(line, CExprKind::Unary(op, Box::new(e))));
        }
        if self.is_kw("sizeof") {
            self.bump();
            if self.is_punct("(") && self.is_type_start_at(1) {
                self.bump();
                let ty = self.parse_type_name()?;
                self.expect_punct(")")?;
                return Ok(self.mk(line, CExprKind::SizeofType(ty)));
            }
            let e = self.parse_unary()?;
            return Ok(self.mk(line, CExprKind::SizeofExpr(Box::new(e))));
        }
        if self.is_kw("_Alignof") {
            return Err(self.unsupported("_Alignof"));
        }
        self.parse_postfix()
    }

    fn parse_postfix(&mut self) -> PResult<CExpr> {
        let line = self.line();
        let mut e = self.parse_primary()?;
        loop {
            if self.eat_punct("[") {
                let idx = self.parse_expr()?;
                self.expect_punct("]")?;
                e = self.mk(line, CExprKind::Index(Box::new(e), Box::new(idx)));
            } else if self.eat_punct("(") {
                let mut args = Vec::new();
                if !self.is_punct(")") {
                    loop {
                        args.push(self.parse_assign()?);
                        if !self.eat_punct(",") {
                            break;
                        }
                    }
                }
                self.expect_punct(")")?;
                e = self.mk(line, CExprKind::Call(Box::new(e), args));
            } else if self.eat_punct(".") {
                let m = self.expect_ident()?;
                e = self.mk(line, CExprKind::Member(Box::new(e), m, false));
            } else if self.eat_punct("->") {
                let m = self.expect_ident()?;
                e = self.mk(line, CExprKind::Member(Box::new(e), m, true));
            } else if self.eat_punct("++") {
                e = self.mk(line, CExprKind::Unary(UnOp::PostInc, Box::new(e)));
            } else if self.eat_punct("--") {
                e = self.mk(line, CExprKind::Unary(UnOp::PostDec, Box::new(e)));
            } else {
                break;
            }
        }
        Ok(e)
    }

    fn parse_primary(&mut self) -> PResult<CExpr> {
        let line = self.line();
        match self.tok().clone() {
            Token::Ident { name } => {
                self.bump();
                if let Some((ename, variant, _)) = self.enum_consts.get(&name) {
                    return Ok(self.mk(
                        line,
                        CExprKind::EnumConst { ename: ename.clone(), variant: variant.clone() },
                    ));
                }
                Ok(self.mk(line, CExprKind::Ident(name)))
            }
            Token::IntLiteral { value, .. } => {
                self.bump();
                Ok(self.mk(line, CExprKind::Int(value.to_string())))
            }
            Token::FloatLiteral { value, .. } => {
                self.bump();
                Ok(self.mk(line, CExprKind::Float(fmt_float(value))))
            }
            Token::CharLiteral { prefix, body } => {
                if !prefix.is_empty() {
                    return Err(self.unsupported("ワイド文字定数(L'' u'' U'')"));
                }
                let v = decode_char(&body).map_err(|m| self.syntax(m))?;
                self.bump();
                Ok(self.mk(line, CExprKind::Char(v)))
            }
            Token::StringLiteral { .. } => {
                let mut s = String::new();
                while let Token::StringLiteral { prefix, body } = self.tok().clone() {
                    if !prefix.is_empty() {
                        return Err(self.unsupported("ワイド/UTF文字列リテラル(L\"\" u\"\" U\"\" u8\"\")"));
                    }
                    s.push_str(&decode_string(&body).map_err(|m| self.syntax(m))?);
                    self.bump();
                }
                Ok(self.mk(line, CExprKind::Str(s)))
            }
            Token::Punct { name: "(" } => {
                if self.is_punct_at(1, "{") {
                    return Err(self.unsupported("GNUの文式 `({ ... })`"));
                }
                self.bump();
                let e = self.parse_expr()?;
                self.expect_punct(")")?;
                Ok(e)
            }
            Token::KeyWord { name } if name == "_Generic" => Err(self.unsupported("_Generic")),
            _ => Err(self.syntax(format!("式が必要です(実際: {})", self.describe()))),
        }
    }

    // ---------------------------------------------------------------
    // 定数式の評価(配列の大きさ・列挙子の値・指示子の添字)
    // ---------------------------------------------------------------

    fn const_eval(&self, e: &CExpr) -> PResult<i64> {
        let fail = |m: &str| ParseError::unsupported(e.line, 0, format!("整数定数式として評価できません: {}", m));
        Ok(match &e.kind {
            CExprKind::Int(s) => s.parse::<u64>().map_err(|_| fail("整数が大きすぎます"))? as i64,
            CExprKind::Char(v) => *v,
            CExprKind::EnumConst { ename, variant } => {
                self.enum_consts
                    .values()
                    .find(|(en, v, _)| en == ename && v == variant)
                    .map(|(_, _, n)| *n)
                    .ok_or_else(|| fail("列挙定数"))?
            }
            CExprKind::Cast(_, inner) => self.const_eval(inner)?,
            CExprKind::Unary(op, a) => {
                let a = self.const_eval(a)?;
                match op {
                    UnOp::Plus => a,
                    UnOp::Neg => a.wrapping_neg(),
                    UnOp::BitNot => !a,
                    UnOp::Not => (a == 0) as i64,
                    _ => return Err(fail("単項演算子")),
                }
            }
            CExprKind::Binary(op, a, b) => {
                let (a, b) = (self.const_eval(a)?, self.const_eval(b)?);
                match op {
                    BinOp::Mul => a.wrapping_mul(b),
                    BinOp::Div => {
                        if b == 0 {
                            return Err(fail("0で割っています"));
                        }
                        a.wrapping_div(b)
                    }
                    BinOp::Mod => {
                        if b == 0 {
                            return Err(fail("0で割っています"));
                        }
                        a.wrapping_rem(b)
                    }
                    BinOp::Add => a.wrapping_add(b),
                    BinOp::Sub => a.wrapping_sub(b),
                    BinOp::Shl => a.wrapping_shl(b as u32),
                    BinOp::Shr => a.wrapping_shr(b as u32),
                    BinOp::Lt => (a < b) as i64,
                    BinOp::Gt => (a > b) as i64,
                    BinOp::Le => (a <= b) as i64,
                    BinOp::Ge => (a >= b) as i64,
                    BinOp::Eq => (a == b) as i64,
                    BinOp::Ne => (a != b) as i64,
                    BinOp::BitAnd => a & b,
                    BinOp::BitXor => a ^ b,
                    BinOp::BitOr => a | b,
                    BinOp::LogAnd => (a != 0 && b != 0) as i64,
                    BinOp::LogOr => (a != 0 || b != 0) as i64,
                }
            }
            CExprKind::Cond(c, a, b) => {
                if self.const_eval(c)? != 0 {
                    self.const_eval(a)?
                } else {
                    self.const_eval(b)?
                }
            }
            CExprKind::SizeofType(_) | CExprKind::SizeofExpr(_) => return Err(fail("sizeof")),
            _ => return Err(fail("定数でない式")),
        })
    }
}

fn fmt_float(v: f64) -> String {
    let s = format!("{:?}", v);
    if s.contains('.') || s.contains("inf") || s.contains("NaN") {
        return s;
    }
    if let Some(i) = s.find('e') {
        let (m, e) = s.split_at(i);
        return format!("{}.0{}", m, e);
    }
    format!("{}.0", s)
}

/// エスケープを展開して、1文字ぶんずつバイト列(コードポイント)にする
fn decode_escapes(body: &str) -> Result<Vec<u32>, String> {
    let chars: Vec<char> = body.chars().collect();
    let mut out = Vec::new();
    let mut i = 0;
    while i < chars.len() {
        let c = chars[i];
        i += 1;
        if c != '\\' {
            out.push(c as u32);
            continue;
        }
        let Some(&e) = chars.get(i) else {
            return Err("文字列の末尾に`\\`があります".to_string());
        };
        i += 1;
        let v = match e {
            'n' => 10,
            't' => 9,
            'r' => 13,
            'a' => 7,
            'b' => 8,
            'f' => 12,
            'v' => 11,
            'e' => 27,
            '\\' => 92,
            '\'' => 39,
            '"' => 34,
            '?' => 63,
            'x' => {
                let mut v: u32 = 0;
                let mut n = 0;
                while let Some(d) = chars.get(i).and_then(|c| c.to_digit(16)) {
                    v = v.wrapping_mul(16).wrapping_add(d);
                    i += 1;
                    n += 1;
                }
                if n == 0 {
                    return Err("`\\x`の後ろに16進数が必要です".to_string());
                }
                v & 0xff
            }
            '0'..='7' => {
                let mut v = e.to_digit(8).unwrap();
                let mut n = 1;
                while n < 3 {
                    match chars.get(i).and_then(|c| c.to_digit(8)) {
                        Some(d) => {
                            v = v * 8 + d;
                            i += 1;
                            n += 1;
                        }
                        None => break,
                    }
                }
                v & 0xff
            }
            'u' | 'U' => {
                let len = if e == 'u' { 4 } else { 8 };
                let mut v: u32 = 0;
                for _ in 0..len {
                    let d = chars
                        .get(i)
                        .and_then(|c| c.to_digit(16))
                        .ok_or_else(|| "ユニバーサル文字名が不正です".to_string())?;
                    v = v * 16 + d;
                    i += 1;
                }
                v
            }
            other => return Err(format!("未知のエスケープシーケンス `\\{}`", other)),
        };
        out.push(v);
    }
    Ok(out)
}

fn decode_char(body: &str) -> Result<i64, String> {
    let cs = decode_escapes(body)?;
    if cs.is_empty() {
        return Err("空の文字定数です".to_string());
    }
    // 複数文字定数はgcc同様に先頭が上位バイト
    let mut v: i64 = 0;
    for c in &cs {
        v = (v << 8) | (*c as i64 & if cs.len() > 1 { 0xff } else { -1 });
    }
    Ok(v)
}

fn decode_string(body: &str) -> Result<String, String> {
    let cs = decode_escapes(body)?;
    Ok(cs.into_iter().filter_map(char::from_u32).collect())
}
