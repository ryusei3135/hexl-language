use std::env;
mod load;

mod parse;
mod runner;
mod token;
mod manager;


fn main() {
    let args: Vec<String> = env::args().collect();

    if args.len() > 1 {
        let file_path = &args[1];
        if file_path.contains(".bnn") {
            load::load_file(file_path).unwrap();
        }
    }
}