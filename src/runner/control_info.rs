
#[derive(Debug, PartialEq, Clone)]
pub enum ControlSemantics {
    BindsVar(String),
    NotBinds,
}
