//! COFF (x86-64 / `IMAGE_FILE_MACHINE_AMD64`) オブジェクトのパーサ。
//!
//! Windows 系ツールチェーン (mingw の `as` / `gcc -c`、MSVC、clang など) が出力する
//! `.o` / `.obj` を読む。リンカ本体は ELF 向けの内部表現 ([`ElfObject`]) で動いているので、
//! ここでは COFF を次のように ELF 風の表現へ変換する。
//!
//! * セクション番号は 1 始まりなので、index 0 にヌルセクションを挿入して番号を揃える
//! * シンボル番号は COFF では 0 始まり (補助エントリも 1 つ数える) なので、
//!   index 0 にヌルシンボルを挿入し、補助エントリの位置には名前なしのローカルシンボルを置く
//! * COFF のリロケーションは暗黙のアドレンド (命令側に書かれた値) を持つので、
//!   セクション内容から読み出して RELA 相当の `addend` に畳み込む
//! * `IMAGE_REL_AMD64_REL32[_n]` は `P + 4 + n` 基準なので、addend を `-4 - n` ずらして
//!   `R_X86_64_PC32` に直す
//! * COMDAT セクションは [`Group`] に変換する (関連付け COMDAT は主セクションのグループに入れる)
//!
//! 入力は信頼できない前提で、範囲外アクセスはパニックではなくエラーにする。

use std::collections::HashMap;

use super::elf::*;
use super::error::{LinkError, LinkResult};

pub const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;

const FILE_HDR_SIZE: usize = 20;
const SECTION_HDR_SIZE: usize = 40;
const SYMBOL_SIZE: usize = 18;
const RELOC_SIZE: usize = 10;

// セクション特性 (IMAGE_SCN_*)
const SCN_CNT_CODE: u32 = 0x0000_0020;
const SCN_CNT_INITIALIZED_DATA: u32 = 0x0000_0040;
const SCN_CNT_UNINITIALIZED_DATA: u32 = 0x0000_0080;
const SCN_LNK_INFO: u32 = 0x0000_0200;
const SCN_LNK_REMOVE: u32 = 0x0000_0800;
const SCN_LNK_COMDAT: u32 = 0x0000_1000;
const SCN_ALIGN_MASK: u32 = 0x00F0_0000;
const SCN_LNK_NRELOC_OVFL: u32 = 0x0100_0000;
const SCN_MEM_DISCARDABLE: u32 = 0x0200_0000;
const SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const SCN_MEM_WRITE: u32 = 0x8000_0000;

// シンボルの記憶クラス (IMAGE_SYM_CLASS_*)
const SYM_CLASS_EXTERNAL: u8 = 2;
const SYM_CLASS_STATIC: u8 = 3;
const SYM_CLASS_WEAK_EXTERNAL: u8 = 105;
const SYM_CLASS_FILE: u8 = 103;

// COMDAT の選択方式 (IMAGE_COMDAT_SELECT_*)
const COMDAT_SELECT_ASSOCIATIVE: u8 = 5;

// リロケーション型 (IMAGE_REL_AMD64_*)
const REL_ABSOLUTE: u16 = 0;
const REL_ADDR64: u16 = 1;
const REL_ADDR32: u16 = 2;
const REL_ADDR32NB: u16 = 3;
const REL_REL32: u16 = 4;
const REL_REL32_5: u16 = 9;

fn le16(b: &[u8], o: usize) -> u16 {
    u16::from_le_bytes(b[o..o + 2].try_into().unwrap())
}
fn le32(b: &[u8], o: usize) -> u32 {
    u32::from_le_bytes(b[o..o + 4].try_into().unwrap())
}

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

/// 8 バイト固定長の名前 (NUL 終端は省略されうる)。
fn short_name(raw: &[u8]) -> String {
    let end = raw.iter().position(|&b| b == 0).unwrap_or(raw.len());
    String::from_utf8_lossy(&raw[..end]).into_owned()
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

/// `data` が COFF (x86-64) オブジェクトらしいかを先頭のヘッダだけで判定する。
///
/// 通常の COFF は先頭 2 バイトが Machine、`/bigobj` 形式は先頭が `00 00 FF FF`。
pub fn looks_like_coff(data: &[u8]) -> bool {
    if data.len() < FILE_HDR_SIZE {
        return false;
    }
    let machine = le16(data, 0);
    machine == IMAGE_FILE_MACHINE_AMD64 || (machine == 0 && le16(data, 2) == 0xFFFF)
}

/// COMDAT セクションの補助情報 (セクションシンボルの補助エントリ + リーダーシンボル)。
#[derive(Default, Clone)]
struct ComdatInfo {
    selection: Option<u8>,
    /// 関連付け COMDAT の主セクション番号
    associated: Option<u16>,
    /// そのセクション内で定義された最初の外部シンボル名 (グループの署名に使う)
    leader: Option<String>,
}

/// COFF オブジェクトをパースし、リンカ内部表現に変換して返す。
pub fn parse(name: impl Into<String>, data: Vec<u8>) -> LinkResult<ElfObject> {
    let name = name.into();
    let perr = |m: String| LinkError::Parse {
        object: name.clone(),
        message: m,
    };
    let uerr = |m: String| LinkError::Unsupported {
        object: name.clone(),
        message: m,
    };

    if data.len() < FILE_HDR_SIZE {
        return Err(perr("file is too small to be a COFF object".into()));
    }
    let machine = le16(&data, 0);
    if machine == 0 && le16(&data, 2) == 0xFFFF {
        return Err(uerr("COFF bigobj format".into()));
    }
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err(uerr(format!(
            "only x86-64 COFF objects are supported (machine {machine:#x})"
        )));
    }
    let nsec = le16(&data, 2) as usize;
    let symptr = le32(&data, 8) as u64;
    let nsym = le32(&data, 12) as usize;
    let optsz = le16(&data, 16) as u64;

    // --- 文字列テーブル (シンボルテーブルの直後。先頭 4 バイトは自身のサイズ) ---
    let sym_raw: &[u8] = if symptr == 0 || nsym == 0 {
        &[]
    } else {
        get(&data, symptr, nsym as u64 * SYMBOL_SIZE as u64)
            .ok_or_else(|| perr("symbol table is outside the file".into()))?
    };
    let strtab: Vec<u8> = if sym_raw.is_empty() {
        Vec::new()
    } else {
        let str_off = symptr + sym_raw.len() as u64;
        match get(&data, str_off, 4) {
            Some(b) => {
                let size = u32::from_le_bytes(b.try_into().unwrap()) as u64;
                if size < 4 {
                    Vec::new()
                } else {
                    get(&data, str_off, size)
                        .ok_or_else(|| perr("string table is outside the file".into()))?
                        .to_vec()
                }
            }
            None => Vec::new(),
        }
    };

    // --- セクションヘッダ ---
    let sec_table = get(
        &data,
        FILE_HDR_SIZE as u64 + optsz,
        nsec as u64 * SECTION_HDR_SIZE as u64,
    )
    .ok_or_else(|| perr("section header table is outside the file".into()))?;

    struct RawSec {
        relptr: u64,
        nrel: u64,
        chars: u32,
    }
    let mut sections = Vec::with_capacity(nsec + 1);
    sections.push(SectionHeader {
        name: String::new(),
        sh_type: SHT_NULL,
        flags: 0,
        offset: 0,
        size: 0,
        link: 0,
        info: 0,
        align: 0,
        entsize: 0,
    });
    let mut raw_secs = vec![RawSec {
        relptr: 0,
        nrel: 0,
        chars: 0,
    }];
    for i in 0..nsec {
        let h = &sec_table[i * SECTION_HDR_SIZE..(i + 1) * SECTION_HDR_SIZE];
        let raw_name = &h[0..8];
        let sec_name = if raw_name[0] == b'/' {
            // "/123": 文字列テーブル内オフセット (10 進)
            let digits = short_name(&raw_name[1..]);
            let off: usize = digits
                .parse()
                .map_err(|_| perr(format!("section {} has a malformed long name", i + 1)))?;
            read_cstr(&strtab, off).ok_or_else(|| {
                perr(format!(
                    "section {} name is outside the string table",
                    i + 1
                ))
            })?
        } else {
            short_name(raw_name)
        };
        let vsize = le32(h, 8) as u64;
        let rawsize = le32(h, 16) as u64;
        let rawptr = le32(h, 20) as u64;
        let relptr = le32(h, 24) as u64;
        let nrel = le16(h, 32) as u64;
        let chars = le32(h, 36);

        let discard = chars & (SCN_LNK_REMOVE | SCN_LNK_INFO | SCN_MEM_DISCARDABLE) != 0;
        let is_bss = chars & SCN_CNT_UNINITIALIZED_DATA != 0
            && chars & (SCN_CNT_INITIALIZED_DATA | SCN_CNT_CODE) == 0;
        let size = if rawsize != 0 { rawsize } else { vsize };

        if !is_bss && rawsize > 0 {
            if rawptr == 0 {
                return Err(perr(format!(
                    "section {} (`{sec_name}') has contents but no file offset",
                    i + 1
                )));
            }
            if get(&data, rawptr, rawsize).is_none() {
                return Err(perr(format!(
                    "section {} (`{sec_name}') extends beyond the end of the file",
                    i + 1
                )));
            }
        }

        let align_bits = (chars & SCN_ALIGN_MASK) >> 20;
        let align = if align_bits == 0 {
            1
        } else {
            1u64 << (align_bits - 1)
        };

        let mut flags = 0;
        if !discard {
            flags |= SHF_ALLOC;
            if chars & (SCN_MEM_EXECUTE | SCN_CNT_CODE) != 0 {
                flags |= SHF_EXECINSTR;
            }
            if chars & SCN_MEM_WRITE != 0 {
                flags |= SHF_WRITE;
            }
        }
        sections.push(SectionHeader {
            name: sec_name,
            sh_type: if is_bss { SHT_NOBITS } else { SHT_PROGBITS },
            flags,
            offset: if is_bss { 0 } else { rawptr },
            size: if is_bss { size } else { rawsize },
            link: 0,
            info: 0,
            align,
            entsize: 0,
        });
        raw_secs.push(RawSec {
            relptr,
            nrel,
            chars,
        });
    }

    // --- シンボルテーブル ---
    // COFF のシンボル番号 k は、ここでは symbols[k + 1] になる。
    let mut symbols = vec![null_symbol()];
    let mut comdat: Vec<ComdatInfo> = vec![ComdatInfo::default(); nsec + 1];
    let mut i = 0usize;
    while i < nsym {
        let e = &sym_raw[i * SYMBOL_SIZE..(i + 1) * SYMBOL_SIZE];
        let sym_name = if e[0..4] == [0, 0, 0, 0] {
            read_cstr(&strtab, le32(e, 4) as usize)
                .ok_or_else(|| perr(format!("symbol {i} has an invalid name offset")))?
        } else {
            short_name(&e[0..8])
        };
        let value = le32(e, 8) as u64;
        let scn = le16(e, 12) as i16;
        let class = e[16];
        let naux = e[17] as usize;
        if i + 1 + naux > nsym {
            return Err(perr(format!(
                "symbol {i} has auxiliary records past the end"
            )));
        }
        if scn > 0 && scn as usize > nsec {
            return Err(perr(format!("symbol {i} has invalid section number {scn}")));
        }

        let (bind, typ, shndx, val, size) = match (class, scn) {
            (SYM_CLASS_EXTERNAL, s) if s > 0 => (STB_GLOBAL, 0, s as u16, value, 0),
            (SYM_CLASS_EXTERNAL, 0) => {
                if value > 0 {
                    // 未初期化の共通シンボル。value がサイズ。
                    let align = value.next_power_of_two().min(16);
                    (STB_GLOBAL, 0, SHN_COMMON, align, value)
                } else {
                    (STB_GLOBAL, 0, SHN_UNDEF, 0, 0)
                }
            }
            (SYM_CLASS_EXTERNAL, -1) => (STB_GLOBAL, 0, SHN_ABS, value, 0),
            // 弱い外部シンボル: 定義が無ければ 0 に解決する (代替シンボルは使わない)
            (SYM_CLASS_WEAK_EXTERNAL, _) => (STB_WEAK, 0, SHN_UNDEF, 0, 0),
            (c, s) if s > 0 => {
                let typ = if c == SYM_CLASS_STATIC && naux > 0 && value == 0 {
                    STT_SECTION
                } else {
                    0
                };
                (STB_LOCAL, typ, s as u16, value, 0)
            }
            (_, -1) => (STB_LOCAL, 0, SHN_ABS, value, 0),
            (c, _) => (
                STB_LOCAL,
                if c == SYM_CLASS_FILE { STT_FILE } else { 0 },
                SHN_UNDEF,
                0,
                0,
            ),
        };

        // COMDAT 情報の収集
        if scn > 0 {
            let s = scn as usize;
            if raw_secs[s].chars & SCN_LNK_COMDAT != 0 {
                if class == SYM_CLASS_STATIC && naux > 0 && value == 0 {
                    let aux = &sym_raw[(i + 1) * SYMBOL_SIZE..(i + 2) * SYMBOL_SIZE];
                    comdat[s].selection = Some(aux[14]);
                    if aux[14] == COMDAT_SELECT_ASSOCIATIVE {
                        comdat[s].associated = Some(le16(aux, 12));
                    }
                } else if class == SYM_CLASS_EXTERNAL
                    && comdat[s].leader.is_none()
                    && !sym_name.is_empty()
                {
                    comdat[s].leader = Some(sym_name.clone());
                }
            }
        }

        symbols.push(Symbol {
            name: sym_name,
            bind,
            typ,
            shndx,
            value: val,
            size,
        });
        for _ in 0..naux {
            symbols.push(null_symbol());
        }
        i += 1 + naux;
    }

    // --- リロケーション ---
    let mut relocs: Vec<Vec<Reloc>> = vec![Vec::new(); nsec + 1];
    for si in 1..=nsec {
        if sections[si].flags & SHF_ALLOC == 0 {
            continue;
        }
        let RawSec {
            relptr,
            mut nrel,
            chars,
        } = {
            let r = &raw_secs[si];
            RawSec {
                relptr: r.relptr,
                nrel: r.nrel,
                chars: r.chars,
            }
        };
        let mut rel_start = relptr;
        if chars & SCN_LNK_NRELOC_OVFL != 0 {
            // 拡張リロケーション数: 先頭レコードの VirtualAddress が本当の個数
            let first = get(&data, relptr, RELOC_SIZE as u64)
                .ok_or_else(|| perr(format!("relocations of section {si} are outside the file")))?;
            nrel = le32(first, 0) as u64;
            rel_start = relptr + RELOC_SIZE as u64;
            if nrel == 0 {
                return Err(perr(format!(
                    "section {si} has an invalid relocation count"
                )));
            }
            nrel -= 1; // 先頭レコード自身を数えている
        }
        if nrel == 0 {
            continue;
        }
        let raw = get(&data, rel_start, nrel * RELOC_SIZE as u64)
            .ok_or_else(|| perr(format!("relocations of section {si} are outside the file")))?;
        let sec = &sections[si];
        let sec_data: &[u8] = if sec.sh_type == SHT_NOBITS {
            &[]
        } else {
            get(&data, sec.offset, sec.size).unwrap_or(&[])
        };
        for e in raw.chunks_exact(RELOC_SIZE) {
            let offset = le32(e, 0) as u64;
            let sym_idx = le32(e, 4) as u64;
            let ty = le16(e, 8);
            if sym_idx >= nsym as u64 {
                return Err(perr(format!(
                    "relocation in section {si} references invalid symbol {sym_idx}"
                )));
            }
            let (typ, addend) = convert_reloc(ty, sec_data, offset, sec.sh_type == SHT_NOBITS)
                .map_err(|m| match m {
                    RelocErr::Unsupported(t) => uerr(format!(
                        "COFF relocation type {t:#x} in section `{}'",
                        sec.name
                    )),
                    RelocErr::OutOfRange => perr(format!(
                        "relocation offset {offset:#x} is outside section `{}'",
                        sec.name
                    )),
                })?;
            relocs[si].push(Reloc {
                offset,
                sym: sym_idx as u32 + 1,
                typ,
                addend,
            });
        }
    }

    // --- COMDAT → グループ ---
    let mut groups: Vec<Group> = Vec::new();
    let mut group_of: HashMap<u16, usize> = HashMap::new();
    let mut pending: Vec<(u16, u16)> = Vec::new(); // (関連付け COMDAT のセクション, 主セクション)
    for s in 1..=nsec {
        if raw_secs[s].chars & SCN_LNK_COMDAT == 0 || sections[s].flags & SHF_ALLOC == 0 {
            continue;
        }
        let info = &comdat[s];
        if let (Some(COMDAT_SELECT_ASSOCIATIVE), Some(main)) = (info.selection, info.associated) {
            pending.push((s as u16, main));
            continue;
        }
        let signature = info
            .leader
            .clone()
            .unwrap_or_else(|| sections[s].name.clone());
        group_of.insert(s as u16, groups.len());
        groups.push(Group {
            signature,
            comdat: true,
            members: vec![s as u32],
        });
    }
    // 主セクションの解決は連鎖しうるので、進展がなくなるまで繰り返す
    loop {
        let before = pending.len();
        pending.retain(|&(s, main)| match group_of.get(&main).copied() {
            Some(gi) => {
                groups[gi].members.push(s as u32);
                group_of.insert(s, gi);
                false
            }
            None => true,
        });
        if pending.is_empty() || pending.len() == before {
            break;
        }
    }
    // 主セクションが見つからない関連付け COMDAT は通常のセクションとして残る

    Ok(ElfObject::from_parts(
        name, data, sections, symbols, relocs, groups,
    ))
}

enum RelocErr {
    Unsupported(u16),
    OutOfRange,
}

/// COFF のリロケーション型を内部の型 + addend に変換する。
///
/// `sec_data` は暗黙のアドレンドを読むためのセクション内容 (NOBITS なら空)。
fn convert_reloc(
    ty: u16,
    sec_data: &[u8],
    off: u64,
    nobits: bool,
) -> std::result::Result<(u32, i64), RelocErr> {
    let width = match ty {
        REL_ABSOLUTE => 0,
        REL_ADDR64 => 8,
        REL_ADDR32 | REL_ADDR32NB | REL_REL32..=REL_REL32_5 => 4,
        t => return Err(RelocErr::Unsupported(t)),
    };
    let implicit = |w: u64| -> std::result::Result<i64, RelocErr> {
        if nobits {
            return Ok(0);
        }
        let b = get(sec_data, off, w).ok_or(RelocErr::OutOfRange)?;
        Ok(match w {
            8 => i64::from_le_bytes(b.try_into().unwrap()),
            _ => i32::from_le_bytes(b.try_into().unwrap()) as i64,
        })
    };
    if width > 0
        && off
            .checked_add(width)
            .is_none_or(|e| e > sec_data.len() as u64)
        && !nobits
    {
        return Err(RelocErr::OutOfRange);
    }
    Ok(match ty {
        REL_ABSOLUTE => (R_X86_64_NONE, 0),
        REL_ADDR64 => (R_X86_64_64, implicit(8)?),
        REL_ADDR32 => (R_X86_64_32, implicit(4)?),
        REL_ADDR32NB => (R_PE_ADDR32NB, implicit(4)?),
        // REL32 は (P + 4) 基準、REL32_n は (P + 4 + n) 基準
        t => (R_X86_64_PC32, implicit(4)? - 4 - (t - REL_REL32) as i64),
    })
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;

    /// 最小の COFF オブジェクト: `.text` 1 本 (`call foo` 相当の REL32 つき) + 外部シンボル 2 つ。
    pub(in super::super) fn tiny_object() -> Vec<u8> {
        let mut o = Vec::new();
        // ファイルヘッダ
        o.extend_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
        o.extend_from_slice(&1u16.to_le_bytes()); // NumberOfSections
        o.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp
        let symptr_pos = o.len();
        o.extend_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable (後で埋める)
        o.extend_from_slice(&2u32.to_le_bytes()); // NumberOfSymbols
        o.extend_from_slice(&0u16.to_le_bytes()); // SizeOfOptionalHeader
        o.extend_from_slice(&0u16.to_le_bytes()); // Characteristics
        // セクションヘッダ (.text)
        let text_off = (FILE_HDR_SIZE + SECTION_HDR_SIZE) as u32;
        let code = [0xE8u8, 0, 0, 0, 0, 0xC3]; // call rel32; ret
        let rel_off = text_off + code.len() as u32;
        o.extend_from_slice(b".text\0\0\0");
        o.extend_from_slice(&0u32.to_le_bytes()); // VirtualSize
        o.extend_from_slice(&0u32.to_le_bytes()); // VirtualAddress
        o.extend_from_slice(&(code.len() as u32).to_le_bytes()); // SizeOfRawData
        o.extend_from_slice(&text_off.to_le_bytes());
        o.extend_from_slice(&rel_off.to_le_bytes());
        o.extend_from_slice(&0u32.to_le_bytes()); // PointerToLinenumbers
        o.extend_from_slice(&1u16.to_le_bytes()); // NumberOfRelocations
        o.extend_from_slice(&0u16.to_le_bytes());
        o.extend_from_slice(&0x6030_0020u32.to_le_bytes()); // CODE|ALIGN_4|EXEC|READ
        // 本体
        o.extend_from_slice(&code);
        // リロケーション: offset 1, symbol 1 (foo), REL32
        o.extend_from_slice(&1u32.to_le_bytes());
        o.extend_from_slice(&1u32.to_le_bytes());
        o.extend_from_slice(&REL_REL32.to_le_bytes());
        // シンボルテーブル
        let symptr = o.len() as u32;
        o[symptr_pos..symptr_pos + 4].copy_from_slice(&symptr.to_le_bytes());
        for (nm, scn) in [(b"_start\0\0", 1i16), (b"foo\0\0\0\0\0", 0)] {
            o.extend_from_slice(nm);
            o.extend_from_slice(&0u32.to_le_bytes());
            o.extend_from_slice(&scn.to_le_bytes());
            o.extend_from_slice(&0x20u16.to_le_bytes());
            o.push(SYM_CLASS_EXTERNAL);
            o.push(0);
        }
        o.extend_from_slice(&4u32.to_le_bytes()); // 空の文字列テーブル
        o
    }

    #[test]
    fn parses_minimal_object() {
        let obj = parse("tiny.obj", tiny_object()).unwrap();
        assert_eq!(obj.sections.len(), 2); // ヌル + .text
        assert_eq!(obj.sections[1].name, ".text");
        assert_eq!(obj.sections[1].align, 4);
        assert_eq!(obj.symbols.len(), 3);
        assert_eq!(obj.symbols[1].name, "_start");
        assert_eq!(obj.symbols[1].shndx, 1);
        assert_eq!(obj.symbols[2].shndx, SHN_UNDEF);
        let r = obj.relocs[1][0];
        assert_eq!(r.typ, R_X86_64_PC32);
        assert_eq!(r.sym, 2);
        assert_eq!(r.addend, -4);
    }

    #[test]
    fn rejects_garbage_and_wrong_machine() {
        assert!(parse("e.obj", Vec::new()).is_err());
        let mut o = tiny_object();
        o[0..2].copy_from_slice(&0x014cu16.to_le_bytes()); // i386
        assert!(parse("x86.obj", o).is_err());
        // 切り詰められたファイル
        let o = tiny_object();
        assert!(parse("cut.obj", o[..50].to_vec()).is_err());
    }

    #[test]
    fn detects_coff() {
        assert!(looks_like_coff(&tiny_object()));
        assert!(!looks_like_coff(
            b"\x7fELF\x02\x01\x01\0\0\0\0\0\0\0\0\0\x01\0>\0"
        ));
        assert!(!looks_like_coff(&[0u8; 200]));
    }
}
