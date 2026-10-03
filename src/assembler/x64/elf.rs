use std::collections::HashMap;

use super::emitter::{Emitter, FixupKind, Section};

pub const ELF_MAGIC: [u8; 4] = [0x7f, b'E', b'L', b'F'];

pub const ELFCLASS64: u8 = 2;
pub const ELFDATA2LSB: u8 = 1;
pub const EV_CURRENT: u32 = 1;

pub const EM_X86_64: u16 = 62;

// ELF file type
pub const ET_NONE: u16 = 0;
pub const ET_REL: u16 = 1;
pub const ET_EXEC: u16 = 2;
pub const ET_DYN: u16 = 3;

// Section type
pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_NOBITS: u32 = 8;

// Section flags
pub const SHF_WRITE: u64 = 1;
pub const SHF_ALLOC: u64 = 2;
pub const SHF_EXECINSTR: u64 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Ehdr {
    pub e_ident: [u8; 16],

    pub e_type: u16,
    pub e_machine: u16,
    pub e_version: u32,

    pub e_entry: u64,
    pub e_phoff: u64,
    pub e_shoff: u64,

    pub e_flags: u32,

    pub e_ehsize: u16,
    pub e_phentsize: u16,
    pub e_phnum: u16,

    pub e_shentsize: u16,
    pub e_shnum: u16,
    pub e_shstrndx: u16,
}

impl Elf64Ehdr {
    pub fn new() -> Self {
        Self {
            e_ident: [
                0x7f,
                b'E',
                b'L',
                b'F',
                ELFCLASS64,
                ELFDATA2LSB,
                EV_CURRENT as u8,
                0, // OS ABI
                0, // ABI version
                0,
                0,
                0,
                0,
                0,
                0,
                0,
            ],

            e_type: ET_REL,
            e_machine: EM_X86_64,
            e_version: EV_CURRENT,

            e_entry: 0,
            e_phoff: 0,
            e_shoff: 0,

            e_flags: 0,

            e_ehsize: 64,
            e_phentsize: 56,
            e_phnum: 0,

            e_shentsize: 64,
            e_shnum: 0,
            e_shstrndx: 0,
        }
    }

    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);

        buf.extend_from_slice(&self.e_ident);
        buf.extend_from_slice(&self.e_type.to_le_bytes());
        buf.extend_from_slice(&self.e_machine.to_le_bytes());
        buf.extend_from_slice(&self.e_version.to_le_bytes());
        buf.extend_from_slice(&self.e_entry.to_le_bytes());
        buf.extend_from_slice(&self.e_phoff.to_le_bytes());
        buf.extend_from_slice(&self.e_shoff.to_le_bytes());
        buf.extend_from_slice(&self.e_flags.to_le_bytes());
        buf.extend_from_slice(&self.e_ehsize.to_le_bytes());
        buf.extend_from_slice(&self.e_phentsize.to_le_bytes());
        buf.extend_from_slice(&self.e_phnum.to_le_bytes());
        buf.extend_from_slice(&self.e_shentsize.to_le_bytes());
        buf.extend_from_slice(&self.e_shnum.to_le_bytes());
        buf.extend_from_slice(&self.e_shstrndx.to_le_bytes());

        debug_assert_eq!(buf.len(), 64);

        buf
    }
}

// ============================================================
// Program header (for executables)
// ============================================================

pub const PT_LOAD: u32 = 1;

pub const PF_X: u32 = 1;
pub const PF_W: u32 = 2;
pub const PF_R: u32 = 4;

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Phdr {
    pub p_type: u32,
    pub p_flags: u32,
    pub p_offset: u64,
    pub p_vaddr: u64,
    pub p_paddr: u64,
    pub p_filesz: u64,
    pub p_memsz: u64,
    pub p_align: u64,
}

impl Elf64Phdr {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(56);

        buf.extend_from_slice(&self.p_type.to_le_bytes());
        buf.extend_from_slice(&self.p_flags.to_le_bytes());
        buf.extend_from_slice(&self.p_offset.to_le_bytes());
        buf.extend_from_slice(&self.p_vaddr.to_le_bytes());
        buf.extend_from_slice(&self.p_paddr.to_le_bytes());
        buf.extend_from_slice(&self.p_filesz.to_le_bytes());
        buf.extend_from_slice(&self.p_memsz.to_le_bytes());
        buf.extend_from_slice(&self.p_align.to_le_bytes());

        debug_assert_eq!(buf.len(), 56);

        buf
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Shdr {
    pub sh_name: u32,
    pub sh_type: u32,
    pub sh_flags: u64,
    pub sh_addr: u64,
    pub sh_offset: u64,
    pub sh_size: u64,
    pub sh_link: u32,
    pub sh_info: u32,
    pub sh_addralign: u64,
    pub sh_entsize: u64,
}

impl Elf64Shdr {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(64);

        buf.extend_from_slice(&self.sh_name.to_le_bytes());
        buf.extend_from_slice(&self.sh_type.to_le_bytes());
        buf.extend_from_slice(&self.sh_flags.to_le_bytes());
        buf.extend_from_slice(&self.sh_addr.to_le_bytes());
        buf.extend_from_slice(&self.sh_offset.to_le_bytes());
        buf.extend_from_slice(&self.sh_size.to_le_bytes());
        buf.extend_from_slice(&self.sh_link.to_le_bytes());
        buf.extend_from_slice(&self.sh_info.to_le_bytes());
        buf.extend_from_slice(&self.sh_addralign.to_le_bytes());
        buf.extend_from_slice(&self.sh_entsize.to_le_bytes());

        debug_assert_eq!(buf.len(), 64);

        buf
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Sym {
    pub st_name: u32,
    pub st_info: u8,
    pub st_other: u8,
    pub st_shndx: u16,
    pub st_value: u64,
    pub st_size: u64,
}

impl Elf64Sym {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(24);

        buf.extend_from_slice(&self.st_name.to_le_bytes());
        buf.push(self.st_info);
        buf.push(self.st_other);
        buf.extend_from_slice(&self.st_shndx.to_le_bytes());
        buf.extend_from_slice(&self.st_value.to_le_bytes());
        buf.extend_from_slice(&self.st_size.to_le_bytes());

        debug_assert_eq!(buf.len(), 24);

        buf
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy)]
pub struct Elf64Rela {
    pub r_offset: u64,
    pub r_info: u64,
    pub r_addend: i64,
}

impl Elf64Rela {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::with_capacity(24);

        buf.extend_from_slice(&self.r_offset.to_le_bytes());
        buf.extend_from_slice(&self.r_info.to_le_bytes());
        buf.extend_from_slice(&self.r_addend.to_le_bytes());

        debug_assert_eq!(buf.len(), 24);

        buf
    }
}

// Symbol binding / type (Elf64_Sym.st_info)
pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;

pub const STT_NOTYPE: u8 = 0;
pub const STT_OBJECT: u8 = 1;
pub const STT_FUNC: u8 = 2;

pub const SHN_UNDEF: u16 = 0;

pub fn st_info(bind: u8, kind: u8) -> u8 {
    (bind << 4) | (kind & 0xf)
}

// x86-64 relocation types (Elf64_Rela.r_info low 32 bits)
pub const R_X86_64_64: u32 = 1;
pub const R_X86_64_PC32: u32 = 2;
pub const R_X86_64_32: u32 = 10;

pub fn r_info(sym: u32, kind: u32) -> u64 {
    ((sym as u64) << 32) | (kind as u64)
}

// ============================================================
// Layout helpers
// ============================================================

fn pad_to(buffer: &mut Vec<u8>, align: usize) {
    if align <= 1 {
        return;
    }

    let remainder = buffer.len() % align;

    if remainder != 0 {
        buffer.extend(std::iter::repeat(0u8).take(align - remainder));
    }
}

// ============================================================
// Relocatable object file (`-o`)
// ============================================================

/// Emitter の結果から ET_REL (再配置可能オブジェクトファイル) を組み立てる。
///
/// 生成されるセクション: .text, .data, (test), (.rela.text), (.rela.data),
/// (.rela.test), .symtab, .strtab, .shstrtab
///
/// `test` セクションは `.section test` が使われたときだけ出力する。
pub fn write_object(emitter: &Emitter) -> Result<Vec<u8>, String> {
    const SHNDX_TEXT: u16 = 1;
    const SHNDX_DATA: u16 = 2;
    const SHNDX_TEST: u16 = 3; // `.section test` を使ったときだけ存在する

    let uses_test = emitter.uses_test;

    // --------------------------------------------------------
    // シンボルテーブルの構築
    // (ローカルシンボルを先に、その後グローバルシンボル)
    // --------------------------------------------------------

    struct SymEntry {
        name: String,
        shndx: u16,
        value: u64,
        kind: u8,
    }

    let mut locals = Vec::new();
    let mut globals = Vec::new();

    for (name, &offset) in emitter.text_labels.iter() {
        let entry = SymEntry {
            name: name.clone(),
            shndx: SHNDX_TEXT,
            value: offset as u64,
            kind: STT_FUNC,
        };

        if emitter.globals.contains(name) {
            globals.push(entry);
        } else {
            locals.push(entry);
        }
    }

    for (name, &offset) in emitter.data_labels.iter() {
        let entry = SymEntry {
            name: name.clone(),
            shndx: SHNDX_DATA,
            value: offset as u64,
            kind: STT_OBJECT,
        };

        if emitter.globals.contains(name) {
            globals.push(entry);
        } else {
            locals.push(entry);
        }
    }

    for (name, &offset) in emitter.test_labels.iter() {
        let entry = SymEntry {
            name: name.clone(),
            shndx: SHNDX_TEST,
            value: offset as u64,
            kind: STT_OBJECT,
        };

        if emitter.globals.contains(name) {
            globals.push(entry);
        } else {
            locals.push(entry);
        }
    }

    for name in &emitter.externs {
        globals.push(SymEntry {
            name: name.clone(),
            shndx: SHN_UNDEF,
            value: 0,
            kind: STT_NOTYPE,
        });
    }

    locals.sort_by(|a, b| a.name.cmp(&b.name));
    globals.sort_by(|a, b| a.name.cmp(&b.name));

    let mut strtab: Vec<u8> = vec![0]; // index 0 は空文字列
    let mut symtab: Vec<u8> = Vec::new();
    let mut sym_index: HashMap<String, u32> = HashMap::new();

    // シンボル 0 番は null エントリ
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

    for entry in &locals {
        let name_off = strtab.len() as u32;
        strtab.extend_from_slice(entry.name.as_bytes());
        strtab.push(0);

        symtab.extend_from_slice(
            &Elf64Sym {
                st_name: name_off,
                st_info: st_info(STB_LOCAL, entry.kind),
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

    let first_global = next_index;

    for entry in &globals {
        let name_off = strtab.len() as u32;
        strtab.extend_from_slice(entry.name.as_bytes());
        strtab.push(0);

        symtab.extend_from_slice(
            &Elf64Sym {
                st_name: name_off,
                st_info: st_info(STB_GLOBAL, entry.kind),
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

    // --------------------------------------------------------
    // 再配置エントリの構築
    //
    // 同一セクション内で完結する Rel32 / RIP相対 は
    // Emitter::finish() が既に直接パッチ済みなので再配置は不要。
    // それ以外 (外部シンボル、セクションをまたぐRIP相対、
    // 絶対アドレス参照) はリンカに解決してもらうため
    // Rela エントリを積む。
    // --------------------------------------------------------

    let mut rela_text: Vec<u8> = Vec::new();
    let mut rela_data: Vec<u8> = Vec::new();
    let mut rela_test: Vec<u8> = Vec::new();

    for fixup in &emitter.fixups {
        let defined_locally = emitter.text_labels.contains_key(&fixup.symbol)
            || emitter.data_labels.contains_key(&fixup.symbol)
            || emitter.test_labels.contains_key(&fixup.symbol);

        let same_section_as = |section: Section| -> bool {
            match section {
                Section::Text => emitter.text_labels.contains_key(&fixup.symbol),
                Section::Data => emitter.data_labels.contains_key(&fixup.symbol),
                Section::Test => emitter.test_labels.contains_key(&fixup.symbol),
            }
        };

        let (needs_reloc, reloc_type, addend) = match fixup.kind {
            FixupKind::Rel32 => (!defined_locally, R_X86_64_PC32, -4i64),

            FixupKind::RipRelative32 => {
                let same_section = defined_locally && same_section_as(fixup.section);
                (!same_section, R_X86_64_PC32, -4i64)
            }

            // 絶対アドレスはリンク時にしか確定しないため、
            // ローカル/外部を問わず常に再配置エントリにする。
            FixupKind::Abs32 => (true, R_X86_64_32, 0i64),
            FixupKind::Abs64 => (true, R_X86_64_64, 0i64),
        };

        if !needs_reloc {
            continue;
        }

        let sym_idx = *sym_index
            .get(&fixup.symbol)
            .ok_or_else(|| format!("undefined symbol: {}", fixup.symbol))?;

        let rela = Elf64Rela {
            r_offset: fixup.offset as u64,
            r_info: r_info(sym_idx, reloc_type),
            r_addend: addend,
        };

        match fixup.section {
            Section::Text => rela_text.extend_from_slice(&rela.to_bytes()),
            Section::Data => rela_data.extend_from_slice(&rela.to_bytes()),
            Section::Test => rela_test.extend_from_slice(&rela.to_bytes()),
        }
    }

    // --------------------------------------------------------
    // セクションヘッダ文字列テーブル + セクション番号の割り当て
    // --------------------------------------------------------

    let mut shstrtab: Vec<u8> = vec![0];

    let mut push_name = |shstrtab: &mut Vec<u8>, s: &str| -> u32 {
        let off = shstrtab.len() as u32;
        shstrtab.extend_from_slice(s.as_bytes());
        shstrtab.push(0);
        off
    };

    let name_text = push_name(&mut shstrtab, ".text");
    let name_data = push_name(&mut shstrtab, ".data");

    let name_test = if uses_test {
        Some(push_name(&mut shstrtab, "test"))
    } else {
        None
    };

    let name_rela_text = if !rela_text.is_empty() {
        Some(push_name(&mut shstrtab, ".rela.text"))
    } else {
        None
    };

    let name_rela_data = if !rela_data.is_empty() {
        Some(push_name(&mut shstrtab, ".rela.data"))
    } else {
        None
    };

    let name_rela_test = if !rela_test.is_empty() {
        Some(push_name(&mut shstrtab, ".rela.test"))
    } else {
        None
    };

    let name_symtab = push_name(&mut shstrtab, ".symtab");
    let name_strtab = push_name(&mut shstrtab, ".strtab");
    let name_shstrtab = push_name(&mut shstrtab, ".shstrtab");

    // 1=.text, 2=.data (, 3=test) は確定済み
    let mut next_shndx: u16 = if uses_test { 4 } else { 3 };

    let rela_text_shndx = if !rela_text.is_empty() {
        let idx = next_shndx;
        next_shndx += 1;
        Some(idx)
    } else {
        None
    };

    let rela_data_shndx = if !rela_data.is_empty() {
        let idx = next_shndx;
        next_shndx += 1;
        Some(idx)
    } else {
        None
    };

    let rela_test_shndx = if !rela_test.is_empty() {
        let idx = next_shndx;
        next_shndx += 1;
        Some(idx)
    } else {
        None
    };

    let symtab_shndx = next_shndx;
    next_shndx += 1;

    let strtab_shndx = next_shndx;
    next_shndx += 1;

    let shstrtab_shndx = next_shndx;
    next_shndx += 1;

    let shnum = next_shndx;

    // --------------------------------------------------------
    // ファイル本体の構築
    // --------------------------------------------------------

    let mut file: Vec<u8> = vec![0u8; 64]; // ELFヘッダ分の場所を確保
    let mut headers: Vec<Elf64Shdr> = Vec::new();

    // index 0: NULL section
    headers.push(Elf64Shdr {
        sh_name: 0,
        sh_type: SHT_NULL,
        sh_flags: 0,
        sh_addr: 0,
        sh_offset: 0,
        sh_size: 0,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 0,
        sh_entsize: 0,
    });

    // .text
    pad_to(&mut file, 16);
    let text_off = file.len();
    file.extend_from_slice(&emitter.text);
    headers.push(Elf64Shdr {
        sh_name: name_text,
        sh_type: SHT_PROGBITS,
        sh_flags: SHF_ALLOC | SHF_EXECINSTR,
        sh_addr: 0,
        sh_offset: text_off as u64,
        sh_size: emitter.text.len() as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 16,
        sh_entsize: 0,
    });

    // .data
    pad_to(&mut file, 8);
    let data_off = file.len();
    file.extend_from_slice(&emitter.data);
    headers.push(Elf64Shdr {
        sh_name: name_data,
        sh_type: SHT_PROGBITS,
        sh_flags: SHF_ALLOC | SHF_WRITE,
        sh_addr: 0,
        sh_offset: data_off as u64,
        sh_size: emitter.data.len() as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 8,
        sh_entsize: 0,
    });

    // test (`.section test`)
    if uses_test {
        pad_to(&mut file, 8);
        let test_off = file.len();
        file.extend_from_slice(&emitter.test);
        headers.push(Elf64Shdr {
            sh_name: name_test.unwrap(),
            sh_type: SHT_PROGBITS,
            sh_flags: SHF_ALLOC | SHF_WRITE,
            sh_addr: 0,
            sh_offset: test_off as u64,
            sh_size: emitter.test.len() as u64,
            sh_link: 0,
            sh_info: 0,
            sh_addralign: 8,
            sh_entsize: 0,
        });
    }

    // .rela.text
    if let Some(shndx) = rela_text_shndx {
        pad_to(&mut file, 8);
        let off = file.len();
        file.extend_from_slice(&rela_text);
        headers.push(Elf64Shdr {
            sh_name: name_rela_text.unwrap(),
            sh_type: SHT_RELA,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: off as u64,
            sh_size: rela_text.len() as u64,
            sh_link: symtab_shndx as u32,
            sh_info: SHNDX_TEXT as u32,
            sh_addralign: 8,
            sh_entsize: 24,
        });
        let _ = shndx;
    }

    // .rela.data
    if let Some(shndx) = rela_data_shndx {
        pad_to(&mut file, 8);
        let off = file.len();
        file.extend_from_slice(&rela_data);
        headers.push(Elf64Shdr {
            sh_name: name_rela_data.unwrap(),
            sh_type: SHT_RELA,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: off as u64,
            sh_size: rela_data.len() as u64,
            sh_link: symtab_shndx as u32,
            sh_info: SHNDX_DATA as u32,
            sh_addralign: 8,
            sh_entsize: 24,
        });
        let _ = shndx;
    }

    // .rela.test
    if let Some(shndx) = rela_test_shndx {
        pad_to(&mut file, 8);
        let off = file.len();
        file.extend_from_slice(&rela_test);
        headers.push(Elf64Shdr {
            sh_name: name_rela_test.unwrap(),
            sh_type: SHT_RELA,
            sh_flags: 0,
            sh_addr: 0,
            sh_offset: off as u64,
            sh_size: rela_test.len() as u64,
            sh_link: symtab_shndx as u32,
            sh_info: SHNDX_TEST as u32,
            sh_addralign: 8,
            sh_entsize: 24,
        });
        let _ = shndx;
    }

    // .symtab
    pad_to(&mut file, 8);
    let symtab_off = file.len();
    file.extend_from_slice(&symtab);
    headers.push(Elf64Shdr {
        sh_name: name_symtab,
        sh_type: SHT_SYMTAB,
        sh_flags: 0,
        sh_addr: 0,
        sh_offset: symtab_off as u64,
        sh_size: symtab.len() as u64,
        sh_link: strtab_shndx as u32,
        sh_info: first_global,
        sh_addralign: 8,
        sh_entsize: 24,
    });

    // .strtab
    let strtab_off = file.len();
    file.extend_from_slice(&strtab);
    headers.push(Elf64Shdr {
        sh_name: name_strtab,
        sh_type: SHT_STRTAB,
        sh_flags: 0,
        sh_addr: 0,
        sh_offset: strtab_off as u64,
        sh_size: strtab.len() as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 1,
        sh_entsize: 0,
    });

    // .shstrtab
    let shstrtab_off = file.len();
    file.extend_from_slice(&shstrtab);
    headers.push(Elf64Shdr {
        sh_name: name_shstrtab,
        sh_type: SHT_STRTAB,
        sh_flags: 0,
        sh_addr: 0,
        sh_offset: shstrtab_off as u64,
        sh_size: shstrtab.len() as u64,
        sh_link: 0,
        sh_info: 0,
        sh_addralign: 1,
        sh_entsize: 0,
    });

    // セクションヘッダテーブル本体
    pad_to(&mut file, 8);
    let shoff = file.len();

    for header in &headers {
        file.extend_from_slice(&header.to_bytes());
    }

    // ELFヘッダを確定させて先頭に書き込む
    let mut ehdr = Elf64Ehdr::new();
    ehdr.e_type = ET_REL;
    ehdr.e_shoff = shoff as u64;
    ehdr.e_shnum = shnum;
    ehdr.e_shstrndx = shstrtab_shndx;

    file[0..64].copy_from_slice(&ehdr.to_bytes());

    Ok(file)
}

// ============================================================
// 実行ファイル (`-c`)
// ============================================================

/// Emitter の結果から、単一の PT_LOAD セグメントに .text/.data (/test) を
/// そのまま詰め込んだ最小構成の ET_EXEC 実行ファイルを組み立てる。
///
/// セクションヘッダテーブル (.text, .data, (test), .shstrtab) を
/// ファイル末尾に付ける。`test` は `.section test` が使われた場合だけ。
///
/// 外部リンクは行わないため、未解決の外部シンボル (`.extern`) が
/// 残っている場合はエラーになる。その場合は `-o` でオブジェクトを
/// 生成し、`ld` 等の外部リンカでリンクすること。
pub fn write_executable(emitter: &Emitter) -> Result<Vec<u8>, String> {
    const BASE: u64 = 0x400000;
    const EHDR_SIZE: u64 = 64;
    const PHDR_SIZE: u64 = 56;

    let text_off = EHDR_SIZE + PHDR_SIZE;
    let data_off = text_off + emitter.text.len() as u64;
    let uses_test = emitter.uses_test;

    // test は 8 バイト境界に置く (パディングは data 側に含める)
    let test_off = if uses_test {
        (data_off + emitter.data.len() as u64 + 7) & !7
    } else {
        data_off + emitter.data.len() as u64
    };

    let mut text = emitter.text.clone();
    let mut data = emitter.data.clone();
    let mut test = emitter.test.clone();

    let resolve = |symbol: &str| -> Option<u64> {
        if let Some(&off) = emitter.text_labels.get(symbol) {
            return Some(BASE + text_off + off as u64);
        }

        if let Some(&off) = emitter.data_labels.get(symbol) {
            return Some(BASE + data_off + off as u64);
        }

        if let Some(&off) = emitter.test_labels.get(symbol) {
            return Some(BASE + test_off + off as u64);
        }

        None
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

        let section_base = match fixup.section {
            Section::Text => BASE + text_off,
            Section::Data => BASE + data_off,
            Section::Test => BASE + test_off,
        };

        let fixup_addr = section_base + fixup.offset as u64;

        let buffer = match fixup.section {
            Section::Text => &mut text,
            Section::Data => &mut data,
            Section::Test => &mut test,
        };

        match fixup.kind {
            FixupKind::Rel32 | FixupKind::RipRelative32 => {
                let relative = target as i64 - (fixup_addr as i64 + 4);

                if relative < i32::MIN as i64 || relative > i32::MAX as i64 {
                    return Err(format!("相対アドレスが範囲外です: {}", fixup.symbol));
                }

                buffer[fixup.offset..fixup.offset + 4]
                    .copy_from_slice(&(relative as i32).to_le_bytes());
            }

            FixupKind::Abs32 => {
                if target > u32::MAX as u64 {
                    return Err(format!("アドレスが32bitに収まりません: {}", fixup.symbol));
                }

                buffer[fixup.offset..fixup.offset + 4]
                    .copy_from_slice(&(target as u32).to_le_bytes());
            }

            FixupKind::Abs64 => {
                buffer[fixup.offset..fixup.offset + 8].copy_from_slice(&target.to_le_bytes());
            }
        }
    }

    let filesz = if uses_test {
        test_off + test.len() as u64
    } else {
        data_off + data.len() as u64
    };

    let entry = match emitter.text_labels.get("_start") {
        Some(&off) => BASE + text_off + off as u64,
        None => BASE + text_off,
    };

    // ---- セクションヘッダ (test セクションを使ったときだけ) ----
    let mut shdr_bytes: Vec<u8> = Vec::new();
    let mut shstrtab: Vec<u8> = vec![0];
    let mut shnum: u16 = 0;
    let mut shstrndx: u16 = 0;
    let mut shstr_off = 0u64;
    let mut shoff = 0u64;

    {
        let mut name = |s: &str| -> u32 {
            let off = shstrtab.len() as u32;
            shstrtab.extend_from_slice(s.as_bytes());
            shstrtab.push(0);
            off
        };
        let n_text = name(".text");
        let n_data = name(".data");
        let n_test = if uses_test { Some(name("test")) } else { None };
        let n_shstr = name(".shstrtab");

        // .shstrtab 本体は最後のセクションの直後、セクションヘッダテーブルはその後ろ (8 バイト境界)
        shstr_off = filesz;
        shoff = (shstr_off + shstrtab.len() as u64 + 7) & !7;

        let mk = |name: u32, ty: u32, flags: u64, addr: u64, off: u64, size: u64, align: u64| {
            Elf64Shdr {
                sh_name: name,
                sh_type: ty,
                sh_flags: flags,
                sh_addr: addr,
                sh_offset: off,
                sh_size: size,
                sh_link: 0,
                sh_info: 0,
                sh_addralign: align,
                sh_entsize: 0,
            }
        };

        let mut headers = vec![
            mk(0, SHT_NULL, 0, 0, 0, 0, 0),
            mk(n_text, SHT_PROGBITS, SHF_ALLOC | SHF_EXECINSTR, BASE + text_off, text_off, text.len() as u64, 16),
            mk(n_data, SHT_PROGBITS, SHF_ALLOC | SHF_WRITE, BASE + data_off, data_off, data.len() as u64, 8),
        ];

        if let Some(n_test) = n_test {
            headers.push(mk(n_test, SHT_PROGBITS, SHF_ALLOC | SHF_WRITE, BASE + test_off, test_off, test.len() as u64, 8));
        }

        headers.push(mk(n_shstr, SHT_STRTAB, 0, 0, shstr_off, shstrtab.len() as u64, 1));

        for h in &headers {
            shdr_bytes.extend_from_slice(&h.to_bytes());
        }

        shnum = headers.len() as u16;
        shstrndx = shnum - 1;
    }

    let mut ehdr = Elf64Ehdr::new();
    ehdr.e_type = ET_EXEC;
    ehdr.e_entry = entry;
    ehdr.e_phoff = EHDR_SIZE;
    ehdr.e_phnum = 1;
    ehdr.e_shoff = shoff;
    ehdr.e_shnum = shnum;
    ehdr.e_shstrndx = shstrndx;

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

    let mut out = Vec::with_capacity(filesz as usize);
    out.extend_from_slice(&ehdr.to_bytes());
    out.extend_from_slice(&phdr.to_bytes());
    out.extend_from_slice(&text);
    out.extend_from_slice(&data);

    if uses_test {
        out.resize(test_off as usize, 0); // data と test の間のパディング
        out.extend_from_slice(&test);
    }

    debug_assert_eq!(out.len() as u64, shstr_off);
    out.extend_from_slice(&shstrtab);
    out.resize(shoff as usize, 0);
    out.extend_from_slice(&shdr_bytes);

    Ok(out)
}
