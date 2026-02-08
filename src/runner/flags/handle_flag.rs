//! 制御構文のフラグをもとに新しく一時的なデータを作成し
//! それをもとに実行結果を変える
use super::*;

/// 制御構文で使うフラグ
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

/// この構造体で、フラグを元に実行結果を制御
pub struct ControlSynFlag {
    status: Vec<SynFlag>,
}

/// 条件分岐に関する処理
impl ControlSynFlag {
    pub fn new() -> Self {
        Self {
            status: Vec::<SynFlag>::new(),
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
        match self.status.last_mut().unwrap() {
            SynFlag::Cond { executed_flag, my_block: _ } => *executed_flag = true,
            _ => panic!("don't if flag"),
        }
    }
    /// 現在のフラグを削除
    pub fn del(&mut self) {
        self.status.pop().unwrap();
    }
    /// 現在のフラグを返す
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
///     ｆｏｒ文のノード初期化以外のときは、None
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
            SynFlag::For { status, for_start, my_block: _ } => {
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
            _ => Err(control_syn::ControlSynErr::DataIsNotFound),
        }
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
                SynFlag::For { status: _, for_start, my_block: _ } => {
                    *for_start = Some(cond_location.unwrap());
                },
                _ => panic!("JJJ for eval"),
            }

            self.update_loop_var(iter_cond_node.clone())
        } else {
            // 実行フェーズ
            self.update_loop_var(iter_cond_node.clone())
        };
    }
}
