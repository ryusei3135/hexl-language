use std::sync::{OnceLock, Mutex};


static LINE: OnceLock<Mutex<Vec<String>>> = OnceLock::new();


pub fn add_line(line: String) {
    LINE
        .get_or_init(|| Mutex::new(Vec::<String>::new()))
        .lock()
        .unwrap()
        .push(line);
}

pub fn get_now_line() -> String {
    LINE
        .get_or_init(|| Mutex::new(Vec::<String>::new()))
        .lock()
        .unwrap()
        .last()
        .unwrap()
        .to_string()
}
