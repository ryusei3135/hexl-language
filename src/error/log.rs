use super::*;


pub fn output_log_l0(log: control_syn::ControlSynErr) {
    match log {
        control_syn::ControlSynErr::DataIsNotFound => {
            eprintln!("[err]:[E:S00:L0] for loop data is not found");
        }
        control_syn::ControlSynErr::InvalidIterCond => {
            eprintln!("[err]:[E:S01:L0] for loop cond is invalid");
        }
        control_syn::ControlSynErr::ValueIsOfInvalidType => {
            eprintln!("[err]:[E:S02:L0] This type cannot be used in control statements");
        }
        control_syn::ControlSynErr::MissingCondInForStatement => {
            eprintln!("[err]:[E:S03:L0] For statement condition not found");
        }
        control_syn::ControlSynErr::EndLoop => {},
        _ => println!("err loop"),
    }
}
