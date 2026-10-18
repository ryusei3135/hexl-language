//! データ型: 宣言指定子・struct/union/enum・宣言子・typedef・初期化子・変数宣言

use super::*;

/// 宣言指定子の読み取り結果
pub(super) struct Specs {
    pub(super) storage: Storage,
    pub(super) is_typedef: bool,
    pub(super) base: CType,
}

pub(super) enum Suffix {
    Array(Option<usize>),
    Func(Vec<CParam>, bool),
}

impl Parser {
    pub(super) fn finish_declarators(
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
                out.push(CDecl {
                    line,
                    name,
                    ty,
                    storage: specs.storage,
                    init,
                });
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

    pub(super) fn is_typedef_name(&self, name: &str) -> bool {
        self.typedefs.contains_key(name)
    }

    /// 現在位置から型名(宣言の先頭)が始まるか
    pub(super) fn is_type_start(&self) -> bool {
        self.is_type_start_at(0)
    }

    pub(super) fn is_type_start_at(&self, n: usize) -> bool {
        match self.peek_at(n) {
            Token::KeyWord { name } => matches!(
                name.as_str(),
                "void"
                    | "char"
                    | "short"
                    | "int"
                    | "long"
                    | "float"
                    | "double"
                    | "signed"
                    | "unsigned"
                    | "_Bool"
                    | "_Complex"
                    | "struct"
                    | "union"
                    | "enum"
                    | "const"
                    | "volatile"
                    | "restrict"
                    | "typedef"
                    | "extern"
                    | "static"
                    | "auto"
                    | "register"
                    | "inline"
                    | "_Noreturn"
                    | "_Atomic"
                    | "_Alignas"
                    | "_Thread_local"
            ),
            Token::Ident { name } => {
                self.is_typedef_name(name)
                    || matches!(
                        name.as_str(),
                        "__extension__"
                            | "__attribute__"
                            | "__restrict"
                            | "__restrict__"
                            | "__inline"
                            | "__inline__"
                            | "__const"
                            | "__volatile__"
                    )
            }
            _ => false,
        }
    }

    pub(super) fn anon_name(&mut self, kind: &str) -> String {
        self.anon += 1;
        format!("__anon_{}_{}", kind, self.anon)
    }

    /// 宣言指定子を読む。`allow_storage`が偽なら記憶域クラスを許さない(型名・構造体メンバー用)
    pub(super) fn parse_specs(&mut self, allow_storage: bool) -> PResult<Specs> {
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
                self.warnings.push(format!(
                    "{}行目: 型指定子が無いので`int`として扱います",
                    self.line()
                ));
            }
            CType::named("int")
        };
        if konst {
            base = base.with_const();
        }
        Ok(Specs {
            storage,
            is_typedef,
            base,
        })
    }

    #[allow(clippy::too_many_arguments)]
    pub(super) fn base_name(
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
        let n_kinds = [c_char, c_float, c_double, c_void, c_bool]
            .iter()
            .filter(|b| **b)
            .count();
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
            if long == 1 { "long double" } else { "double" }
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
            if unsigned { "unsigned short" } else { "short" }
        } else if long == 1 {
            if unsigned { "unsigned long" } else { "long" }
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

    pub(super) fn parse_struct_or_union(
        &mut self,
        is_union: bool,
        typedef_ctx: bool,
    ) -> PResult<CType> {
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
                fields.push(CField {
                    name: name.unwrap(),
                    ty,
                });
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
            CDef::Union {
                name: name.clone(),
                fields,
            }
        } else {
            CDef::Struct {
                name: name.clone(),
                fields,
            }
        };
        self.items.push(CItem::Def(def));
        Ok(CType::named(&name))
    }

    pub(super) fn parse_enum(&mut self, typedef_ctx: bool) -> PResult<CType> {
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
            self.enum_consts
                .insert(name.clone(), (ename.clone(), name.clone(), next));
            members.push((name, next));
            next = next.wrapping_add(1);
            if !self.eat_punct(",") {
                break;
            }
        }
        self.expect_punct("}")?;
        self.items.push(CItem::Def(CDef::Enum {
            name: ename.clone(),
            members,
        }));
        Ok(CType::named(&ename))
    }

    // ---------------------------------------------------------------
    // 宣言子
    // ---------------------------------------------------------------

    /// `*`と直接宣言子を読み、`base`に型を適用した結果を返す。
    /// `abstract_ok`なら名前が無くてもよい(型名・無名の仮引数)
    pub(super) fn declarator(
        &mut self,
        base: CType,
        abstract_ok: bool,
    ) -> PResult<(Option<String>, CType)> {
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
                ty = CType::Ptr {
                    inner: Box::new(ty),
                    konst,
                };
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
                    return Err(
                        self.syntax(format!("宣言する名前が必要です(実際: {})", self.describe()))
                    );
                }
            }
        };
        let ty = self.declarator_suffixes(ty)?;
        Ok((name, ty))
    }

    pub(super) fn paren_is_nested_declarator(&self) -> bool {
        match self.peek_at(1) {
            Token::Punct { name } => *name == "*" || *name == "(" || *name == "[",
            Token::Ident { name } => {
                !self.is_typedef_name(name)
                    && !matches!(name.as_str(), "__restrict" | "__restrict__" | "__const")
            }
            _ => false,
        }
    }

    pub(super) fn declarator_suffixes(&mut self, ty: CType) -> PResult<CType> {
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
                            ));
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
                Suffix::Array(len) => CType::Array {
                    inner: Box::new(ty),
                    len,
                },
                Suffix::Func(params, variadic) => CType::Func {
                    ret: Box::new(ty),
                    params,
                    variadic,
                },
            };
        }
        Ok(ty)
    }

    pub(super) fn parse_type_name(&mut self) -> PResult<CType> {
        let specs = self.parse_specs(false)?;
        let (_, ty) = self.declarator(specs.base, true)?;
        Ok(ty)
    }

    // ---------------------------------------------------------------
    // 初期化子
    // ---------------------------------------------------------------

    pub(super) fn parse_initializer(&mut self) -> PResult<CInit> {
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

    /// 関数内の宣言(`;`まで読む)
    pub(super) fn parse_decl_stmt(&mut self) -> PResult<CStmt> {
        let line = self.line();
        let specs = self.parse_specs(true)?;
        if self.eat_punct(";") {
            return Ok(CStmt {
                line,
                kind: CStmtKind::Decl(Vec::new()),
            });
        }
        let (name, ty) = self.declarator(specs.base.clone(), false)?;
        let decls = self.finish_declarators(&specs, line, name.unwrap(), ty, true)?;
        // 関数内の関数プロトタイプ / extern宣言は無視
        let decls: Vec<CDecl> = decls
            .into_iter()
            .filter(|d| !matches!(d.ty, CType::Func { .. }) && d.storage != Storage::Extern)
            .collect();
        Ok(CStmt {
            line,
            kind: CStmtKind::Decl(decls),
        })
    }
}
