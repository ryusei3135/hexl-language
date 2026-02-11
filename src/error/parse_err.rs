//! parseで発生した例外を列挙型にまとめる

#[derive(Clone, Debug, PartialEq)]
pub enum ParseErrs {
    TypeSpecUnspecified,
    TypeSpecAngleBracketForbid,
    VarMissingAssignmentTarget,
    VarMultipleVariableNames,
    VarMissingVarNameAfterType,
    MissingCommaBetweenArguments,
}

impl ParseErrs {
    pub fn print_log(&self, line: &usize) {
        println!("line -> {}", line);
        match self {
            ParseErrs::TypeSpecUnspecified => println!("[parse err]: \"type\" of \"<type>\" is not specified when defining the variable type"),
            ParseErrs::TypeSpecAngleBracketForbid => println!("[parse err]: cannot use the tokens \"<\" or \">\" within \"<>\" in a variable definition \"<type>\""),
            ParseErrs::VarMissingAssignmentTarget => println!("[parse err]: When assigning a value to a variable, the variable name is not specified."),
            ParseErrs::VarMultipleVariableNames => println!("[parse err]: The expression that defines the variable has two names"),
            ParseErrs::VarMissingVarNameAfterType => println!("[parse err]: There is no target next to the type"),
            ParseErrs::MissingCommaBetweenArguments => println!("[parse err]: When assigning two or more arguments, they are not separated by \",\""),
        }
    }
}
