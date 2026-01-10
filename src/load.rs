use std::fs::File;
use std::io::{BufRead, BufReader};
use std::io::Error;

use crate::parse::ast;
use crate::token::tokenizer;
use crate::runner;
use crate::error;


pub fn load_file(file_path: &str) -> Result<(), Error> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut tokenizer = tokenizer::Tokenizer::new();
    let mut parser = ast::Parser::new();

    for (number, line) in reader.lines().enumerate() {
        let line = line?;
        error::err_handling::add_line(line.clone());
        let token_data = tokenizer.make_token(line, number);
        tokenizer.init();

        parser.make_node(token_data);
    }

    runner::run::start_process();

    Ok(())
}
