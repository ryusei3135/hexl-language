
common.rs:51:契約を終了するメソッド名の定義 CONTRACT_END_METHOD = "del"
common.rs:73-75:is_contract_end_call()。式が del の呼び出しかを判定します。
common.rs:169 付近:walk_expr 内の ExprNode::Member { scope, target } の分岐。a.del() の a を mark_used で終了済みにします。
loop_check.rs:157:ループ内の走査（scan_loop_expr）での、同じ a.del() の分岐