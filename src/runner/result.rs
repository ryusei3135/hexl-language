use super::*;


pub fn output_log(log: control_syn::ControlSynErr) {
    match log {
        control_syn::ControlSynErr::DataIsNotFound => {
            eprintln!("[err]: for loop data is not found");
        }
        control_syn::ControlSynErr::InvalidIterCond => {
            eprintln!("[err]: for loop cond is invalid");
        }
        control_syn::ControlSynErr::ValueIsOfInvalidType => {
            eprintln!("[err]: This type cannot be used in control statements");
        }
        control_syn::ControlSynErr::MissingCondInForStatement => {
            eprintln!("[err]: For statement condition not found");
        }
        control_syn::ControlSynErr::EndLoop => {},
        _ => println!("err loop"),
    }
}

#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $cond_status:ident) => {
        var_manager().remove_stack();
        var_manager().make_new_stack();

        match $cond_status.now_loop(None, None) {
            Ok(cond_location) => {
                $index = cond_location + 1;
                continue;
            }
            Err(log) => {
                $cond_status.del();
                var_manager().remove_stack();
                output_log(log);
                break;
            }
        }
    };
}
