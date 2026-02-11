use std::fs::File;
use std::io::{BufRead, BufReader};
use std::io::Error;

use crate::parse::ast;
use crate::parse::node;
use crate::token::tokenizer;
use crate::runner::run;


pub fn load_file(file_path: &str) -> Result<(), Error> {
    let file = File::open(file_path)?;
    let reader = BufReader::new(file);

    let mut all_info = node::AllInfo::new();
    let mut tokenizer = tokenizer::Tokenizer::new();
    let mut parser = ast::Parser::new(&mut all_info);

    for (number, line) in reader.lines().enumerate() {
        let line = line?;
        let token_data = tokenizer.make_token(&line, number);
        tokenizer.init();

        if let Err(e) = parser.make_node(token_data) {
            e.print_log(&number, &line);
        }
    }

    let mut runtime = run::Runtime::new(all_info);
    runtime.start_process();

    Ok(())
}
