use std::fs::File;
use std::io::{BufRead, BufReader};
use std::io::Error;

use crate::parse::parse;
use crate::runner::run;
use crate::token::tokenizer;
use crate::manager::func;

use crate::runner;


pub fn load_file(file_path: &str) -> Result<(), Error> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut tokenizer = tokenizer::Tokenizer::new();
    let mut parser = parse::Parser::new();

    let mut func_datas = func::FuncManager::new();

    for (number, line) in reader.lines().enumerate() {
        let line = line?;
        let token_data = tokenizer.make_token(line, number);

        parser.make_node(
            token_data, 
            &mut func_datas
        );
    }

    runner::run::start_process(&func_datas);

    Ok(())
}