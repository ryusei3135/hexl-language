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
type LoopStatus =
    Option< // 反復処理に関すること
        (
            usize,  //  反復処理の条件がある場所
            node::CalculNode, //  反復処理の繰り返すcount
            Option<type_info::VarValue>, //  ループ変数
        )
    >;

pub struct ControlSynFlag {
    pub status: Vec<(bool, usize, LoopStatus)>,
}

/// 条件分岐に関する処理
impl ControlSynFlag {
    pub fn new() -> Self {
        Self {
            status: Vec::<
                (
                    bool,   // 条件分岐ですでにtrueになっていないか
                    usize,    // 条件分岐のブロックがある場所
                    LoopStatus
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
fn running_for_loop(loop_status: &mut LoopStatus) -> Result<(), control_syn::ControlSynErr> {
    match is_for_iterable(loop_status.clone().unwrap().1.clone(), &loop_status.clone().unwrap().2.clone()) {
        Ok(result) => {
            if result.0 {
                if let control_info::ControlSemantics::BindsVar(var_name) = result.2 {
                    var_manager().add_var(
                        var_name,
                        result.1.clone(),
                        VarRegion::Stack,
                    );
                }
                loop_status.as_mut().unwrap().2 = Some(result.1.clone());
                Ok(())
            } else {
                //  反復処理の終了
                // var_manager().remove_stack();
                Err(control_syn::ControlSynErr::EndLoop)
            }
        }
        Err(e) => {
            // var_manager().remove_stack();
            Err(e)
        }
    }
}

/// 反復処理に関する関数
impl ControlSynFlag {
    ///  反復処理がこれから始まることを設定
    pub fn now_loop(
        &mut self,
        cond_location: Option<usize>,
        loop_count: Option<node::CalculNode>,
    ) -> Result<usize, control_syn::ControlSynErr> {

        let status = self.status.last_mut().ok_or(control_syn::ControlSynErr::DataIsNotFound)?;

        // 設定フェーズ
        if cond_location.is_some() && loop_count.is_some() {
            status.2 = Some((
                cond_location.unwrap(),
                loop_count.unwrap(),
                None
            ));

            return Err(
                match running_for_loop(&mut status.2.clone()) {
                    Ok(_) => control_syn::ControlSynErr::SETTING,
                    Err(e) => {
                        self.status.pop();
                        e
                    }
                }
            );
        }

        // 実行フェーズ
        match running_for_loop(&mut status.2) {
            //  for文が続くので、条件がある場所を返す
            Ok(_) => Ok(status.2.clone().unwrap().0),
            Err(e) => {
                self.status.pop();
                Err(e)
            }
        }
    }
}
