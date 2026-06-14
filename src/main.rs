mod lex;
mod err;
mod parse;
mod node;
mod ir;

use std::fs;
use std::io::{self, BufRead, BufReader};

fn main() -> io::Result<()> {
    let content = fs::read_to_string("test.hexl")?;

    let mut lexer = lex::Lexer::new();
    let mut parser = parse::Parser::new();
    let mut count: usize = 1;
    let mut ir_builder = ir::IR::new();


    lexer.analy(&content).map_err(|v| v.print_log(&content)); 

    let nodes = parser.parser(lexer.gen_tkns.clone(), &count)
        .map_err(|v| v.print_log(&content))
        .unwrap();
    ir_builder.builder(&nodes);
    Ok(())
}
