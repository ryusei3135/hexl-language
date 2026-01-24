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
            type_info::VarValue, //  反復処理の繰り返すcount
            Option<type_info::VarValue>, //  ループ変数
        )
    >;

pub struct CondStatus {
    pub status: Vec<(bool, i32, LoopStatus)>,
}

/// 条件分岐に関する処理
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

    pub fn cond_true(&mut self) {
        if let Some((flag, _value, _loop)) = self.status.last_mut() {
            *flag = true;   // ← list[last].0 を変更
        }
    }

    pub fn del(&mut self) {
        self.status.pop().unwrap();
    }
}

/// 反復処理に関する関数
impl CondStatus {
    /// for文のみ
    pub fn get_loop_var(&self) -> type_info::VarValue {
        self.status.last().unwrap().2.clone().unwrap().2.unwrap()
    }
    ///  反復処理がこれから始まることを設定
    pub fn now_loop(
        &mut self,
        cond_location: Option<u32>,
        loop_count: Option<type_info::VarValue>,
    ) -> Result<u32, control_syn::ControlSynErr> {

        let status = self.status.last_mut()
            .ok_or(control_syn::ControlSynErr::DATA_IS_NOT_FOUND)?;

        // 設定フェーズ
        if cond_location.is_some() && loop_count.is_some() {
            status.2 = Some((
                cond_location.unwrap(),
                loop_count.unwrap(),
                None
            ));
            return Err(control_syn::ControlSynErr::SETTING);
        }

        let loop_status = status.2.as_mut()
            .ok_or(control_syn::ControlSynErr::INVALID_ITER_COND)?;

        // 実行フェーズ
        if !is_not_zero(loop_status.1.clone())
            .unwrap_or(false)
        {
            self.status.pop();
            return Err(control_syn::ControlSynErr::END_LOOP);
        }

        match dec_and_get_item(loop_status.1.clone()) {
            Ok(result) => {
                loop_status.1 = result.clone();
                loop_status.2 = Some(result);
                Ok(loop_status.0)
            }
            Err(e) => Err(e),
        }
    }

}
