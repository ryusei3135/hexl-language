use super::*;

#[test]
fn counts_start_from_one_use() {
    let mut c = VarCounter::new();
    assert_eq!(c.var_use_count("a").0, 0);
    c.use_var("a");
    assert_eq!(c.var_use_count("a").0, 1);
    c.use_var("a");
    assert_eq!(c.var_use_count("a").0, 2);
}

#[test]
fn args_are_counted_by_index() {
    let mut c = VarCounter::with_arg_count(3);
    c.use_arg(1);
    c.use_arg(1);
    c.use_arg(2);
    assert_eq!(c.arg_use_count(0), 0);
    assert_eq!(c.arg_use_count(1), 2);
    assert_eq!(c.arg_use_count(2), 1);
    assert_eq!(c.args_by_priority(), vec![(1, 2), (2, 1), (0, 0)]);
}

#[test]
fn vars_by_priority_orders_by_count_then_name() {
    let mut c = VarCounter::new();
    c.use_var("b");
    c.use_var("a");
    c.use_var("c");
    c.use_var("c");
    assert_eq!(c.vars_by_priority(), vec![("c", 2), ("a", 1), ("b", 1)]);
}
