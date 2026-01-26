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
        control_syn::ControlSynErr::EndLoop => {},
        _ => println!("err loop"),
    }
}

#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $cond_status:ident) => {
        match $cond_status.now_loop(None, None) {
            Ok(cond_location) => {
                $index = cond_location + 1;
                continue;
            }
            Err(log) => {
                output_log(log);
                break;
            }
        }
    };
}
