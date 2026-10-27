//! CAST -> `node`(Group1Node / Group2Info / Expr)への変換。
//! `node`に直接対応するものが無い構文(`if`/`while`/`for`/`switch`/`&&`など)は、
//! 意味を変えない形で`Match`/`Loop`などに書き換える。書き換えできないものは
//! `ParseErrorKind::Unsupported`を返す(一覧はNOTE.md)。

use super::cast::*;
use super::error::*;
use super::prelude::*;
use std::collections::{HashMap, HashSet};
use std::rc::Rc;

#[derive(Clone, Copy, PartialEq, Eq)]
enum TagKind {
    Struct,
    Union,
    Enum,
}

struct StructInfo {
    fields: Vec<CField>,
}

/// 入れ子になった`break`/`continue`の行き先
enum Frame {
    /// ループ。`cont`は`continue`の前に実行する文(`for`の更新式・`do-while`の判定)
    Loop {
        cont: Vec<Group2Info>,
    },
    Switch,
}

pub struct Lowerer {
    tags: HashMap<String, TagKind>,
    structs: HashMap<String, StructInfo>,
    out: Vec<Group1Node>,
    pub warnings: Vec<String>,

    // --- 関数ごとの状態 ---
    /// Cの名前 -> 出力する名前(同じ`Body`内で名前が衝突したときに改名する)
    scopes: Vec<HashMap<String, String>>,
    /// 今の`Body`(Vec)で既に宣言された出力名
    body_names: Vec<HashSet<String>>,
    frames: Vec<Frame>,
    rename_seq: usize,
    cur_line: usize,
    self_file_name: Rc<String>,
}

fn unsup<T>(line: usize, msg: impl Into<String>) -> PResult<T> {
    Err(ParseError::unsupported(line, 0, msg))
}

fn span_at(line: usize, self_file_name: &Rc<String>) -> std::rc::Rc<crate::compiler::err::Span> {
    std::rc::Rc::new(crate::compiler::err::Span::new(
        line,
        0,
        "undefine".to_string(),
        self_file_name,
    ))
}

fn expr_at(node: ExprNode, line: usize, self_file_name: &Rc<String>) -> Expr {
    Expr::new(node, span_at(line, self_file_name))
}

impl Lowerer {
    pub fn new(file_name: &Rc<String>) -> Self {
        Lowerer {
            tags: HashMap::new(),
            structs: HashMap::new(),
            out: Vec::new(),
            warnings: Vec::new(),
            scopes: Vec::new(),
            body_names: Vec::new(),
            frames: Vec::new(),
            rename_seq: 0,
            cur_line: 0,
            self_file_name: Rc::clone(&file_name),
        }
    }

    pub fn lower_unit(mut self, items: Vec<CItem>) -> PResult<(Vec<Group1Node>, Vec<String>)> {
        // 先に型の種類を集める(前方参照のため)
        for it in &items {
            if let CItem::Def(d) = it {
                match d {
                    CDef::Struct { name, .. } => self.tags.insert(name.clone(), TagKind::Struct),
                    CDef::Union { name, .. } => self.tags.insert(name.clone(), TagKind::Union),
                    CDef::Enum { name, .. } => self.tags.insert(name.clone(), TagKind::Enum),
                };
            }
        }
        for it in &items {
            if let CItem::Def(CDef::Struct { name, fields } | CDef::Union { name, fields }) = it {
                self.structs.insert(
                    name.clone(),
                    StructInfo {
                        fields: fields.clone(),
                    },
                );
            }
        }
        for it in items {
            match it {
                CItem::Def(d) => self.lower_def(d)?,
                CItem::Func(f) => self.lower_func(f)?,
                CItem::Proto { name } => {
                    self.warnings.push(format!(
                        "関数宣言 `{}` は読み飛ばしました(nodeに対応するものが無い)",
                        name
                    ));
                }
                CItem::ExternVar { name } => {
                    self.warnings
                        .push(format!("extern変数宣言 `{}` は読み飛ばしました", name));
                }
                CItem::Global(d) => {
                    return unsup(
                        d.line,
                        format!(
                            "グローバル変数 `{}`(Group1Nodeにグローバル変数のノードが無い)",
                            d.name
                        ),
                    );
                }
            }
        }
        Ok((self.out, self.warnings))
    }

    // ---------------------------------------------------------------
    // 型
    // ---------------------------------------------------------------

    /// 型の`(大きさ, 境界)`(バイト、LP64)。決められなければ`None`
    ///
    /// 構造体は、メンバーを定義順に境界へ合わせて並べ、全体を最大の境界に切り上げる。
    /// 共用体は最大のメンバーの大きさ(境界に切り上げ)。列挙型は`int`と同じ
    fn size_align(&self, t: &CType) -> Option<(u64, u64)> {
        self.size_align_in(t, &mut Vec::new())
    }

    /// `seen`は計算中のタグ(自分自身を含む構造体の無限再帰を止める)
    fn size_align_in(&self, t: &CType, seen: &mut Vec<String>) -> Option<(u64, u64)> {
        if let Some(r) = t.prim_size_align() {
            return Some(r);
        }
        match t {
            CType::Named { name, .. } => {
                if seen.contains(name) {
                    return None;
                }
                match self.tags.get(name)? {
                    TagKind::Enum => Some((4, 4)),
                    kind => {
                        let info = self.structs.get(name)?;
                        seen.push(name.clone());
                        let mut offset = 0u64;
                        let mut max_align = 1u64;
                        let mut max_size = 0u64;
                        for f in &info.fields {
                            let (size, align) = self.size_align_in(&f.ty, seen)?;
                            max_align = max_align.max(align);
                            if *kind == TagKind::Union {
                                max_size = max_size.max(size);
                            } else {
                                offset = offset.div_ceil(align) * align + size;
                            }
                        }
                        seen.pop();
                        let total = if *kind == TagKind::Union {
                            max_size
                        } else {
                            offset
                        };
                        Some((total.div_ceil(max_align) * max_align, max_align))
                    }
                }
            }
            CType::Array {
                inner,
                len: Some(n),
            } => {
                let (size, align) = self.size_align_in(inner, seen)?;
                Some((size.checked_mul(*n as u64)?, align))
            }
            _ => None,
        }
    }

    fn to_ty(&self, t: &CType, line: usize) -> PResult<TyNode> {
        Ok(match t {
            CType::Named { name, .. } => TyNode::Ty(name.clone()),
            CType::Ptr { inner, .. } => {
                let is_const = inner.is_const();
                TyNode::Pointer {
                    is_const,
                    ty_name: Box::new(self.to_ty(inner, line)?),
                    range: None,
                }
            }
            CType::Array { inner, len } => {
                let Some(len) = len else {
                    return unsup(line, "大きさの無い配列型(初期化子から大きさを決められない)");
                };
                TyNode::Stack {
                    name: Box::new(self.to_ty(inner, line)?),
                    len: *len,
                }
            }
            CType::Func { .. } => {
                return unsup(line, "関数型・関数ポインタ(TyNodeに対応するものが無い)");
            }
        })
    }

    fn tag_kind(&self, t: &CType) -> Option<TagKind> {
        match t {
            CType::Named { name, .. } => self.tags.get(name).copied(),
            _ => None,
        }
    }

    // ---------------------------------------------------------------
    // 型定義
    // ---------------------------------------------------------------

    fn lower_def(&mut self, d: CDef) -> PResult<()> {
        match d {
            CDef::Struct { name, fields } => {
                let mut fs = Vec::new();
                for f in &fields {
                    fs.push(StructField {
                        name: f.name.clone(),
                        ty: self.to_ty(&f.ty, 0)?,
                        span: span_at(0, &self.self_file_name),
                    });
                }
                self.out.push(StructDefine::new(
                    name,
                    fs,
                    Vec::new(),
                    span_at(0, &self.self_file_name),
                ));
            }
            CDef::Union { name, fields } => {
                let mut fs = Vec::new();
                for f in &fields {
                    fs.push(VariantField {
                        name: f.name.clone(),
                        ty: Some(self.to_ty(&f.ty, 0)?),
                        mode: VariantMode::Unsafe,
                        span: span_at(0, &self.self_file_name),
                    });
                }
                self.out.push(VariantDefine::new(
                    name,
                    fs,
                    Vec::new(),
                    span_at(0, &self.self_file_name),
                ));
            }
            CDef::Enum { name, members } => {
                for (i, (m, v)) in members.iter().enumerate() {
                    if *v != i as i64 {
                        return unsup(
                            0,
                            format!(
                                "enum `{}` のメンバー `{}` の値が{}ですが、nodeの列挙型は定義順(0,1,2..)の値しか持てません",
                                name, m, v
                            ),
                        );
                    }
                }
                let fs = members
                    .into_iter()
                    .map(|(m, _)| VariantField::typeless_new(m, span_at(0, &self.self_file_name)))
                    .collect();
                self.out.push(VariantDefine::new(
                    name,
                    fs,
                    Vec::new(),
                    span_at(0, &self.self_file_name),
                ));
            }
        }
        Ok(())
    }

    // ---------------------------------------------------------------
    // 関数
    // ---------------------------------------------------------------

    fn lower_func(&mut self, f: CFunc) -> PResult<()> {
        if f.variadic {
            return unsup(f.line, format!("可変長引数の関数 `{}`(`...`)", f.name));
        }
        self.cur_line = f.line;
        self.scopes = vec![HashMap::new()];
        self.body_names = vec![HashSet::new()];
        self.frames.clear();

        let mut params = Vec::new();
        for (i, p) in f.params.iter().enumerate() {
            let name = p.name.clone().unwrap_or_else(|| format!("_arg{}", i));
            self.scopes[0].insert(name.clone(), name.clone());
            self.body_names[0].insert(name.clone());
            params.push(ArgsNode {
                name,
                ty: self.to_ty(&p.ty, f.line)?,
                var_attr: if p.ty.is_const() {
                    VarMutAttr::Invar
                } else {
                    VarMutAttr::Var
                },
                span: span_at(f.line, &self.self_file_name),
            });
        }
        let ret_ty = self.to_ty(&f.ret, f.line)?;
        let is_void = matches!(&f.ret, CType::Named { name, .. } if name == "void");

        let mut body: Vec<Group2Info> = Vec::new();
        for s in &f.body {
            self.lower_stmt(s, &mut body)?;
        }
        // void関数の末尾の`return;`は不要
        if is_void {
            if let Some(Group2Info {
                node:
                    Group2Node::Stmt(StmtNode::Return(Expr {
                        node: ExprNode::Number(n),
                        ..
                    })),
                ..
            }) = body.last()
            {
                if n == "0" && ends_with_bare_return(&f.body) {
                    body.pop();
                }
            }
        }

        let mut node = FuncDefine::new(
            f.name,
            params,
            ret_ty,
            !f.is_static,
            span_at(f.line, &self.self_file_name),
        );
        if let Group1Node::FuncDefine(fd) = &mut node {
            for b in body {
                fd.add(b);
            }
        }
        self.out.push(node);
        Ok(())
    }

    // ---------------------------------------------------------------
    // 名前の管理
    // ---------------------------------------------------------------

    fn resolve(&self, name: &str) -> String {
        for s in self.scopes.iter().rev() {
            if let Some(n) = s.get(name) {
                return n.clone();
            }
        }
        name.to_string()
    }

    /// 局所変数を宣言する。同じ`Body`の中で名前が衝突したら改名する
    /// (`for(int i..)`を2つ並べたときなど、スコープを平らにした結果の衝突を避けるため)
    fn declare(&mut self, name: &str) -> String {
        let mut emitted = name.to_string();
        let set = self.body_names.last_mut().expect("body scope");
        if set.contains(&emitted) {
            loop {
                self.rename_seq += 1;
                let cand = format!("{}__{}", name, self.rename_seq);
                if !set.contains(&cand) {
                    emitted = cand;
                    break;
                }
            }
        }
        set.insert(emitted.clone());
        self.scopes
            .last_mut()
            .expect("scope")
            .insert(name.to_string(), emitted.clone());
        emitted
    }

    // ---------------------------------------------------------------
    // 文
    // ---------------------------------------------------------------

    fn info(&self, line: usize, node: Group2Node) -> Group2Info {
        node.gen_group_info(line, span_at(line, &self.self_file_name))
    }

    /// 新しい`Body`を作って、中で`s`を変換する
    fn lower_body(&mut self, s: &CStmt) -> PResult<Body> {
        self.lower_body_with(std::slice::from_ref(s), Vec::new(), Vec::new())
    }

    fn lower_body_with(
        &mut self,
        stmts: &[CStmt],
        prefix: Vec<Group2Info>,
        suffix: Vec<Group2Info>,
    ) -> PResult<Body> {
        self.body_names.push(HashSet::new());
        self.scopes.push(HashMap::new());
        let mut v: Vec<Group2Info> = prefix;
        let mut res = Ok(());
        for s in stmts {
            res = self.lower_stmt(s, &mut v);
            if res.is_err() {
                break;
            }
        }
        self.scopes.pop();
        self.body_names.pop();
        res?;
        v.extend(suffix);
        Ok(v)
    }

    fn lower_stmt(&mut self, s: &CStmt, out: &mut Vec<Group2Info>) -> PResult<()> {
        let line = s.line;
        self.cur_line = line;
        match &s.kind {
            CStmtKind::Decl(decls) => {
                for d in decls {
                    let node = self.lower_decl(d)?;
                    out.push(self.info(line, Group2Node::Expr(node)));
                }
            }
            CStmtKind::Expr(None) => {}
            CStmtKind::Expr(Some(e)) => self.lower_expr_stmt(e, out)?,
            CStmtKind::Block(items) => {
                // `Body`に「ブロック」というノードが無いので、親の`Body`へ平らに展開する
                self.scopes.push(HashMap::new());
                let mut res = Ok(());
                for it in items {
                    res = self.lower_stmt(it, out);
                    if res.is_err() {
                        break;
                    }
                }
                self.scopes.pop();
                res?;
            }
            CStmtKind::If(c, t, e) => {
                let m = self.lower_if(c, t, e.as_deref())?;
                out.push(self.info(line, Group2Node::Expr(m)));
            }
            CStmtKind::Switch(c, body) => {
                let m = self.lower_switch(c, body, line)?;
                out.push(self.info(line, Group2Node::Expr(m)));
            }
            CStmtKind::While(c, body) => {
                let l = self.lower_while(c, body, line)?;
                out.push(self.info(line, Group2Node::Expr(l)));
            }
            CStmtKind::DoWhile(body, c) => {
                let l = self.lower_do_while(body, c, line)?;
                out.push(self.info(line, Group2Node::Expr(l)));
            }
            CStmtKind::For {
                init,
                cond,
                step,
                body,
            } => {
                self.lower_for(
                    init.as_deref(),
                    cond.as_ref(),
                    step.as_ref(),
                    body,
                    line,
                    out,
                )?;
            }
            CStmtKind::Return(e) => {
                let v = match e {
                    Some(e) => self.lower_expr(e, true)?,
                    None => void_value(line, &self.self_file_name),
                };
                out.push(self.info(line, Group2Node::Stmt(StmtNode::Return(v))));
            }
            CStmtKind::Break => match self.frames.last() {
                Some(Frame::Loop { .. }) => {
                    out.push(self.info(
                        line,
                        Group2Node::Stmt(StmtNode::Break(span_at(line, &self.self_file_name))),
                    ));
                }
                Some(Frame::Switch) => {
                    return unsup(
                        line,
                        "switchの途中にある`break`(case本体の末尾以外。Matchにはswitchを抜ける手段が無い)",
                    );
                }
                None => {
                    return Err(ParseError::syntax(
                        line,
                        0,
                        "ループ/switchの外に`break`があります",
                    ));
                }
            },
            CStmtKind::Continue => {
                let cont = self.frames.iter().rev().find_map(|f| match f {
                    Frame::Loop { cont } => Some(cont.clone()),
                    Frame::Switch => None,
                });
                match cont {
                    Some(c) => {
                        out.extend(c);
                        out.push(self.info(line, Group2Node::Stmt(StmtNode::Continue)));
                    }
                    None => {
                        return Err(ParseError::syntax(
                            line,
                            0,
                            "ループの外に`continue`があります",
                        ));
                    }
                }
            }
            CStmtKind::Goto(l) => {
                return unsup(line, format!("goto {}(nodeにgoto/ラベルが無い)", l));
            }
            CStmtKind::Label(l) => {
                return unsup(line, format!("ラベル {}:(nodeにgoto/ラベルが無い)", l));
            }
            CStmtKind::Case(_) | CStmtKind::Default => {
                return unsup(
                    line,
                    "switch直下以外にあるcase/defaultラベル(Duff's deviceなど)",
                );
            }
        }
        Ok(())
    }

    fn lower_decl(&mut self, d: &CDecl) -> PResult<Expr> {
        let line = d.line;
        // 初期化子から配列の大きさを決める
        let ty = fix_array_len(&d.ty, d.init.as_ref());
        let value = self.lower_init(&ty, d.init.as_ref(), line)?;
        let mut tynode = self.to_ty(&ty, line)?;
        if d.storage == Storage::Static {
            tynode = match tynode {
                TyNode::Stack { name, len } => TyNode::Static { name, len },
                other => TyNode::Static {
                    name: Box::new(other),
                    len: 1,
                },
            };
        }
        // 初期化子は宣言した変数自身をまだ見えない状態で評価したいので、宣言は最後
        let name = self.declare(&d.name);
        let var_attr = if d.ty.is_const() {
            VarMutAttr::Invar
        } else {
            VarMutAttr::Var
        };
        Ok(DefineVar::new(&name, value, &tynode, var_attr)
            .wrap(span_at(line, &self.self_file_name)))
    }

    fn lower_init(&mut self, ty: &CType, init: Option<&CInit>, line: usize) -> PResult<Expr> {
        match init {
            None => Ok(match ty {
                CType::Array { .. } => {
                    expr_at(ExprNode::Array(Vec::new()), line, &self.self_file_name)
                }
                _ => match self.tag_kind(ty) {
                    Some(TagKind::Struct) => expr_at(
                        ExprNode::InitStruct {
                            is_self: false,
                            name: type_name(ty),
                            fields: HashMap::new(),
                        },
                        line,
                        &self.self_file_name,
                    ),
                    Some(TagKind::Union) => expr_at(
                        ExprNode::InitVariant {
                            is_self: false,
                            name: type_name(ty),
                            fields: HashMap::new(),
                        },
                        line,
                        &self.self_file_name,
                    ),
                    // スカラ・ポインタ・列挙型は0で初期化(Cでは不定値だが、0にしても矛盾しない)
                    _ => expr_at(
                        ExprNode::Number("0".to_string()),
                        line,
                        &self.self_file_name,
                    ),
                },
            }),
            Some(CInit::Expr(e)) => self.lower_expr(e, true),
            Some(CInit::List(items)) => self.lower_init_list(ty, items, line),
        }
    }

    fn lower_init_list(
        &mut self,
        ty: &CType,
        items: &[(Vec<Designator>, CInit)],
        line: usize,
    ) -> PResult<Expr> {
        match ty {
            CType::Array { inner, .. } => {
                let mut v = Vec::new();
                for (d, it) in items {
                    if !d.is_empty() {
                        return unsup(
                            line,
                            "配列の添字指示子 `[i] = v`(Expr::Arrayは先頭からの並びしか持てない)",
                        );
                    }
                    v.push(self.lower_init(inner, Some(it), line)?);
                }
                Ok(expr_at(ExprNode::Array(v), line, &self.self_file_name))
            }
            _ => match self.tag_kind(ty) {
                Some(kind @ (TagKind::Struct | TagKind::Union)) => {
                    let name = type_name(ty);
                    let fields_def: Vec<CField> = match self.structs.get(&name) {
                        Some(s) => s.fields.clone(),
                        None => {
                            return unsup(
                                line,
                                format!("`{}`の定義が見えないので初期化子の対応が取れない", name),
                            );
                        }
                    };
                    let mut fields: HashMap<String, Box<Expr>> = HashMap::new();
                    let mut next = 0usize;
                    for (d, it) in items {
                        let idx = match d.first() {
                            Some(Designator::Field(n)) => {
                                if d.len() > 1 {
                                    return unsup(line, "入れ子の指示子 `.a.b = v`");
                                }
                                fields_def
                                    .iter()
                                    .position(|f| &f.name == n)
                                    .ok_or_else(|| {
                                        ParseError::syntax(
                                            line,
                                            0,
                                            format!("`{}`にメンバー`{}`がありません", name, n),
                                        )
                                    })?
                            }
                            Some(Designator::Index(_)) => {
                                return unsup(line, "構造体への添字指示子");
                            }
                            None => next,
                        };
                        let Some(f) = fields_def.get(idx) else {
                            return Err(ParseError::syntax(
                                line,
                                0,
                                format!("`{}`の初期化子が多すぎます", name),
                            ));
                        };
                        let v = self.lower_init(&f.ty, Some(it), line)?;
                        fields.insert(f.name.clone(), Box::new(v));
                        next = idx + 1;
                        if kind == TagKind::Union {
                            break;
                        }
                    }
                    Ok(if kind == TagKind::Struct {
                        expr_at(
                            ExprNode::InitStruct {
                                is_self: false,
                                name,
                                fields,
                            },
                            line,
                            &self.self_file_name,
                        )
                    } else {
                        expr_at(
                            ExprNode::InitVariant {
                                is_self: false,
                                name,
                                fields,
                            },
                            line,
                            &self.self_file_name,
                        )
                    })
                }
                _ => {
                    // `int x = {1};` はスカラを波括弧で包んだだけ
                    if items.len() == 1 && items[0].0.is_empty() {
                        return self.lower_init(ty, Some(&items[0].1), line);
                    }
                    Err(ParseError::syntax(
                        line,
                        0,
                        "スカラ型に波括弧の初期化子が付いています",
                    ))
                }
            },
        }
    }

    // ---------------------------------------------------------------
    // 制御構文
    // ---------------------------------------------------------------

    fn lower_if(&mut self, c: &CExpr, t: &CStmt, e: Option<&CStmt>) -> PResult<Expr> {
        let mut arms = Vec::new();
        let mut arm_else = None;
        let (mut cond, mut then, mut els) = (c, t, e);
        loop {
            let pat = self.lower_cond(cond)?;
            let body = self.lower_body(then)?;
            arms.push(MatchArm {
                pattern: Box::new(pat),
                body,
                span: span_at(cond.line, &self.self_file_name),
            });
            match els {
                None => break,
                Some(CStmt {
                    kind: CStmtKind::If(c2, t2, e2),
                    ..
                }) => {
                    cond = c2;
                    then = t2;
                    els = e2.as_deref();
                }
                Some(other) => {
                    arm_else = Some(self.lower_body(other)?);
                    break;
                }
            }
        }
        Ok(expr_at(
            ExprNode::Match {
                pattern: None,
                arms,
                arm_else,
            },
            c.line,
            &self.self_file_name,
        ))
    }

    fn guard_break(&mut self, cond: &CExpr, line: usize) -> PResult<Option<Group2Info>> {
        if is_const_true(cond) {
            return Ok(None);
        }
        let (e, is_bool) = self.lower_expr_ex(cond, true)?;
        let neg = negate(e, is_bool, line, &self.self_file_name);
        let brk = self.info(
            line,
            Group2Node::Stmt(StmtNode::Break(span_at(line, &self.self_file_name))),
        );
        let m = expr_at(
            ExprNode::Match {
                pattern: None,
                arms: vec![MatchArm {
                    pattern: Box::new(neg),
                    body: vec![brk],
                    span: span_at(line, &self.self_file_name),
                }],
                arm_else: None,
            },
            line,
            &self.self_file_name,
        );
        Ok(Some(self.info(line, Group2Node::Expr(m))))
    }

    fn lower_while(&mut self, c: &CExpr, body: &CStmt, line: usize) -> PResult<Expr> {
        let guard = self.guard_break(c, line)?;
        self.frames.push(Frame::Loop { cont: Vec::new() });
        let b = self.lower_body_with(
            std::slice::from_ref(body),
            guard.into_iter().collect(),
            Vec::new(),
        );
        self.frames.pop();
        Ok(expr_at(
            ExprNode::Loop {
                pattern: None,
                body: b?,
            },
            line,
            &self.self_file_name,
        ))
    }

    fn lower_do_while(&mut self, body: &CStmt, c: &CExpr, line: usize) -> PResult<Expr> {
        let guard: Vec<Group2Info> = self.guard_break(c, line)?.into_iter().collect();
        self.frames.push(Frame::Loop {
            cont: guard.clone(),
        });
        let b = self.lower_body_with(std::slice::from_ref(body), Vec::new(), guard);
        self.frames.pop();
        Ok(expr_at(
            ExprNode::Loop {
                pattern: None,
                body: b?,
            },
            line,
            &self.self_file_name,
        ))
    }

    fn lower_for(
        &mut self,
        init: Option<&CStmt>,
        cond: Option<&CExpr>,
        step: Option<&CExpr>,
        body: &CStmt,
        line: usize,
        out: &mut Vec<Group2Info>,
    ) -> PResult<()> {
        // `for`の初期化で宣言した変数は`for`の中だけで見える
        self.scopes.push(HashMap::new());
        let r = (|| -> PResult<()> {
            if let Some(i) = init {
                self.lower_stmt(i, out)?;
            }
            let guard: Vec<Group2Info> = match cond {
                Some(c) => self.guard_break(c, line)?.into_iter().collect(),
                None => Vec::new(),
            };
            let mut step_nodes = Vec::new();
            if let Some(s) = step {
                self.lower_expr_stmt(s, &mut step_nodes)?;
            }
            self.frames.push(Frame::Loop {
                cont: step_nodes.clone(),
            });
            let b = self.lower_body_with(std::slice::from_ref(body), guard, step_nodes);
            self.frames.pop();
            out.push(self.info(
                line,
                Group2Node::Expr(expr_at(
                    ExprNode::Loop {
                        pattern: None,
                        body: b?,
                    },
                    line,
                    &self.self_file_name,
                )),
            ));
            Ok(())
        })();
        self.scopes.pop();
        r
    }

    fn lower_switch(&mut self, c: &CExpr, body: &CStmt, line: usize) -> PResult<Expr> {
        let CStmtKind::Block(items) = &body.kind else {
            return unsup(line, "`{}`で囲まれていないswitch本体");
        };
        // ラベルごとのグループに分ける: (ラベル列, 本体の文)
        enum Label<'a> {
            Case(&'a CExpr),
            Default,
        }
        let mut groups: Vec<(Vec<Label>, Vec<&CStmt>)> = Vec::new();
        let mut cur_labels: Vec<Label> = Vec::new();
        let mut cur_body: Vec<&CStmt> = Vec::new();
        let mut in_labels = true;
        for it in items {
            match &it.kind {
                CStmtKind::Case(_) | CStmtKind::Default => {
                    if !in_labels {
                        groups.push((
                            std::mem::take(&mut cur_labels),
                            std::mem::take(&mut cur_body),
                        ));
                        in_labels = true;
                    }
                    match &it.kind {
                        CStmtKind::Case(e) => cur_labels.push(Label::Case(e)),
                        _ => cur_labels.push(Label::Default),
                    }
                }
                _ => {
                    if cur_labels.is_empty() && groups.is_empty() {
                        return unsup(it.line, "switchの最初のcaseより前にある文");
                    }
                    in_labels = false;
                    cur_body.push(it);
                }
            }
        }
        groups.push((cur_labels, cur_body));

        // 各グループの本体と、「次へ落ちる(fallthrough)か」を決める
        let n = groups.len();
        let mut bodies: Vec<Vec<&CStmt>> = Vec::new();
        let mut falls: Vec<bool> = Vec::new();
        for (_, b) in &groups {
            let (stripped, terminated) = strip_trailing_break(b);
            falls.push(!terminated);
            bodies.push(stripped);
        }

        let pattern = self.lower_expr(c, true)?;
        let mut arms = Vec::new();
        let mut arm_else: Option<Body> = None;
        for i in 0..n {
            if groups[i].0.is_empty() {
                continue;
            }
            // 落ちる分は、続くグループの本体を後ろにつなげる(コードを複製する)
            let mut chain: Vec<&CStmt> = bodies[i].clone();
            let mut j = i;
            while falls[j] && j + 1 < n {
                j += 1;
                chain.extend(bodies[j].iter().copied());
            }
            self.frames.push(Frame::Switch);
            let lowered = self.lower_body_with(
                &chain.into_iter().cloned().collect::<Vec<_>>(),
                Vec::new(),
                Vec::new(),
            );
            self.frames.pop();
            let lowered = lowered?;
            for l in &groups[i].0 {
                match l {
                    Label::Case(e) => {
                        let p = self.lower_expr(e, true)?;
                        arms.push(MatchArm {
                            pattern: Box::new(p),
                            body: lowered.clone(),
                            span: span_at(e.line, &self.self_file_name),
                        });
                    }
                    Label::Default => arm_else = Some(lowered.clone()),
                }
            }
        }
        Ok(expr_at(
            ExprNode::Match {
                pattern: Some(Box::new(pattern)),
                arms,
                arm_else,
            },
            line,
            &self.self_file_name,
        ))
    }

    // ---------------------------------------------------------------
    // 式
    // ---------------------------------------------------------------

    fn lower_expr_stmt(&mut self, e: &CExpr, out: &mut Vec<Group2Info>) -> PResult<()> {
        if let CExprKind::Comma(list) = &e.kind {
            for x in list {
                self.lower_expr_stmt(x, out)?;
            }
            return Ok(());
        }
        let v = self.lower_expr(e, false)?;
        out.push(self.info(e.line, Group2Node::Expr(v)));
        Ok(())
    }

    fn lower_expr(&mut self, e: &CExpr, used: bool) -> PResult<Expr> {
        Ok(self.lower_expr_ex(e, used)?.0)
    }

    /// 条件として使う式(0/1の値になる形にする)
    fn lower_cond(&mut self, e: &CExpr) -> PResult<Expr> {
        let (x, is_bool) = self.lower_expr_ex(e, true)?;
        Ok(to_bool(x, is_bool, e.line, &self.self_file_name))
    }

    fn bin(&mut self, a: &CExpr, b: &CExpr) -> PResult<(Box<Expr>, Box<Expr>)> {
        Ok((
            Box::new(self.lower_expr(a, true)?),
            Box::new(self.lower_expr(b, true)?),
        ))
    }

    /// `(変換した式, 値が必ず0か1か)`
    fn lower_expr_ex(&mut self, e: &CExpr, used: bool) -> PResult<(Expr, bool)> {
        let line = e.line;
        let r = match &e.kind {
            CExprKind::Ident(n) => ExprNode::Var(self.resolve(n)),
            CExprKind::EnumConst { ename, variant } => ExprNode::EnumVariant {
                name: ename.clone(),
                variant: variant.clone(),
            },
            CExprKind::Int(s) => ExprNode::Number(s.clone()),
            CExprKind::Float(s) => ExprNode::Float(s.clone()),
            CExprKind::Char(v) => ExprNode::Number(v.to_string()),
            CExprKind::Str(s) => ExprNode::Str(s.clone()),
            CExprKind::Unary(op, a) => return self.lower_unary(*op, a, used, line),
            CExprKind::Binary(op, a, b) => return self.lower_binary(*op, a, b, line),
            CExprKind::Member(..) => return Ok((self.lower_member(e)?, false)),
            CExprKind::Assign(op, l, r) => {
                let dst = self.lower_expr(l, true)?;
                let name = self.root_name(l);
                let value = match op {
                    None => self.lower_expr(r, true)?,
                    Some(op) => {
                        if !side_effect_free(l) {
                            return unsup(
                                line,
                                "副作用のある左辺への複合代入(`a[i++] += 1`など。左辺が2回評価されてしまう)",
                            );
                        }
                        let (a, b) = self.bin(l, r)?;
                        make_bin(*op, a, b, line, &self.self_file_name)
                    }
                };
                ExprNode::Assign(AssignVar {
                    name,
                    dst: Box::new(dst),
                    value: Box::new(value),
                })
            }
            CExprKind::Cond(c, a, b) => {
                let pat = self.lower_cond(c)?;
                let va = self.lower_expr(a, true)?;
                let vb = self.lower_expr(b, true)?;
                let ba = vec![self.info(line, Group2Node::Expr(va))];
                let bb = vec![self.info(line, Group2Node::Expr(vb))];
                ExprNode::Match {
                    pattern: None,
                    arms: vec![MatchArm {
                        pattern: Box::new(pat),
                        body: ba,
                        span: span_at(line, &self.self_file_name),
                    }],
                    arm_else: Some(bb),
                }
            }
            CExprKind::Call(f, args) => {
                let CExprKind::Ident(name) = &f.kind else {
                    return unsup(line, "関数名以外を呼び出す式(関数ポインタ経由の呼び出し)");
                };
                let mut a = Vec::new();
                for x in args {
                    a.push(self.lower_expr(x, true)?);
                }
                ExprNode::CallFunc(CallInfo {
                    name: name.clone(),
                    temp_ty: Vec::new(),
                    args: a,
                })
            }
            CExprKind::Index(base, idx) => {
                let dst = self.lower_expr(base, true)?;
                let index = self.lower_expr(idx, true)?;
                ExprNode::RefArray {
                    name: self.root_name(base),
                    dst: Box::new(dst),
                    index: Box::new(index),
                    else_idx: None,
                }
            }
            CExprKind::Cast(t, a) => {
                let ty = self.to_ty(t, line)?;
                ExprNode::Cast((ty, Box::new(self.lower_expr(a, true)?)))
            }
            // 型の大きさは、定義が見えていればこの段階で決まるので数値にする(LP64)
            CExprKind::SizeofType(t) => match self.size_align(t) {
                Some((size, _)) => ExprNode::Number(size.to_string()),
                None => {
                    return unsup(
                        line,
                        "sizeof(大きさを決められない型: 定義が見えない構造体・void・関数型など)",
                    );
                }
            },
            CExprKind::SizeofExpr(_) => {
                return unsup(
                    line,
                    "sizeof(式)(式の型はこの段階では決まらない。`sizeof(型)`は使える)",
                );
            }
            CExprKind::Comma(_) => {
                return unsup(
                    line,
                    "式の中のカンマ演算子(文・`for`の初期化/更新式の中だけ分解して対応)",
                );
            }
        };
        Ok((expr_at(r, line, &self.self_file_name), false))
    }

    fn lower_unary(
        &mut self,
        op: UnOp,
        a: &CExpr,
        used: bool,
        line: usize,
    ) -> PResult<(Expr, bool)> {
        Ok(match op {
            UnOp::Plus => (self.lower_expr(a, true)?, false),
            UnOp::Neg => match &a.kind {
                CExprKind::Int(s) => (
                    expr_at(
                        ExprNode::Number(format!("-{}", s)),
                        line,
                        &self.self_file_name,
                    ),
                    false,
                ),
                CExprKind::Float(s) => (
                    expr_at(
                        ExprNode::Float(format!("-{}", s)),
                        line,
                        &self.self_file_name,
                    ),
                    false,
                ),
                _ => (
                    expr_at(
                        ExprNode::Sub(Expr::wrap(
                            expr_at(ExprNode::Number("0".into()), line, &self.self_file_name),
                            self.lower_expr(a, true)?,
                        )),
                        line,
                        &self.self_file_name,
                    ),
                    false,
                ),
            },
            UnOp::Not => {
                let (x, is_bool) = self.lower_expr_ex(a, true)?;
                (negate(x, is_bool, line, &self.self_file_name), true)
            }
            UnOp::BitNot => (
                expr_at(
                    ExprNode::XorCalcul(Expr::wrap(
                        self.lower_expr(a, true)?,
                        expr_at(ExprNode::Number("-1".into()), line, &self.self_file_name),
                    )),
                    line,
                    &self.self_file_name,
                ),
                false,
            ),
            UnOp::Deref => (
                expr_at(
                    ExprNode::ConnectAddr(Box::new(self.lower_expr(a, true)?)),
                    line,
                    &self.self_file_name,
                ),
                false,
            ),
            UnOp::Addr => (
                expr_at(
                    ExprNode::GetAddress(Box::new(self.lower_expr(a, true)?)),
                    line,
                    &self.self_file_name,
                ),
                false,
            ),
            UnOp::PreInc => (
                expr_at(
                    ExprNode::Inc(Box::new(self.lower_expr(a, true)?)),
                    line,
                    &self.self_file_name,
                ),
                false,
            ),
            UnOp::PreDec => (
                expr_at(
                    ExprNode::Dec(Box::new(self.lower_expr(a, true)?)),
                    line,
                    &self.self_file_name,
                ),
                false,
            ),
            UnOp::PostInc | UnOp::PostDec => {
                if used {
                    return unsup(
                        line,
                        "値を使っている後置`++`/`--`(nodeのInc/Decは前置と後置を区別できない)",
                    );
                }
                let x = Box::new(self.lower_expr(a, true)?);
                (
                    if op == UnOp::PostInc {
                        expr_at(ExprNode::Inc(x), line, &self.self_file_name)
                    } else {
                        expr_at(ExprNode::Dec(x), line, &self.self_file_name)
                    },
                    false,
                )
            }
        })
    }

    fn lower_binary(
        &mut self,
        op: BinOp,
        a: &CExpr,
        b: &CExpr,
        line: usize,
    ) -> PResult<(Expr, bool)> {
        match op {
            BinOp::LogAnd | BinOp::LogOr => {
                let l = self.lower_cond(a)?;
                let r = self.lower_cond(b)?;
                let e = if is_pure(b) {
                    // 右辺に副作用も異常終了もないので、短絡評価しなくても結果は同じ
                    let pair = Expr::wrap(l, r);
                    if op == BinOp::LogAnd {
                        expr_at(ExprNode::AndCalcul(pair), line, &self.self_file_name)
                    } else {
                        expr_at(ExprNode::OrCalcul(pair), line, &self.self_file_name)
                    }
                } else {
                    // 短絡評価を保つため、Matchで書く(`a && b` = `a ? b : 0`)
                    let (then_v, else_v) = if op == BinOp::LogAnd {
                        (
                            r,
                            expr_at(ExprNode::Number("0".into()), line, &self.self_file_name),
                        )
                    } else {
                        (
                            expr_at(ExprNode::Number("1".into()), line, &self.self_file_name),
                            r,
                        )
                    };
                    let tb = vec![self.info(line, Group2Node::Expr(then_v))];
                    let eb = vec![self.info(line, Group2Node::Expr(else_v))];
                    expr_at(
                        ExprNode::Match {
                            pattern: None,
                            arms: vec![MatchArm {
                                pattern: Box::new(l),
                                body: tb,
                                span: span_at(line, &self.self_file_name),
                            }],
                            arm_else: Some(eb),
                        },
                        line,
                        &self.self_file_name,
                    )
                };
                Ok((e, true))
            }
            _ => {
                let (x, y) = self.bin(a, b)?;
                let is_bool = matches!(
                    op,
                    BinOp::Lt | BinOp::Gt | BinOp::Le | BinOp::Ge | BinOp::Eq | BinOp::Ne
                );
                Ok((make_bin(op, x, y, line, &self.self_file_name), is_bool))
            }
        }
    }

    /// `a.b` / `p->b` / `a[i].b` / `(*p).b` を`Member`/`PtrMember`にする
    fn lower_member(&mut self, e: &CExpr) -> PResult<Expr> {
        let line = e.line;
        // 外側から`.`を辿って、メンバー名を集める
        let mut fields: Vec<String> = Vec::new();
        let mut cur = e;
        let mut arrow_base: Option<&CExpr> = None;
        let dot_base: &CExpr;
        loop {
            match &cur.kind {
                CExprKind::Member(inner, m, false) => {
                    fields.push(m.clone());
                    cur = &**inner;
                }
                CExprKind::Member(inner, m, true) => {
                    fields.push(m.clone());
                    arrow_base = Some(&**inner);
                    dot_base = &**inner;
                    break;
                }
                _ => {
                    dot_base = cur;
                    break;
                }
            }
        }
        fields.reverse();
        let target = |fields: &[String], file_name: &Rc<String>| -> Expr {
            if fields.len() == 1 {
                expr_at(ExprNode::Var(fields[0].clone()), line, &file_name)
            } else {
                expr_at(
                    ExprNode::Member {
                        scope: fields[..fields.len() - 1].to_vec(),
                        target: Box::new(expr_at(
                            ExprNode::Var(fields[fields.len() - 1].clone()),
                            line,
                            &file_name,
                        )),
                    },
                    line,
                    &file_name,
                )
            }
        };
        if arrow_base.is_some() {
            // `p->m...`: pは変数名
            return match &dot_base.kind {
                CExprKind::Ident(p) => Ok(expr_at(
                    ExprNode::PtrMember {
                        name: self.resolve(p),
                        index: None,
                        else_idx: None,
                        target: Box::new(target(&fields, &self.self_file_name)),
                    },
                    line,
                    &self.self_file_name,
                )),
                CExprKind::Unary(UnOp::Deref, _) => {
                    unsup(line, "`(*pp)->m`(ポインタのポインタ経由のメンバー参照)")
                }
                _ => unsup(
                    line,
                    "変数名以外を左辺に持つ`->`(`p->next->val`のような連鎖、関数の戻り値、配列要素のポインタなど。PtrMemberは変数名しか持てない)",
                ),
            };
        }
        match &dot_base.kind {
            CExprKind::Ident(s) => {
                let mut scope = vec![self.resolve(s)];
                scope.extend_from_slice(&fields[..fields.len() - 1]);
                Ok(expr_at(
                    ExprNode::Member {
                        scope,
                        target: Box::new(expr_at(
                            ExprNode::Var(fields[fields.len() - 1].clone()),
                            line,
                            &self.self_file_name,
                        )),
                    },
                    line,
                    &self.self_file_name,
                ))
            }
            // `a[i].m`
            CExprKind::Index(base, idx) => match &base.kind {
                CExprKind::Ident(p) => Ok(expr_at(
                    ExprNode::PtrMember {
                        name: self.resolve(p),
                        index: Some(Box::new(self.lower_expr(idx, true)?)),
                        else_idx: None,
                        target: Box::new(target(&fields, &self.self_file_name)),
                    },
                    line,
                    &self.self_file_name,
                )),
                _ => unsup(
                    line,
                    "変数名以外を添字アクセスしてからメンバーを参照する式(`a[i][j].m`など)",
                ),
            },
            // `(*p).m`
            CExprKind::Unary(UnOp::Deref, inner) => match &inner.kind {
                CExprKind::Ident(p) => Ok(expr_at(
                    ExprNode::PtrMember {
                        name: self.resolve(p),
                        index: None,
                        else_idx: None,
                        target: Box::new(target(&fields, &self.self_file_name)),
                    },
                    line,
                    &self.self_file_name,
                )),
                _ => unsup(line, "`(*式).m`(変数名以外のポインタ)"),
            },
            _ => unsup(
                line,
                "変数名・配列要素・`*p`以外にある`.`(関数の戻り値や複合リテラルのメンバー参照)",
            ),
        }
    }

    /// 代入先の根になっている変数名
    fn root_name(&self, e: &CExpr) -> String {
        match &e.kind {
            CExprKind::Ident(n) => self.resolve(n),
            CExprKind::Index(b, _) => self.root_name(b),
            CExprKind::Member(b, _, _) => self.root_name(b),
            CExprKind::Unary(UnOp::Deref, b) => self.root_name(b),
            _ => String::new(),
        }
    }
}

// -------------------------------------------------------------------
// 補助関数
// -------------------------------------------------------------------

fn type_name(t: &CType) -> String {
    match t {
        CType::Named { name, .. } => name.clone(),
        _ => String::new(),
    }
}

/// `return;`に使う値。nodeのReturnは必ず式を取るので0で代用する(NOTE.md)
fn void_value(line: usize, self_file_name: &Rc<String>) -> Expr {
    expr_at(ExprNode::Number("0".to_string()), line, self_file_name)
}

fn ends_with_bare_return(body: &[CStmt]) -> bool {
    matches!(
        body.last(),
        Some(CStmt {
            kind: CStmtKind::Return(None),
            ..
        })
    )
}

fn is_const_true(e: &CExpr) -> bool {
    matches!(&e.kind, CExprKind::Int(s) if s != "0")
        || matches!(&e.kind, CExprKind::Char(v) if *v != 0)
}

fn make_bin(
    op: BinOp,
    a: Box<Expr>,
    b: Box<Expr>,
    line: usize,
    self_file_name: &Rc<String>,
) -> Expr {
    let p = (a, b);
    let node = match op {
        BinOp::Mul => ExprNode::Mul(p),
        BinOp::Div => ExprNode::Div(p),
        BinOp::Mod => ExprNode::Surplus(p),
        BinOp::Add => ExprNode::Add(p),
        BinOp::Sub => ExprNode::Sub(p),
        BinOp::Shl => ExprNode::LShift(p),
        BinOp::Shr => ExprNode::RShift(p),
        BinOp::Lt => ExprNode::LessThen(p),
        BinOp::Gt => ExprNode::GreaterThen(p),
        BinOp::Le => ExprNode::LessThenEq(p),
        BinOp::Ge => ExprNode::GreaterThenEq(p),
        BinOp::Eq => ExprNode::Equal(p),
        BinOp::Ne => ExprNode::NotEq(p),
        BinOp::BitAnd => ExprNode::AndCalcul(p),
        BinOp::BitXor => ExprNode::XorCalcul(p),
        BinOp::BitOr => ExprNode::OrCalcul(p),
        BinOp::LogAnd => ExprNode::CmpAnd(p),
        BinOp::LogOr => ExprNode::CmpOr(p),
    };
    expr_at(node, line, self_file_name)
}

/// 値を0/1にする(既に0/1ならそのまま)
fn to_bool(e: Expr, is_bool: bool, line: usize, self_file_name: &Rc<String>) -> Expr {
    if is_bool {
        e
    } else {
        expr_at(
            ExprNode::NotEq(Expr::wrap(
                e,
                expr_at(ExprNode::Number("0".to_string()), line, self_file_name),
            )),
            line,
            self_file_name,
        )
    }
}

/// 条件の否定
fn negate(e: Expr, is_bool: bool, line: usize, self_file_name: &Rc<String>) -> Expr {
    let node = match e.node {
        ExprNode::Equal(p) => ExprNode::NotEq(p),
        ExprNode::NotEq(p) => ExprNode::Equal(p),
        ExprNode::LessThen(p) => ExprNode::GreaterThenEq(p),
        ExprNode::GreaterThenEq(p) => ExprNode::LessThen(p),
        ExprNode::GreaterThen(p) => ExprNode::LessThenEq(p),
        ExprNode::LessThenEq(p) => ExprNode::GreaterThen(p),
        other => {
            let _ = is_bool;
            ExprNode::Equal(Expr::wrap(
                Expr::new(other, e.span),
                expr_at(ExprNode::Number("0".to_string()), line, &self_file_name),
            ))
        }
    };
    expr_at(node, line, &self_file_name)
}

/// 評価しても副作用(代入・関数呼び出し・++/--)が無い
fn side_effect_free(e: &CExpr) -> bool {
    match &e.kind {
        CExprKind::Ident(_)
        | CExprKind::EnumConst { .. }
        | CExprKind::Int(_)
        | CExprKind::Float(_)
        | CExprKind::Char(_)
        | CExprKind::Str(_)
        | CExprKind::SizeofType(_) => true,
        CExprKind::Unary(op, a) => {
            !matches!(
                op,
                UnOp::PreInc | UnOp::PreDec | UnOp::PostInc | UnOp::PostDec
            ) && side_effect_free(a)
        }
        CExprKind::Binary(_, a, b) | CExprKind::Index(a, b) => {
            side_effect_free(a) && side_effect_free(b)
        }
        CExprKind::Cond(a, b, c) => {
            side_effect_free(a) && side_effect_free(b) && side_effect_free(c)
        }
        CExprKind::Member(a, _, _) | CExprKind::Cast(_, a) | CExprKind::SizeofExpr(a) => {
            side_effect_free(a)
        }
        CExprKind::Assign(..) | CExprKind::Call(..) | CExprKind::Comma(_) => false,
    }
}

/// 副作用も、メモリ参照・0除算による異常終了も無い(短絡評価を省いても結果が変わらない)
fn is_pure(e: &CExpr) -> bool {
    match &e.kind {
        CExprKind::Ident(_)
        | CExprKind::EnumConst { .. }
        | CExprKind::Int(_)
        | CExprKind::Float(_)
        | CExprKind::Char(_)
        | CExprKind::Str(_)
        | CExprKind::SizeofType(_) => true,
        CExprKind::Unary(op, a) => {
            matches!(
                op,
                UnOp::Plus | UnOp::Neg | UnOp::Not | UnOp::BitNot | UnOp::Addr
            ) && is_pure(a)
        }
        CExprKind::Binary(op, a, b) => {
            !matches!(op, BinOp::Div | BinOp::Mod) && is_pure(a) && is_pure(b)
        }
        CExprKind::Cond(a, b, c) => is_pure(a) && is_pure(b) && is_pure(c),
        CExprKind::Cast(_, a) => is_pure(a),
        CExprKind::Member(a, _, false) => is_pure(a),
        _ => false,
    }
}

/// 末尾の`break`を外す。戻り値の`bool`は、この本体が次のcaseへ落ちずに終わるか
fn strip_trailing_break<'a>(body: &[&'a CStmt]) -> (Vec<&'a CStmt>, bool) {
    let mut v: Vec<&'a CStmt> = body.to_vec();
    match v.last().map(|s| &s.kind) {
        Some(CStmtKind::Break) => {
            v.pop();
            (v, true)
        }
        Some(CStmtKind::Return(_)) | Some(CStmtKind::Continue) | Some(CStmtKind::Goto(_)) => {
            (v, true)
        }
        _ => (v, false),
    }
}

/// `int a[] = {1,2,3}` / `char s[] = "abc"` の大きさを決める
fn fix_array_len(ty: &CType, init: Option<&CInit>) -> CType {
    match (ty, init) {
        (CType::Array { inner, len: None }, Some(CInit::List(items))) => CType::Array {
            inner: inner.clone(),
            len: Some(items.len()),
        },
        (
            CType::Array { inner, len: None },
            Some(CInit::Expr(CExpr {
                kind: CExprKind::Str(s),
                ..
            })),
        ) => CType::Array {
            inner: inner.clone(),
            len: Some(s.chars().count() + 1),
        },
        _ => ty.clone(),
    }
}
