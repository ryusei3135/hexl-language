//! 関数の定義を引くための木(`FuncTree`)と、関数の定義情報(`FnDefInfo`/`FnDefMetaData`)
use super::*;

#[derive(Clone, Debug, PartialEq)]
pub struct FnDefInfo {
    pub name: String,
    pub module: Option<String>,
    pub args: Vec<ArgsNode>,
    pub body: Vec<inst::Inst>,
    pub ret_ty: Option<TyNode>,
    /// IRの型定義から解決したアセンブリ上の戻り値サイズ。構造体戻り値では`None`。
    ret_size: Option<Size>,
    pub temp_ty: Vec<TyNode>,
    pub var_counter: counter_mnger::VarCounter,
    pub public: bool,
    pub stk_size: usize,
    /// 署名だけの登録(本体が無い)。`FuncTree::declare`が作った後、
    /// 本体のIRが`add`で登録されなかった関数(`#include`した型のメゾットなど)。
    /// アセンブリ言語は生成せず、呼び出しだけを生成する(`asm_gen`)
    pub is_declaration: bool,
}

impl FnDefInfo {
    pub fn new(
        name: String,
        module: Option<String>,
        args: Vec<ArgsNode>,
        body: Vec<inst::Inst>,
        ret_ty: Option<TyNode>,
        temp_ty: Vec<TyNode>,
        public: bool,
        stk_size: usize,
        var_counter: counter_mnger::VarCounter,
    ) -> Self {
        let ret_size = ret_ty.as_ref().and_then(|ty| match ty {
            TyNode::SelfTy(..) => None,
            TyNode::Ty(name) if !types::Size::is_builtin_ty_name(name) => None,
            ty => ty.try_into().ok(),
        });
        Self {
            name,
            module,
            args,
            body,
            ret_ty,
            ret_size,
            temp_ty,
            var_counter,
            public,
            stk_size,
            is_declaration: false,
        }
    }
    /// 第一引数が`Self`型、または`Self`を指すポインタ型
    /// (`self: Self` / `self: Self*` / `self: Self*mut`)かどうかを判定する
    pub fn first_param_is_self(&self) -> bool {
        self.args.get(0).is_some_and(|f| match &f.ty {
            TyNode::SelfTy(..) => true,
            TyNode::Pointer { ty_name, .. } => {
                matches!(**ty_name, TyNode::SelfTy(..))
            }
            _ => false,
        })
    }

    pub fn get_ret_ty(&self) -> Option<Size> {
        if self.ret_ty.is_none() {
            return types::Size::Void.wrap_dst_size();
        }
        self.ret_size.clone()
    }

    fn with_declaration(mut self, is_declaration: bool) -> Self {
        self.is_declaration = is_declaration;
        self
    }

    fn with_ret_size(mut self, ret_size: Option<Size>) -> Self {
        self.ret_size = ret_size;
        self
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct FuncTree {
    pub func: HashMap<String, FnDefInfo>,
}

impl FuncTree {
    pub(in crate::compiler::ir) fn new() -> Self {
        Self {
            func: HashMap::new(),
        }
    }

    /// `func`ハッシュマップに登録する際のキーを生成する
    ///
    /// 構造体のメゾットを展開した関数など、モジュール名を持つ
    /// 関数は`モジュール名::関数名`をキーにする(`StructName::method`の
    /// ように同名のメゾットが別の構造体にあっても衝突しないようにする)。
    /// モジュール名を持たない、通常の(トップレベルの)関数は
    /// そのまま関数名だけをキーにする
    fn make_key(name: &str, module_name: Option<&str>, temp_ty: &[TyNode]) -> String {
        let base = match module_name {
            Some(module) => format!("{}::{}", module, name),
            None => name.to_owned(),
        };
        if temp_ty.is_empty() {
            base
        } else {
            format!("{}::<{:?}>", base, temp_ty)
        }
    }

    pub(in crate::compiler::ir) fn get(
        &self,
        name: &str,
        mod_name: Option<&str>,
    ) -> Option<FnDefInfo> {
        let no_temp_ty: &[TyNode] = &[];
        self.get_with_temp(name, mod_name, no_temp_ty)
    }

    pub(in crate::compiler::ir) fn get_with_temp(
        &self,
        name: &str,
        module_name: Option<&str>,
        temp_ty: &[TyNode],
    ) -> Option<FnDefInfo> {
        self.func
            .get(&Self::make_key(name, module_name, temp_ty))
            .cloned()
    }

    pub(in crate::compiler::ir) fn add(
        &mut self,
        body: &[inst::Inst],
        meta_data: &FuncDefine,
        ret_ty: &TyNode,
        ret_size: Option<Size>,
        stk_size: usize,
        var_counter: &counter_mnger::VarCounter,
    ) {
        // 関数の名前
        let fn_name = &meta_data.name;
        // 関数が属しているモジュールのツリー
        let fn_mod = &meta_data.module;
        //  関数がジェネリクスの場合
        let fn_tmp_ty = &meta_data.temp_ty;
        let key = Self::make_key(&fn_name, fn_mod.as_ref().map(|x| x.as_str()), &fn_tmp_ty);
        self.func.insert(
            key,
            FnDefInfo::new(
                fn_name.clone(),
                // 構造体のメゾットとして展開された関数の場合、
                // 属している構造体の名前がここに入る
                fn_mod.to_owned(),
                meta_data.params.clone(),
                body.to_owned(),
                Some(meta_data.contract.as_ref().unwrap_or(ret_ty).to_owned()),
                fn_tmp_ty.to_owned(),
                meta_data.public,
                stk_size,
                var_counter.clone(),
            )
            .with_ret_size(ret_size),
        );
    }

    pub(in crate::compiler::ir) fn declare(&mut self, meta_data: &FuncDefine) {
        let key = Self::make_key(
            &meta_data.name,
            meta_data.module.as_ref().map(|x| x.as_str()),
            &meta_data.temp_ty,
        );
        self.func.entry(key).or_insert_with(|| {
            FnDefInfo::new(
                meta_data.name.clone(),
                meta_data.module.clone(),
                meta_data.params.clone(),
                Vec::new(),
                Some(
                    meta_data
                        .contract
                        .as_ref()
                        .unwrap_or(&meta_data.ret_ty)
                        .clone(),
                ),
                meta_data.temp_ty.clone(),
                meta_data.public,
                0,
                counter_mnger::VarCounter::new(),
            )
            .with_declaration(true)
        });
    }
}

#[derive(Clone, Debug)]
pub struct FnDefMetaData {
    module: Option<String>,
    pub name: String,
    params: Vec<ArgsNode>,
    ret_ty: Option<TyNode>,
    // `#include`で他のファイルから読み込む際に、公開(pub)された
    // 関数かどうかを判定するために使う
    pub public: bool,
}

impl FnDefMetaData {
    /// moduleは自分自身がどのモジュールに属しているか
    /// Noneの場合は、#includeで関数の名前ごと指定しているか
    /// 自分のファイルの中にあるかのどちらか
    pub(in crate::compiler::ir) fn new(info: &FuncDefine, module: Option<&String>) -> Self {
        Self {
            module: module.map(|v| v.clone()),
            name: info.name.clone(),
            params: info.params.clone(),
            ret_ty: Some(info.ret_ty.clone()),
            public: info.public,
        }
    }

    #[inline(always)]
    pub(in crate::compiler::ir) fn add_self_module_name(&mut self, self_name: &str) {
        self.module = Some(self_name.to_string());
    }

    /// この関数がどのモジュール名で登録されているかを返す
    /// (`#include`でモジュール名を指定せず取り込んだ関数は`None`)
    pub(in crate::compiler::ir) fn module(&self) -> Option<&str> {
        self.module.as_ref().map(|x| x.as_str())
    }

    pub(in crate::compiler::ir) fn gen_fn_def(&self, stk_size: usize) -> FnDefInfo {
        FnDefInfo::new(
            self.name.clone(),
            None,
            self.params.clone(),
            Vec::new(),
            self.ret_ty.clone(),
            Vec::new(),
            true,
            stk_size,
            // 外部の関数の本体はこのファイルにないので、使用回数は無い
            // (処理中の関数のカウンターを渡すと、関数ごとの区切りが崩れる)
            counter_mnger::VarCounter::new(),
        )
    }
}
