//! ELF64 (x86-64 / リトルエンディアン) リロケータブルオブジェクトのパーサ。
//!
//! このモジュールの [`ElfObject`] はリンカ全体の内部表現でもあり、COFF 入力
//! ([`super::coff`]) もここへ変換されてからリンクされる。
//!
//! 入力は信頼できない前提で、範囲外アクセスや巨大なサイズはすべて
//! パニックではなくエラーとして返す。

use super::error::{LinkError, LinkResult};

pub const SHT_NULL: u32 = 0;
pub const SHT_PROGBITS: u32 = 1;
pub const SHT_SYMTAB: u32 = 2;
pub const SHT_STRTAB: u32 = 3;
pub const SHT_RELA: u32 = 4;
pub const SHT_NOBITS: u32 = 8;
pub const SHT_REL: u32 = 9;
pub const SHT_GROUP: u32 = 17;
pub const SHT_SYMTAB_SHNDX: u32 = 18;

pub const SHF_WRITE: u64 = 0x1;
pub const SHF_ALLOC: u64 = 0x2;
pub const SHF_EXECINSTR: u64 = 0x4;
pub const SHF_TLS: u64 = 0x400;

pub const SHN_UNDEF: u16 = 0;
pub const SHN_LORESERVE: u16 = 0xff00;
pub const SHN_ABS: u16 = 0xfff1;
pub const SHN_COMMON: u16 = 0xfff2;
pub const SHN_XINDEX: u16 = 0xffff;

pub const STB_LOCAL: u8 = 0;
pub const STB_GLOBAL: u8 = 1;
pub const STB_WEAK: u8 = 2;

pub const STT_SECTION: u8 = 3;
pub const STT_FILE: u8 = 4;
pub const STT_TLS: u8 = 6;
pub const STT_GNU_IFUNC: u8 = 10;

pub const R_X86_64_NONE: u32 = 0;
pub const R_X86_64_64: u32 = 1;
pub const R_X86_64_PC32: u32 = 2;
pub const R_X86_64_PLT32: u32 = 4;
pub const R_X86_64_GOTPCREL: u32 = 9;
pub const R_X86_64_32: u32 = 10;
pub const R_X86_64_32S: u32 = 11;
pub const R_X86_64_PC64: u32 = 24;
pub const R_X86_64_GOTPCRELX: u32 = 41;
pub const R_X86_64_REX_GOTPCRELX: u32 = 42;

/// COFF の `IMAGE_REL_AMD64_ADDR32NB` (イメージベースからの 32 ビット RVA) に対応する内部用の型。
/// ELF の値とは衝突しない範囲に置いてある。COFF パーサだけが生成する。
pub const R_PE_ADDR32NB: u32 = 0x8000_0001;

pub const GRP_COMDAT: u32 = 1;

const ET_REL: u16 = 1;
const EM_X86_64: u16 = 62;

const EHDR_SIZE: usize = 64;
const SHDR_SIZE: usize = 64;
const SYM_SIZE: usize = 24;
const RELA_SIZE: usize = 24;

/// 1 セクションあたり / alignment の上限 (後段のサイズ計算でオーバーフローしないための制限)
const MAX_SECTION_SIZE: u64 = 1 << 32;
const MAX_ALIGN: u64 = 1 << 24;

#[derive(Debug, Clone)]
pub struct SectionHeader {
    pub name: String,
    pub sh_type: u32,
    pub flags: u64,
    pub offset: u64,
    pub size: u64,
    pub link: u32,
    pub info: u32,
    pub align: u64,
    pub entsize: u64,
}

#[derive(Debug, Clone)]
pub struct Symbol {
    pub name: String,
    pub bind: u8,
    pub typ: u8,
    pub shndx: u16,
    /// 通常はセクション内オフセット。`SHN_COMMON` のシンボルではアライメント。
    pub value: u64,
    pub size: u64,
}

#[derive(Debug, Clone, Copy)]
pub struct Reloc {
    pub offset: u64,
    pub sym: u32,
    pub typ: u32,
    pub addend: i64,
}

#[derive(Debug, Clone)]
pub struct Group {
    pub signature: String,
    pub comdat: bool,
    pub members: Vec<u32>,
}

/// パース済みのオブジェクトファイル。
#[derive(Debug)]
pub struct ElfObject {
    pub name: String,
    data: Vec<u8>,
    pub sections: Vec<SectionHeader>,
    /// index 0 はヌルシンボル
    pub symbols: Vec<Symbol>,
    /// セクション番号ごとのリロケーション (リンク対象 = `SHF_ALLOC` セクションのみ保持)
    pub relocs: Vec<Vec<Reloc>>,
    pub groups: Vec<Group>,
}

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}
fn le64(b: &[u8], o: usize) -> u64 {
    u64::from_le_bytes(b[o..o + 8].try_into().unwrap())
}

/// `data[off .. off+len]` を範囲チェック付きで取り出す。
fn get(data: &[u8], off: u64, len: u64) -> Option<&[u8]> {
    let end = off.checked_add(len)?;
    if end > data.len() as u64 {
        return None;
    }
    Some(&data[off as usize..end as usize])
}

fn read_cstr(tab: &[u8], off: usize) -> Option<String> {
    let rest = tab.get(off..)?;
    let end = rest.iter().position(|&b| b == 0)?;
    Some(String::from_utf8_lossy(&rest[..end]).into_owned())
}

impl ElfObject {
    /// 他の形式 (COFF など) のパーサが、変換済みの内部表現から組み立てるための入口。
    ///
    /// `sections` / `relocs` はセクション番号で引ける形 (index 0 がヌルセクション) で、
    /// `symbols` は index 0 がヌルシンボルであること。範囲の検証は呼び出し側の責任。
    pub(super) fn from_parts(
        name: String,
        data: Vec<u8>,
        sections: Vec<SectionHeader>,
        symbols: Vec<Symbol>,
        relocs: Vec<Vec<Reloc>>,
        groups: Vec<Group>,
    ) -> Self {
        ElfObject {
            name,
            data,
            sections,
            symbols,
            relocs,
            groups,
        }
    }

    pub fn parse(name: impl Into<String>, data: Vec<u8>) -> LinkResult<Self> {
        let name = name.into();
        let perr = |m: String| LinkError::Parse {
            object: name.clone(),
            message: m,
        };
        let uerr = |m: String| LinkError::Unsupported {
            object: name.clone(),
            message: m,
        };

        if data.len() < EHDR_SIZE {
            return Err(perr("file is too small to be an ELF object".into()));
        }
        if &data[0..4] != b"\x7fELF" {
            return Err(perr("not an ELF file (bad magic)".into()));
        }
        if data[4] != 2 {
            return Err(uerr("only ELF64 is supported".into()));
        }
        if data[5] != 1 {
            return Err(uerr("only little-endian ELF is supported".into()));
        }
        match le16(&data, 16) {
            ET_REL => {}
            _ => {
                return Err(perr(
                    "not a relocatable object file (e_type != ET_REL)".into(),
                ));
            }
        }
        if le16(&data, 18) != EM_X86_64 {
            return Err(uerr("only x86-64 objects are supported".into()));
        }

        let shoff = le64(&data, 40);
        let shentsize = le16(&data, 58) as usize;
        let shnum = le16(&data, 60) as usize;
        let shstrndx = le16(&data, 62) as usize;

        if shnum == 0 {
            if shoff != 0 {
                return Err(uerr("extended section numbering is not supported".into()));
            }
            // セクションを持たない空のオブジェクト
            return Ok(ElfObject {
                name,
                data,
                sections: Vec::new(),
                symbols: vec![null_symbol()],
                relocs: Vec::new(),
                groups: Vec::new(),
            });
        }
        if shentsize != SHDR_SIZE {
            return Err(perr(format!("unexpected section header size {shentsize}")));
        }
        if shstrndx == SHN_XINDEX as usize {
            return Err(uerr("extended section string table index".into()));
        }
        if shstrndx >= shnum {
            return Err(perr("section name table index is out of range".into()));
        }
        let table = get(&data, shoff, (shnum * SHDR_SIZE) as u64)
            .ok_or_else(|| perr("section header table is outside the file".into()))?;

        // --- セクションヘッダ ---
        let mut sections = Vec::with_capacity(shnum);
        let mut name_offs = Vec::with_capacity(shnum);
        for i in 0..shnum {
            let h = &table[i * SHDR_SIZE..(i + 1) * SHDR_SIZE];
            let sh = SectionHeader {
                name: String::new(),
                sh_type: le32(h, 4),
                flags: le64(h, 8),
                offset: le64(h, 24),
                size: le64(h, 32),
                link: le32(h, 40),
                info: le32(h, 44),
                align: le64(h, 48),
                entsize: le64(h, 56),
            };
            if sh.sh_type != SHT_NOBITS
                && sh.sh_type != SHT_NULL
                && get(&data, sh.offset, sh.size).is_none()
            {
                return Err(perr(format!(
                    "section {i} extends beyond the end of the file"
                )));
            }
            if sh.size > MAX_SECTION_SIZE {
                return Err(uerr(format!(
                    "section {i} is too large ({:#x} bytes)",
                    sh.size
                )));
            }
            if sh.align > MAX_ALIGN || (sh.align > 1 && !sh.align.is_power_of_two()) {
                return Err(perr(format!(
                    "section {i} has invalid alignment {}",
                    sh.align
                )));
            }
            name_offs.push(le32(h, 0) as usize);
            sections.push(sh);
        }

        let shstr_sec = &sections[shstrndx];
        let shstr = get(&data, shstr_sec.offset, shstr_sec.size)
            .ok_or_else(|| perr("section name table is outside the file".into()))?;
        for (sec, off) in sections.iter_mut().zip(&name_offs) {
            sec.name = read_cstr(shstr, *off)
                .ok_or_else(|| perr("section name is outside the name table".into()))?;
        }

        // --- シンボルテーブル ---
        let mut symtab_idx: Option<usize> = None;
        for (i, s) in sections.iter().enumerate() {
            match s.sh_type {
                SHT_SYMTAB => {
                    if symtab_idx.replace(i).is_some() {
                        return Err(perr("multiple symbol tables".into()));
                    }
                }
                SHT_SYMTAB_SHNDX => {
                    return Err(uerr(
                        "extended section index table (SHT_SYMTAB_SHNDX)".into(),
                    ));
                }
                _ => {}
            }
        }

        let mut symbols = vec![null_symbol()];
        if let Some(si) = symtab_idx {
            let st = &sections[si];
            let strtab_idx = st.link as usize;
            if strtab_idx >= shnum || sections[strtab_idx].sh_type != SHT_STRTAB {
                return Err(perr("symbol table has no valid string table".into()));
            }
            if st.size % SYM_SIZE as u64 != 0 {
                return Err(perr("symbol table size is not a multiple of 24".into()));
            }
            let strtab = get(
                &data,
                sections[strtab_idx].offset,
                sections[strtab_idx].size,
            )
            .ok_or_else(|| perr("string table is outside the file".into()))?;
            let raw = get(&data, st.offset, st.size).unwrap();
            let count = raw.len() / SYM_SIZE;
            symbols = Vec::with_capacity(count.max(1));
            for i in 0..count {
                let e = &raw[i * SYM_SIZE..(i + 1) * SYM_SIZE];
                let info = e[4];
                let shndx = le16(e, 6);
                let sym = Symbol {
                    name: read_cstr(strtab, le32(e, 0) as usize)
                        .ok_or_else(|| perr(format!("symbol {i} has an invalid name offset")))?,
                    bind: info >> 4,
                    typ: info & 0xf,
                    shndx,
                    value: le64(e, 8),
                    size: le64(e, 16),
                };
                if shndx == SHN_XINDEX {
                    return Err(uerr(format!("symbol {i} uses an extended section index")));
                }
                if shndx == SHN_COMMON {
                    if sym.value > MAX_ALIGN || sym.size > MAX_SECTION_SIZE {
                        return Err(uerr(format!("common symbol {i} is too large")));
                    }
                } else if shndx >= SHN_LORESERVE {
                    if shndx != SHN_ABS {
                        return Err(uerr(format!(
                            "symbol {i} has reserved section index {shndx:#x}"
                        )));
                    }
                } else if shndx as usize >= shnum {
                    return Err(perr(format!(
                        "symbol {i} has invalid section index {shndx}"
                    )));
                }
                symbols.push(sym);
            }
            if symbols.is_empty() {
                symbols.push(null_symbol());
            }
        }

        // --- リロケーション ---
        let mut relocs: Vec<Vec<Reloc>> = vec![Vec::new(); shnum];
        for (i, s) in sections.iter().enumerate() {
            if s.sh_type != SHT_RELA && s.sh_type != SHT_REL {
                continue;
            }
            let target = s.info as usize;
            if target >= shnum {
                return Err(perr(format!(
                    "relocation section {i} has invalid target {target}"
                )));
            }
            // デバッグ情報など、出力に載らないセクションへのリロケーションは無視する
            if sections[target].flags & SHF_ALLOC == 0 {
                continue;
            }
            if s.sh_type == SHT_REL {
                return Err(uerr("SHT_REL relocations (only RELA is supported)".into()));
            }
            if Some(s.link as usize) != symtab_idx {
                return Err(perr(format!(
                    "relocation section {i} does not reference the symbol table"
                )));
            }
            if s.size % RELA_SIZE as u64 != 0 {
                return Err(perr(format!("relocation section {i} has a bad size")));
            }
            let raw = get(&data, s.offset, s.size).unwrap();
            for e in raw.chunks_exact(RELA_SIZE) {
                let info = le64(e, 8);
                let r = Reloc {
                    offset: le64(e, 0),
                    sym: (info >> 32) as u32,
                    typ: info as u32,
                    addend: le64(e, 16) as i64,
                };
                if r.sym as usize >= symbols.len() {
                    return Err(perr(format!(
                        "relocation in section {i} references invalid symbol {}",
                        r.sym
                    )));
                }
                relocs[target].push(r);
            }
        }

        // --- COMDAT / セクショングループ ---
        let mut groups = Vec::new();
        for (i, s) in sections.iter().enumerate() {
            if s.sh_type != SHT_GROUP {
                continue;
            }
            let raw = get(&data, s.offset, s.size).unwrap();
            if raw.len() < 4 || !raw.len().is_multiple_of(4) {
                return Err(perr(format!("group section {i} is malformed")));
            }
            let sig_idx = s.info as usize;
            if sig_idx >= symbols.len() {
                return Err(perr(format!(
                    "group section {i} has an invalid signature symbol"
                )));
            }
            let sig = &symbols[sig_idx];
            let signature = if !sig.name.is_empty() {
                sig.name.clone()
            } else if (sig.shndx as usize) < shnum && sig.shndx != SHN_UNDEF {
                sections[sig.shndx as usize].name.clone()
            } else {
                String::new()
            };
            let flags = le32(raw, 0);
            let members: Vec<u32> = raw[4..]
                .chunks_exact(4)
                .map(|c| u32::from_le_bytes(c.try_into().unwrap()))
                .collect();
            if members.iter().any(|&m| m as usize >= shnum) {
                return Err(perr(format!("group section {i} has an invalid member")));
            }
            groups.push(Group {
                signature,
                comdat: flags & GRP_COMDAT != 0,
                members,
            });
        }

        Ok(ElfObject {
            name,
            data,
            sections,
            symbols,
            relocs,
            groups,
        })
    }

    /// セクションの内容 (`SHT_NOBITS` は空)。パース時に範囲を検証済み。
    pub fn section_data(&self, idx: usize) -> &[u8] {
        let s = &self.sections[idx];
        if s.sh_type == SHT_NOBITS || s.sh_type == SHT_NULL {
            return &[];
        }
        &self.data[s.offset as usize..(s.offset + s.size) as usize]
    }
}

fn null_symbol() -> Symbol {
    Symbol {
        name: String::new(),
        bind: STB_LOCAL,
        typ: 0,
        shndx: SHN_UNDEF,
        value: 0,
        size: 0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rejects_empty_and_garbage() {
        assert!(ElfObject::parse("empty.o", Vec::new()).is_err());
        assert!(ElfObject::parse("junk.o", vec![0u8; 200]).is_err());
        assert!(
            ElfObject::parse(
                "text.o",
                b"hello, this is not an object file at all......................................"
                    .to_vec()
            )
            .is_err()
        );
    }

    fn header() -> Vec<u8> {
        let mut h = vec![0u8; 64];
        h[0..4].copy_from_slice(b"\x7fELF");
        h[4] = 2; // ELFCLASS64
        h[5] = 1; // little endian
        h[6] = 1;
        h[16..18].copy_from_slice(&1u16.to_le_bytes()); // ET_REL
        h[18..20].copy_from_slice(&62u16.to_le_bytes()); // x86-64
        h[52..54].copy_from_slice(&64u16.to_le_bytes());
        h
    }

    #[test]
    fn accepts_object_without_sections() {
        let o = ElfObject::parse("nothing.o", header()).unwrap();
        assert!(o.sections.is_empty());
        assert_eq!(o.symbols.len(), 1);
    }

    #[test]
    fn rejects_wrong_class_machine_and_type() {
        let mut h = header();
        h[4] = 1;
        assert!(ElfObject::parse("a.o", h).is_err());

        let mut h = header();
        h[18..20].copy_from_slice(&183u16.to_le_bytes()); // AArch64
        assert!(ElfObject::parse("b.o", h).is_err());

        let mut h = header();
        h[16..18].copy_from_slice(&2u16.to_le_bytes()); // ET_EXEC
        assert!(ElfObject::parse("c.o", h).is_err());
    }

    #[test]
    fn rejects_section_table_outside_file() {
        let mut h = header();
        h[40..48].copy_from_slice(&0x1000u64.to_le_bytes()); // e_shoff
        h[58..60].copy_from_slice(&64u16.to_le_bytes()); // e_shentsize
        h[60..62].copy_from_slice(&5u16.to_le_bytes()); // e_shnum
        assert!(ElfObject::parse("d.o", h).is_err());
    }
}
