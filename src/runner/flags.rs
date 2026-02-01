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

pub struct ControlSynFlag {
    pub status: Vec<(bool, usize, Option<LoopStatus>)>,
}

/// 条件分岐に関する処理
impl ControlSynFlag {
    pub fn new() -> Self {
        Self {
            status: Vec::<
                (
                    bool,   // 条件分岐ですでにtrueになっていないか
                    usize,    // 条件分岐のブロックがある場所
                    Option<LoopStatus>
                )
            >::new()
        }
    }

    pub fn push(&mut self, cond: bool, block: usize) {
        if self.status.len() > 0 {
            if self.status.last().unwrap().1 == block {
                if let Some((flag, _value, _is_loop)) = self.status.last_mut() {
                    *flag = cond;
                    return;
                }
            }
        }

        self.status.push((cond, block, None));
    }
    /// 今の条件分岐の条件がtrueのほかに、前回までに、trueになった場所がないか、
    /// 同じブロック内にいるかを確認し、booleanを返す
    pub fn judge_cond(&self, now_block: usize) -> bool {
        //  エリアが同じかつ、今までの条件で、trueになってなければ、今の条件を実行可能
        self.status.last().unwrap().1 == now_block && !self.status.last().unwrap().0
    }

    pub fn cond_true(&mut self) {
        if let Some((flag, _value, _loop)) = self.status.last_mut() {
            *flag = true;   // ← list[last].0 を変更
        }
    }

    pub fn del(&mut self) {
        self.status.pop().unwrap();
    }
}

/// for文のみ
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
    fn update_loop_var(
            &mut self,
            iter_cond_node: Option<node::CalculNode>,
    ) -> Result<usize, control_syn::ControlSynErr> {
        let status = self.status.last_mut().ok_or(control_syn::ControlSynErr::DataIsNotFound)?;

        match running_for_loop(status.2.clone().unwrap().status, iter_cond_node.clone()) {
            Ok(result) => {
                if let Some(loop_status) = status.2.as_mut() {
                    loop_status.status = Some(result);
                }
                if iter_cond_node.is_none() {
                    Ok(status.2.clone().unwrap().for_start)
                } else {
                    Err(control_syn::ControlSynErr::SETTING)
                }
            }
            Err(e) => {
                Err(e)
            }
        }
    }

    ///  反復処理がこれから始まることを設定
    pub fn now_loop(
        &mut self,
        cond_location: Option<usize>,
        iter_cond_node: Option<node::CalculNode>,
    ) -> Result<usize, control_syn::ControlSynErr> {

        let status = self.status.last_mut().ok_or(control_syn::ControlSynErr::DataIsNotFound)?;

        // 設定フェーズ
        return if cond_location.is_some() && iter_cond_node.is_some() {
            status.2 = Some(
                LoopStatus {
                    for_start: cond_location.unwrap(),
                    status: None,
                }
            );

            self.update_loop_var(iter_cond_node.clone())
        } else {
            // 実行フェーズ
            self.update_loop_var(iter_cond_node.clone())
        };
    }
}
