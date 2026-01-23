use super::*;

///  条件分岐などの現在の式が何かを代入
static PROCESS_KIND: OnceLock<Mutex<node::NodeKind>> = OnceLock::new();


pub fn process_kind(kind: node::NodeKind) {
    *PROCESS_KIND
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap() = kind;
}

pub fn get_process_kind() -> node::NodeKind {
    PROCESS_KIND
        .get_or_init(|| Mutex::new(node::NodeKind::NodeNull))
        .lock()
        .unwrap()
        .clone()
}

///  反復処理の情報
type LoopStatus =
    Option< // 反復処理に関すること
        (
            u32,  //  反復処理の条件がある場所
            type_info::VarValue //  反復処理の繰り返すcount
        )
    >;

pub struct CondStatus {
    pub status: Vec<(bool, i32, LoopStatus)>,
}

impl CondStatus {
    pub fn new() -> Self {
        Self {
            status: Vec::<
                (
                    bool,   // 条件分岐ですでにtrueになっていないか
                    i32,    // 条件分岐のブロックがある場所
                    LoopStatus
                )
            >::new()
        }
    }

    pub fn push(&mut self, cond: bool, block: i32) {
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
    pub fn judge_cond(&self, now_block: i32) -> bool {
        //  エリアが同じかつ、今までの条件で、trueになってなければ、今の条件を実行可能
        self.status.last().unwrap().1 == now_block && !self.status.last().unwrap().0
    }
    ///  反復処理がこれから始まることを設定
    pub fn now_loop(
            &mut self,
            // 反復処理の条件がある場所
            cond_location: Option<u32>,
            loop_count: Option<type_info::VarValue>
    ) -> Result<u32, control_syn::ControlSynErr> {
        /// 条件分岐や反復処理のデータがあれば
        if !self.status.is_empty() {
            let loop_status: &mut LoopStatus
                = &mut self.status.last_mut().unwrap().2;

            if cond_location.is_none() || loop_count.is_none() {
                /// このブロック内で、Noneを返した場合、今の反復処理のデータは破棄する
                ///  繰り返すloop_countが0でなければ、反復処理の条件がある場所を返す
                if let Some(count_result) = is_not_zero(loop_status.clone().unwrap().1) {
                    if count_result {
                        return Ok(loop_status.clone().unwrap().0);
                    } else {
                        /// 反復処理が終わったので、データを破棄、反復処理が終了したことを返す
                        self.status.pop();
                        return Err(control_syn::ControlSynErr::END_LOOP);
                    }
                } else {
                    /// そもそも反復処理の条件が無効なので、データを破棄し、エラーを返す
                    self.status.pop();
                    return Err(control_syn::ControlSynErr::INVALID_ITER_COND);
                }
            } else {
                // 反復処理のデータを設定
                *loop_status = Some((cond_location.unwrap(), loop_count.unwrap()));
                return Err(control_syn::ControlSynErr::SETTING);
            }
        }
        ///  条件分岐や反復処理のデータがないので、Noneを返す
        Err(control_syn::ControlSynErr::DATA_IS_NOT_FOUND)
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
