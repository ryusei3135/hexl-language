use super::*;

///  条件分岐などの現在の式が何かを代入
static CONTROL_SYN_FLAG: OnceLock<Mutex<node::NodeKind>> = OnceLock::new();


pub fn control_syn_flag(kind: node::NodeKind) {
    *CONTROL_SYN_FLAG
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap() = kind;
}

pub fn get_control_syn_flag() -> node::NodeKind {
    CONTROL_SYN_FLAG
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap()
        .clone()
}

///  反復処理の情報
#[derive(Clone)]
pub struct LoopStatus {
    for_start: usize,             // 反復処理の条件がある場所
    status: Option<IterStatus>,          // 反復処理の情
}

enum SynFlag {
    Cond {
        executed_flag: bool,
        my_block: usize,
    },
    For {
        status: Option<IterStatus>,
        for_start: Option<usize>,
        my_block: usize,
    },
}

pub struct ControlSynFlag {
    status: Vec<SynFlag>,
}

/// 条件分岐に関する処理
impl ControlSynFlag {
    pub fn new() -> Self {
        Self {
            status: Vec::<SynFlag>::new()
        }
    }

    pub fn make_new_flag(
            &mut self,
            cond: bool,
            block: usize,
            flag: node::NodeKind
    ) {
        match flag {
            node::NodeKind::NodeIf => {
                self.status.push(
                    SynFlag::Cond {
                        executed_flag: cond,
                        my_block: block
                    }
                );
            }
            node::NodeKind::NodeFor => {
                self.status.push(
                    SynFlag::For {
                        status: None,
                        for_start: None,
                        my_block: block,
                    }
                );
            }
            _ => panic!("[system err]: An invalid flag was assigned when setting control syntax flags."),
        }
    }
    /// 今の条件分岐の条件がtrueのほかに、前回までに、trueになった場所がないか、
    /// 同じブロック内にいるかを確認し、booleanを返す
    pub fn judge_cond(&self, now_block: usize) -> bool {
        //  エリアが同じかつ、今までの条件で、trueになってなければ、今の条件を実行可能
        match self.status.last().unwrap() {
            SynFlag::Cond { executed_flag, my_block } => *my_block == now_block && !executed_flag,
            _ => panic!("don't if flag"),
        }
    }

    /// if else文がtrueになった際に、下に続いている条件分岐を実行しないようにする
    pub fn cond_status_true(&mut self) {
        match self.status.last().unwrap() {
            SynFlag::Cond { mut executed_flag, my_block } => executed_flag = true,
            _ => panic!("don't if flag"),
        }
    }
    /// 現在のフラグを削除
    pub fn del(&mut self) {
        self.status.pop().unwrap();
    }

    pub fn get_now_flag(&self) -> Option<node::NodeKind> {
        if let Some(flags) = self.status.last() {
            Some(
                match flags {
                    SynFlag::Cond {..} => node::NodeKind::NodeIf,
                    SynFlag::For {..}=> node::NodeKind::NodeFor,
                }
            )
        } else {
            None
        }
    }
}

/// # 引数
/// - for_loop_status
///     現在のｆｏｒ文の情報がある変数
/// - loop_cond
///     ｆｏｒ文の条件のノード初期化以外のときは、None
/// # 戻り値
/// - Ok
///     更新されたfor文の情報
/// - Err
///     for文の情報を更新する際に起こったエラーの情報
fn running_for_loop(
        for_loop_status: Option<IterStatus>,
        loop_cond: Option<node::CalculNode>,
) -> Result<IterStatus, control_syn::ControlSynErr> {
    let status = is_for_iterable(loop_cond, for_loop_status)?;

    if status.executable {
        if let control_info::ControlSemantics::BindsVar(ref var_name) = status.var_setting {
            var_manager().add_var(
                var_name.clone(),
                status.loop_var.clone(),
                VarRegion::Stack,
            );
        }
        Ok(status)
    } else {
        Err(control_syn::ControlSynErr::EndLoop)
    }
}

/// 反復処理に関する関数
impl ControlSynFlag {
    /// for文の初期化やループ変数などの更新などをする関数
    /// # 引数
    /// - iter_cond_node
    ///     for文の条件のノード
    ///     すでに条件が設定された場合はNoneを代入すること
    /// # 戻り値
    /// - もしfor文を初期化した場合、初期化したことを表す
    /// - SETTINGを返す
    /// - ふつうはｆｏｒ文の条件がある行を返す
    fn update_loop_var(
            &mut self,
            iter_cond_node: Option<node::CalculNode>,
    ) -> Result<usize, control_syn::ControlSynErr> {
        match self.status.last_mut().ok_or(control_syn::ControlSynErr::DataIsNotFound)? {
            SynFlag::For { status, for_start, my_block } => {
                return match running_for_loop(status.clone(), iter_cond_node.clone()) {
                    Ok(result) => {
                        // ループ変数を更新
                        *status = Some(result);

                        if iter_cond_node.is_none() {
                            Ok(for_start.unwrap())
                        } else {
                            Err(control_syn::ControlSynErr::SETTING)
                        }
                    }
                    Err(e) => {
                        Err(e)
                    }
                };
            }
            _ => panic!("JH"),
        }
        Err(control_syn::ControlSynErr::DataIsNotFound)
    }

    ///  反復処理がこれから始まることを設定
    pub fn now_loop(
        &mut self,
        cond_location: Option<usize>,
        iter_cond_node: Option<node::CalculNode>,
    ) -> Result<usize, control_syn::ControlSynErr> {
        // 設定フェーズ
        return if cond_location.is_some() && iter_cond_node.is_some() {
            match self.status.last_mut().ok_or(control_syn::ControlSynErr::DataIsNotFound)? {
                SynFlag::For { status, for_start, my_block } => {
                    *for_start = Some(cond_location.unwrap());
                },
                _ => panic!("JJJ"),
            }

            self.update_loop_var(iter_cond_node.clone())
        } else {
            // 実行フェーズ
            self.update_loop_var(iter_cond_node.clone())
        };
    }
}
