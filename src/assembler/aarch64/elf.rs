//! aarch64 用の ELF 出力
//!
//! ELF のヘッダ構造体 (`Elf64Ehdr` / `Elf64Shdr` / `Elf64Phdr` / `Elf64Sym` /
//! `Elf64Rela`) と定数は、一つ上のディレクトリの `elf.rs` (x64 と共用) のものを使う。
//! ここにあるのは、aarch64 の `Emitter` からファイルの中身を組み立てる部分だけ。
//!
//! - `write_object`     : 再配置可能オブジェクトファイル (`-o`)
//! - `write_executable` : 単独で実行できる ELF (`-c linux`)

use std::collections::HashMap;

use super::super::elf::{
    ET_EXEC, ET_REL, Elf64Ehdr, Elf64Phdr, Elf64Rela, Elf64Shdr, Elf64Sym, PF_R, PF_W, PF_X,
    PT_LOAD, SHF_ALLOC, SHF_EXECINSTR, SHF_WRITE, SHN_UNDEF, SHT_NULL, SHT_PROGBITS, SHT_RELA,
    SHT_STRTAB, SHT_SYMTAB, STB_GLOBAL, STB_LOCAL, STT_FUNC, STT_NOTYPE, STT_OBJECT, r_info,
    st_info,
};
use super::emitter::{Emitter, Section};

pub const EM_AARCH64: u16 = 183;

fn pad_to(buffer: &mut Vec<u8>, align: usize) {
    if align > 1 && buffer.len() % align != 0 {
        buffer.resize(buffer.len() + (align - buffer.len() % align), 0);
    }
}

fn align_up(v: u64, align: u64) -> u64 {
    (v + align - 1) & !(align - 1)
}

fn add_name(table: &mut Vec<u8>, name: &str) -> u32 {
    let off = table.len() as u32;
    table.extend_from_slice(name.as_bytes());
    table.push(0);
    off
}

fn shdr(
    name: u32,
    ty: u32,
    flags: u64,
    addr: u64,
    off: u64,
    size: u64,
    link: u32,
    info: u32,
    align: u64,
    entsize: u64,
) -> Elf64Shdr {
    Elf64Shdr {
        sh_name: name,
        sh_type: ty,
        sh_flags: flags,
        sh_addr: addr,
        sh_offset: off,
        sh_size: size,
        sh_link: link,
        sh_info: info,
        sh_addralign: align,
        sh_entsize: entsize,
    }
}

// ============================================================
// 再配置可能オブジェクトファイル (`-o`)
// ============================================================

/// セクション: .text, .data, (.rela.text), (.rela.data), .symtab, .strtab, .shstrtab
///
/// 同じセクション内で完結する PC 相対の参照は `Emitter::finish()` が解決済みなので、
/// ここでは残った参照 (別セクション / 外部シンボル / 絶対アドレス / `:lo12:` / `adrp`)
/// を `Rela` エントリにする。addend はすべて 0。
pub fn write_object(emitter: &Emitter) -> Result<Vec<u8>, String> {
    const SHNDX_TEXT: u16 = 1;
    const SHNDX_DATA: u16 = 2;

    struct SymEntry {
        name: String,
        shndx: u16,
        value: u64,
        kind: u8,
    }

    let mut locals = Vec::new();
    let mut globals = Vec::new();

    let mut classify = |name: &String, shndx: u16, value: usize, kind: u8| {
        let entry = SymEntry {
            name: name.clone(),
            shndx,
            value: value as u64,
            kind,
        };

        if emitter.globals.contains(name) {
            globals.push(entry);
        } else {
            locals.push(entry);
        }
    };

    for (name, &off) in &emitter.text_labels {
        classify(name, SHNDX_TEXT, off, STT_FUNC);
    }

    for (name, &off) in &emitter.data_labels {
        classify(name, SHNDX_DATA, off, STT_OBJECT);
    }

    for name in &emitter.externs {
        // 同じ名前が定義済みなら重複させない
        if !emitter.text_labels.contains_key(name) && !emitter.data_labels.contains_key(name) {
            globals.push(SymEntry {
                name: name.clone(),
                shndx: SHN_UNDEF,
                value: 0,
                kind: STT_NOTYPE,
            });
        }
    }

    locals.sort_by(|a, b| a.name.cmp(&b.name));
    globals.sort_by(|a, b| a.name.cmp(&b.name));

    let mut strtab: Vec<u8> = vec![0];
    let mut symtab: Vec<u8> = Vec::new();
    let mut sym_index: HashMap<String, u32> = HashMap::new();

    symtab.extend_from_slice(
        &Elf64Sym {
            st_name: 0,
            st_info: 0,
            st_other: 0,
            st_shndx: 0,
            st_value: 0,
            st_size: 0,
        }
        .to_bytes(),
    );

    let mut next_index = 1u32;

    for (bind, entries) in [(STB_LOCAL, &locals), (STB_GLOBAL, &globals)] {
        if bind == STB_GLOBAL {
            // .symtab の sh_info (最初のグローバルシンボルの番号) は下で使う
        }

        for entry in entries.iter() {
            let name_off = add_name(&mut strtab, &entry.name);

            symtab.extend_from_slice(
                &Elf64Sym {
                    st_name: name_off,
                    st_info: st_info(bind, entry.kind),
                    st_other: 0,
                    st_shndx: entry.shndx,
                    st_value: entry.value,
                    st_size: 0,
                }
                .to_bytes(),
            );

            sym_index.insert(entry.name.clone(), next_index);
            next_index += 1;
        }
    }

    let first_global = 1 + locals.len() as u32;

    // ---- 再配置エントリ ----
    let mut rela_text: Vec<u8> = Vec::new();
    let mut rela_data: Vec<u8> = Vec::new();

    for fixup in &emitter.fixups {
        let idx = *sym_index
            .get(&fixup.symbol)
            .ok_or_else(|| format!("undefined symbol: {}", fixup.symbol))?;

        let rela = Elf64Rela {
            r_offset: fixup.offset as u64,
            r_info: r_info(idx, fixup.kind.elf_reloc_type()),
            r_addend: 0,
        };

        match fixup.section {
            Section::Text => rela_text.extend_from_slice(&rela.to_bytes()),
            Section::Data => rela_data.extend_from_slice(&rela.to_bytes()),
        }
    }

    // ---- セクション名 + 番号 ----
    let mut shstrtab: Vec<u8> = vec![0];

    let n_text = add_name(&mut shstrtab, ".text");
    let n_data = add_name(&mut shstrtab, ".data");
    let n_rela_text = (!rela_text.is_empty()).then(|| add_name(&mut shstrtab, ".rela.text"));
    let n_rela_data = (!rela_data.is_empty()).then(|| add_name(&mut shstrtab, ".rela.data"));
    let n_symtab = add_name(&mut shstrtab, ".symtab");
    let n_strtab = add_name(&mut shstrtab, ".strtab");
    let n_shstrtab = add_name(&mut shstrtab, ".shstrtab");

    let mut next_shndx = 3u32;
    let mut take = |present: bool| -> Option<u32> {
        if present {
            let i = next_shndx;
            next_shndx += 1;
            Some(i)
        } else {
            None
        }
    };

    let i_rela_text = take(n_rela_text.is_some());
    let i_rela_data = take(n_rela_data.is_some());
    let i_symtab = take(true).unwrap();
    let i_strtab = take(true).unwrap();
    let i_shstrtab = take(true).unwrap();

    // ---- ファイルの配置 ----
    let mut out: Vec<u8> = vec![0; 64]; // ELF ヘッダ (最後に書く)
    let mut headers: Vec<Elf64Shdr> = vec![shdr(0, SHT_NULL, 0, 0, 0, 0, 0, 0, 0, 0)];

    pad_to(&mut out, 16);
    let text_off = out.len() as u64;
    out.extend_from_slice(&emitter.text);
    headers.push(shdr(
        n_text,
        SHT_PROGBITS,
        SHF_ALLOC | SHF_EXECINSTR,
        0,
        text_off,
        emitter.text.len() as u64,
        0,
        0,
        16,
        0,
    ));

    pad_to(&mut out, 8);
    let data_off = out.len() as u64;
    out.extend_from_slice(&emitter.data);
    headers.push(shdr(
        n_data,
        SHT_PROGBITS,
        SHF_ALLOC | SHF_WRITE,
        0,
        data_off,
        emitter.data.len() as u64,
        0,
        0,
        8,
        0,
    ));

    for (name, idx, body, target) in [
        (n_rela_text, i_rela_text, &rela_text, SHNDX_TEXT),
        (n_rela_data, i_rela_data, &rela_data, SHNDX_DATA),
    ] {
        if let (Some(name), Some(_)) = (name, idx) {
            pad_to(&mut out, 8);
            let off = out.len() as u64;
            out.extend_from_slice(body);
            headers.push(shdr(
                name,
                SHT_RELA,
                0x40, // SHF_INFO_LINK
                0,
                off,
                body.len() as u64,
                i_symtab,
                target as u32,
                8,
                24,
            ));
        }
    }

    pad_to(&mut out, 8);
    let symtab_off = out.len() as u64;
    out.extend_from_slice(&symtab);
    headers.push(shdr(
        n_symtab,
        SHT_SYMTAB,
        0,
        0,
        symtab_off,
        symtab.len() as u64,
        i_strtab,
        first_global,
        8,
        24,
    ));

    let strtab_off = out.len() as u64;
    out.extend_from_slice(&strtab);
    headers.push(shdr(
        n_strtab,
        SHT_STRTAB,
        0,
        0,
        strtab_off,
        strtab.len() as u64,
        0,
        0,
        1,
        0,
    ));

    let shstrtab_off = out.len() as u64;
    out.extend_from_slice(&shstrtab);
    headers.push(shdr(
        n_shstrtab,
        SHT_STRTAB,
        0,
        0,
        shstrtab_off,
        shstrtab.len() as u64,
        0,
        0,
        1,
        0,
    ));

    pad_to(&mut out, 8);
    let shoff = out.len() as u64;

    for h in &headers {
        out.extend_from_slice(&h.to_bytes());
    }

    let mut ehdr = Elf64Ehdr::new();
    ehdr.e_type = ET_REL;
    ehdr.e_machine = EM_AARCH64;
    ehdr.e_shoff = shoff;
    ehdr.e_shnum = headers.len() as u16;
    ehdr.e_shstrndx = i_shstrtab as u16;

    out[..64].copy_from_slice(&ehdr.to_bytes());

    Ok(out)
}

// ============================================================
// 実行ファイル (`-c linux`)
// ============================================================

/// 単独で実行できる ELF (ET_EXEC) を作る。
///
/// RWX の PT_LOAD を1つだけ持ち、ベースアドレスは 0x400000。
/// `_start` ラベルがあればそこから、無ければ `.text` の先頭から実行する。
/// 外部リンクは行わないので、未解決の外部シンボルが残っているとエラーになる。
pub fn write_executable(emitter: &Emitter) -> Result<Vec<u8>, String> {
    const BASE: u64 = 0x40_0000;
    const EHDR_SIZE: u64 = 64;
    const PHDR_SIZE: u64 = 56;

    // 命令を16バイト境界に置く
    let text_off = align_up(EHDR_SIZE + PHDR_SIZE, 16);
    let data_off = align_up(text_off + emitter.text.len() as u64, 16);

    let mut text = emitter.text.clone();
    let mut data = emitter.data.clone();

    let resolve = |symbol: &str| -> Option<u64> {
        if let Some(&off) = emitter.text_labels.get(symbol) {
            return Some(BASE + text_off + off as u64);
        }

        emitter
            .data_labels
            .get(symbol)
            .map(|&off| BASE + data_off + off as u64)
    };

    for fixup in &emitter.fixups {
        let target = resolve(&fixup.symbol).ok_or_else(|| {
            format!(
                "実行ファイルを直接生成できません: シンボル `{}` が未定義です \
                 (外部シンボルが必要な場合は -o でオブジェクトファイルを生成し、\
                 ld 等でリンクしてください)",
                fixup.symbol
            )
        })?;

        let (section_base, buffer) = match fixup.section {
            Section::Text => (BASE + text_off, &mut text),
            Section::Data => (BASE + data_off, &mut data),
        };

        fixup
            .kind
            .patch(
                buffer,
                fixup.offset,
                target,
                section_base + fixup.offset as u64,
            )
            .map_err(|e| format!("{} ({})", e, fixup.symbol))?;
    }

    let filesz = data_off + data.len() as u64;

    let entry = match emitter.text_labels.get("_start") {
        Some(&off) => BASE + text_off + off as u64,
        None => BASE + text_off,
    };

    // ---- セクションヘッダ (.text, .data, .shstrtab) ----
    let mut shstrtab: Vec<u8> = vec![0];
    let n_text = add_name(&mut shstrtab, ".text");
    let n_data = add_name(&mut shstrtab, ".data");
    let n_shstr = add_name(&mut shstrtab, ".shstrtab");

    let shstr_off = filesz;
    let shoff = align_up(shstr_off + shstrtab.len() as u64, 8);

    let headers = [
        shdr(0, SHT_NULL, 0, 0, 0, 0, 0, 0, 0, 0),
        shdr(
            n_text,
            SHT_PROGBITS,
            SHF_ALLOC | SHF_EXECINSTR,
            BASE + text_off,
            text_off,
            text.len() as u64,
            0,
            0,
            16,
            0,
        ),
        shdr(
            n_data,
            SHT_PROGBITS,
            SHF_ALLOC | SHF_WRITE,
            BASE + data_off,
            data_off,
            data.len() as u64,
            0,
            0,
            16,
            0,
        ),
        shdr(
            n_shstr,
            SHT_STRTAB,
            0,
            0,
            shstr_off,
            shstrtab.len() as u64,
            0,
            0,
            1,
            0,
        ),
    ];

    let mut ehdr = Elf64Ehdr::new();
    ehdr.e_type = ET_EXEC;
    ehdr.e_machine = EM_AARCH64;
    ehdr.e_entry = entry;
    ehdr.e_phoff = EHDR_SIZE;
    ehdr.e_phnum = 1;
    ehdr.e_shoff = shoff;
    ehdr.e_shnum = headers.len() as u16;
    ehdr.e_shstrndx = (headers.len() - 1) as u16;

    let phdr = Elf64Phdr {
        p_type: PT_LOAD,
        p_flags: PF_R | PF_W | PF_X,
        p_offset: 0,
        p_vaddr: BASE,
        p_paddr: BASE,
        p_filesz: filesz,
        p_memsz: filesz,
        p_align: 0x1000,
    };

    let mut out = Vec::with_capacity(shoff as usize + 4 * 64);
    out.extend_from_slice(&ehdr.to_bytes());
    out.extend_from_slice(&phdr.to_bytes());
    out.resize(text_off as usize, 0);
    out.extend_from_slice(&text);
    out.resize(data_off as usize, 0);
    out.extend_from_slice(&data);

    debug_assert_eq!(out.len() as u64, shstr_off);

    out.extend_from_slice(&shstrtab);
    out.resize(shoff as usize, 0);

    for h in &headers {
        out.extend_from_slice(&h.to_bytes());
    }

    Ok(out)
}
