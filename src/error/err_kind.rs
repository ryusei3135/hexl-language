//! parseで発生した例外を列挙型にまとめる

#[derive(Clone, Debug, PartialEq)]
pub enum ErrorsKind {
    TypeSpecUnspecified,
    TypeSpecAngleBracketForbid,
    VarMissingAssignmentTarget,
    VarMultipleVariableNames,
    VarMissingVarNameAfterType,
    MissingCommaBetweenArguments,
    // 変数を定義する際に、"mut"や"imm"が複数指定されている
    MultipleMutabilitySpecifiers,
    // 不変変数が変更されようとしている
    AssignmentToImmutableVariable,
    UndefinedVariable,
    VariableAlreadyDefined,
    UndefinedFunction,
    FunctionAlreadyDefined,
    AssignmentTypeMismatch,
    ReturnTypeMismatch,
}

impl ErrorsKind {
    pub fn print_log(&self, line: &usize, txt: &String) {
        println!("`{}`: line -> {}", txt, line);
        match self {
            ErrorsKind::TypeSpecUnspecified => println!("[parse err]: \"type\" of \"<type>\" is not specified when defining the variable type"),
            ErrorsKind::TypeSpecAngleBracketForbid => println!("[parse err]: cannot use the tokens \"<\" or \">\" within \"<>\" in a variable definition \"<type>\""),
            ErrorsKind::VarMissingAssignmentTarget => println!("[parse err]: When assigning a value to a variable, the variable name is not specified."),
            ErrorsKind::VarMultipleVariableNames => println!("[parse err]: The expression that defines the variable has two names"),
            ErrorsKind::VarMissingVarNameAfterType => println!("[syntax]: There is no target next to the type"),
            ErrorsKind::MissingCommaBetweenArguments => println!("[parse err]: When assigning two or more arguments, they are not separated by \",\""),
            ErrorsKind::MultipleMutabilitySpecifiers => println!("[syntax]: There are multiple ways to specify mutability when declaring variables."),
            ErrorsKind::AssignmentToImmutableVariable => println!("[syntax]: An immutable variable is about to be modified"),
            ErrorsKind::UndefinedVariable => println!("[name err]: undefined variable"),
            ErrorsKind::VariableAlreadyDefined => println!("[err]: variable already defined"),
            ErrorsKind::UndefinedFunction => println!("[name err]: undefined function"),
            ErrorsKind::FunctionAlreadyDefined => println!("[name err]: function already defined"),
            ErrorsKind::AssignmentTypeMismatch => println!("[type err]: The type is different"),
            ErrorsKind::ReturnTypeMismatch => println!("[type err]: The return type is different")
        }
    }
}
