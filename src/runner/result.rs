
#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $cond_status:ident) => {
        match $cond_status.now_loop(None, None) {
            Ok(cond_location) => {
                $index = cond_location + 1;
                continue;
            }
            Err(log) => {
                match log {
                    control_syn::ControlSynErr::DATA_IS_NOT_FOUND => {
                        eprintln!("[err]: for loop data is not found");
                    }
                    control_syn::ControlSynErr::INVALID_ITER_COND => {
                        eprintln!("[err]: for loop cond is invalid");
                    }
                    _ => println!("err loop"),
                }
            }
        }
    };
}
