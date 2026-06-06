mod parse;
mod reg;
mod mnemo;
mod err;
mod lex;
mod data;
mod gen;
mod sections;

use std::io::Write;
use std::fs::{self, *};

fn main() -> Result<(), std::io::Error> {
    let content = fs::read_to_string("a.s")?;

    let mut parser = parse::Parser::new();
    let mut lexer = lex::Lexer::new();
    let mut generate = gen::Generater::new();

    let tkns = lexer.analy(&content).unwrap();
    let nodes = parser.parser(&tkns).unwrap();
    generate.gen_codes(&nodes).unwrap();
    generate.update_label();

    let mut gen_bin = sections::GenerateBin::new();

    gen_bin.setting_text(generate.code_data.get_code());
    gen_bin.setting_data(generate.data_info.get_code());
    gen_bin.setting_rela_text(generate.rela.clone());
    gen_bin.setting_symtab(generate.symtab.get_code());
    gen_bin.setting_strtab(generate.strtab.clone());

    gen_bin.insert_codes();
    gen_bin.generate_shstrtab();
    gen_bin.generate_elf_header();

    let mut f = File::create("a.o").unwrap();
    f.write_all(&gen_bin.bin).unwrap();

    Ok(())
}
