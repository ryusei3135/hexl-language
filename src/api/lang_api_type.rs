use super::*;

/// 反復処理(for文)の今の状態を返す戻り値の型
/// ControlSynErr はerror/control_syn.rsを参照
/// ControlSemantics はrunner/control_info.rsを参照
#[derive(Clone)]
pub struct IterStatus {
    pub executable: bool,// 反復処理が実行可能か
    pub loop_var: type_info::VarValue,// 反復処理(for)のループ変数に代入する値
    pub var_setting: ControlSemantics,// loop変数に値を代入するかしないかの設定
    pub multiple: Option<variable::MultipleVar>,
    pub range: [type_info::VarValue; 2],
}

/// IterStatusを初期化する関数
/// # 引数
/// - range = 繰り返す範囲
/// - bind_var_name =
///     for文を使って反復処理をする際に
///     Someなら渡された変数に値を代入する
pub fn init_iter_status(
        mut range: [type_info::VarValue; 2],
        bind_var_name: Option<String>,
        multiple: Option<variable::MultipleVar>,
) -> Result<IterStatus, ControlSynErr> {
    Ok(
        IterStatus {
            executable: true,    // 最初は反復処理を実行可能なので、trueを代入
            loop_var: {
                if let type_info::VarValue::Array(arr) = range[0].clone() {
                    range[1] = type_info::VarValue::Int32(1);
                    *arr[0].clone()
                } else {
                    range[0].clone()
                }
            },
            var_setting: if let Some(name) = bind_var_name {
                ControlSemantics::BindsVar(name.clone())
            } else {
                ControlSemantics::NotBinds
            },
            multiple: multiple.clone(),
            range: range.into(),
        }
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn check_init_data(
            result: Result<IterStatus, ControlSynErr>,
            range: [type_info::VarValue; 2],
    ) -> Option<bool> {
        if let Ok(status) = result {
            assert_eq!(status.executable, true);
            assert_eq!(status.loop_var, range[0]);
            dbg!(status.loop_var);
            assert_eq!(status.range, range);
            dbg!(status.range);
            return Some(
                if let ControlSemantics::BindsVar(_) = status.var_setting {
                    true
                } else {
                    false
                }
            );
        }

        None
    }

    #[test]
    fn check_init_iter_status() {
        const RANGE: [type_info::VarValue; 2]
            = [type_info::VarValue::Int32(0), type_info::VarValue::Int32(5)];
        // 変数に代入あり
        assert_eq!(
            check_init_data(
                init_iter_status(RANGE,Some("test".to_string())),
                RANGE
            ).unwrap(),
            true
        );
        // 変数に代入なし
        assert_eq!(
            check_init_data(
                init_iter_status(RANGE, None),
                RANGE,
            ).unwrap(),
            false
        );
    }
}
