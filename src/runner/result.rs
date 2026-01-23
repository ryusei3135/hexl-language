
#[macro_export]
macro_rules! update_array_index {
    ($index:ident, $cond_status:ident) => {
        match $cond_status.now_loop(None, None) {
            Ok(cond_location) => {
                $index = cond_location;
                continue;
            }
            Err(log) => {
                match log {
                    control_syn::ControlSynErr::DATA_IS_NOT_FOUND => {},
                    _ => println!("err loop"),
                }
            }
        }
    };
}
