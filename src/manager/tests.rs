use super::*;

#[cfg(test)]
mod var {
    use super::*;

    const RANGE: [&str; 5] = [
        "a",
        "b",
        "c",
        "d",
        "e",
    ];

    fn push_var(
            variable: &mut variable::VariableManager,
            name: &String,
            multiple: Option<variable::MultipleVar>,
    ) {
        variable.add_var(
            name,
            type_info::VarValue::Int32(10),
            &None,
            VarRegion::Stack,
            None
        ).unwrap();
    }

    fn make_test_stack_var(
            variables: &mut variable::VariableManager,
            i: usize,
    ) {
        for v in RANGE {
            push_var(variables, &(v.to_owned() + i.to_string().as_str()).to_string(), None);
        }
    }

    /// スタック領域だけ確認
    #[test]
    fn check_remove_stack() {
        let mut variables = variable::VariableManager::new();
        variables.make_scope();

        for i in 0..3 {
            variables.make_new_stack();
            make_test_stack_var(&mut variables, i);
        }
        assert_eq!(variables.variables_info_vec.len(), 15);
        assert_eq!(variables.region_stack_index.len(), 3);
        variables.remove_stack();
        assert_eq!(variables.variables_info_vec.len(), 10);
    }

    #[test]
    fn check_stack_and_static() {
        let mut variables = variable::VariableManager::new();
        variables.make_scope();

        for i in 0..3 {
            variables.make_new_stack();
            make_test_stack_var(&mut variables, i);
            push_var(
                &mut variables,
                &("k".to_owned() + i.to_string().as_str()).to_string(),
                None
            );
        }
        assert_eq!(variables.variables_info_vec.len(), 18);
        assert_eq!(variables.region_stack_index.len(), 3);
        variables.remove_stack();
        assert_eq!(variables.variables_info_vec.len(), 12);
    }

    #[test]
    fn check_move_scope() {
        let mut variables = variable::VariableManager::new();
        variables.make_scope();

        for i in 0..3 {
            variables.make_new_stack();
            make_test_stack_var(&mut variables, i);
        }
        variables.make_scope();
        for i in 3..6 {
            variables.make_new_stack();
            make_test_stack_var(&mut variables, i);
        }
        assert_eq!(variables.local_scope.last().unwrap().len(), 3);
        variables.remove_scope();
        assert_eq!(variables.local_scope.last().unwrap().len(), 3);
        assert_eq!(variables.region_stack_index.len(), 3);
    }

    #[test]
    fn check_imm_var_err() {
         let mut var_info = variable::VariableManager::new();

         var_info.make_scope();
         var_info.make_new_stack();

         push_var(&mut var_info, &"a".to_string(), Some(variable::MultipleVar::IsImm));
         push_var(&mut var_info, &"b".to_string(), None);
         //  不変変数が不変か、調べる
         assert_eq!(
             var_info.update_var(&"a".to_string(), &type_info::VarValue::Int32(1)),
             Err(err_kind::ErrorsKind::AssignmentToImmutableVariable),
         );
         assert_eq!(
             var_info.update_var(&"b".to_string(), &type_info::VarValue::Int32(1)),
             Err(err_kind::ErrorsKind::AssignmentToImmutableVariable),
         );
    }
}
