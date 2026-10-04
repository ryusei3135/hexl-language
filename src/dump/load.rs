//! x86-64 の ELF64 / PE32+ (64bit PE) から `.text` / `.data` セクションを取り出し、
//! 16バイトごとに 16進ダンプ表示するAPI。
//!
//! ```ignore
//! // 標準出力へ表示
//! dump::load::dump("a.out", "text")?;
//! // 文字列として受け取る
//! let s = dump::load::dump_to_string("a.exe", "data")?;
//! ```

use std::convert::TryFrom;
use std::fmt::{self, Write as _};
use std::fs;
use std::io;

/// 1行に表示するバイト数
const WIDTH: usize = 16;

// ---------------------------------------------------------------------------
// 型
// ---------------------------------------------------------------------------

/// 表示対象のセクション
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SectionKind {
    Text,
    Data,
}

impl SectionKind {
    /// `"text"` / `"data"`（先頭の `.` と大文字小文字は無視）から変換する
    pub fn parse(section: &str) -> Result<Self, DumpError> {
        match section.trim().trim_start_matches('.').to_ascii_lowercase().as_str() {
            "text" => Ok(SectionKind::Text),
            "data" => Ok(SectionKind::Data),
            _ => Err(DumpError::InvalidArgument(format!(
                "section は \"text\" か \"data\" を指定してください: {:?}",
                section
            ))),
        }
    }

    fn section_name(self) -> &'static str {
        match self {
            SectionKind::Text => ".text",
            SectionKind::Data => ".data",
        }
    }
}

/// 対応しているファイル形式
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Format {
    Elf64,
    Pe64,
}

/// 取り出したセクションの中身
#[derive(Debug, Clone)]
pub struct SectionData {
    pub format: Format,
    pub name: String,
    /// 仮想アドレス（ELF: sh_addr / PE: ImageBase + VirtualAddress）
    pub address: u64,
    /// ファイル内オフセット
    pub file_offset: u64,
    pub bytes: Vec<u8>,
}

#[derive(Debug)]
pub enum DumpError {
    Io(io::Error),
    InvalidArgument(String),
    UnsupportedFormat(String),
    Malformed(String),
    SectionNotFound(String),
}

impl fmt::Display for DumpError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            DumpError::Io(e) => write!(f, "I/Oエラー: {}", e),
            DumpError::InvalidArgument(m) => write!(f, "引数エラー: {}", m),
            DumpError::UnsupportedFormat(m) => write!(f, "未対応の形式: {}", m),
            DumpError::Malformed(m) => write!(f, "ファイルが壊れています: {}", m),
            DumpError::SectionNotFound(n) => write!(f, "セクション {} が見つかりません", n),
        }
    }
}

impl std::error::Error for DumpError {}

impl From<io::Error> for DumpError {
    fn from(e: io::Error) -> Self {
        DumpError::Io(e)
    }
}

// ---------------------------------------------------------------------------
// 公開API
// ---------------------------------------------------------------------------

/// ファイルを読み、指定セクションを16バイトずつ標準出力へ表示する。
///
/// * `file_name` : ELF64(x86-64) または PE32+(x64) のパス
/// * `section`   : `"text"` か `"data"`
pub fn dump(file_name: &str, section: &str) -> Result<(), DumpError> {
    use std::io::Write;
    let text = dump_to_string(file_name, section)?;
    // print! だと pipe が閉じたときに panic するため write_all でエラーとして返す
    io::stdout().lock().write_all(text.as_bytes())?;
    Ok(())
}

/// `dump` と同じ内容を `String` で返す。
pub fn dump_to_string(file_name: &str, section: &str) -> Result<String, DumpError> {
    let data = extract_section(file_name, section)?;
    Ok(format_hexdump(&data))
}

/// ファイルから指定セクションのバイト列とメタ情報を取り出す。
pub fn extract_section(file_name: &str, section: &str) -> Result<SectionData, DumpError> {
    let kind = SectionKind::parse(section)?; // 先に引数を検証
    let image = fs::read(file_name)?;
    parse_section(&image, kind)
}

/// メモリ上のイメージから指定セクションを取り出す（形式は自動判別）。
pub fn parse_section(image: &[u8], kind: SectionKind) -> Result<SectionData, DumpError> {
    if image.starts_with(b"\x7FELF") {
        parse_elf64(image, kind)
    } else if image.starts_with(b"MZ") {
        parse_pe64(image, kind)
    } else {
        Err(DumpError::UnsupportedFormat(
            "ELF でも PE でもありません（マジックナンバー不一致）".into(),
        ))
    }
}

/// 16バイトごとの 16進ダンプ文字列を作る。
pub fn format_hexdump(s: &SectionData) -> String {
    let mut out = String::new();
    let _ = writeln!(
        out,
        "; {:?} section {} : addr=0x{:016X} file_offset=0x{:X} size={} bytes",
        s.format,
        s.name,
        s.address,
        s.file_offset,
        s.bytes.len()
    );

    for (i, chunk) in s.bytes.chunks(WIDTH).enumerate() {
        let addr = s.address.wrapping_add((i * WIDTH) as u64);
        let _ = write!(out, "{:016X}  ", addr);

        for col in 0..WIDTH {
            match chunk.get(col) {
                Some(b) => {
                    let _ = write!(out, "{:02X} ", b);
                }
                None => out.push_str("   "),
            }
            if col == 7 {
                out.push(' '); // 8バイトごとに区切り
            }
        }

        out.push_str(" |");
        for &b in chunk {
            out.push(if b.is_ascii_graphic() || b == b' ' { b as char } else { '.' });
        }
        out.push_str("|\n");
    }
    out
}

// ---------------------------------------------------------------------------
// 境界チェック付きの読み出しヘルパ
// ---------------------------------------------------------------------------

fn malformed(msg: &str) -> DumpError {
    DumpError::Malformed(msg.to_string())
}

fn slice(b: &[u8], off: u64, len: u64) -> Result<&[u8], DumpError> {
    let end = off.checked_add(len).ok_or_else(|| malformed("オフセットがオーバーフロー"))?;
    let o = usize::try_from(off).map_err(|_| malformed("オフセットが大きすぎます"))?;
    let e = usize::try_from(end).map_err(|_| malformed("オフセットが大きすぎます"))?;
    b.get(o..e).ok_or_else(|| malformed("ファイル範囲外を参照しています"))
}

fn u16_at(b: &[u8], off: u64) -> Result<u16, DumpError> {
    Ok(u16::from_le_bytes(slice(b, off, 2)?.try_into().unwrap()))
}
fn u32_at(b: &[u8], off: u64) -> Result<u32, DumpError> {
    Ok(u32::from_le_bytes(slice(b, off, 4)?.try_into().unwrap()))
}
fn u64_at(b: &[u8], off: u64) -> Result<u64, DumpError> {
    Ok(u64::from_le_bytes(slice(b, off, 8)?.try_into().unwrap()))
}

fn cstr_at(b: &[u8], off: u64) -> Result<&[u8], DumpError> {
    let o = usize::try_from(off).map_err(|_| malformed("文字列オフセットが大きすぎます"))?;
    let rest = b.get(o..).ok_or_else(|| malformed("文字列オフセットが範囲外"))?;
    let end = rest.iter().position(|&c| c == 0).unwrap_or(rest.len());
    Ok(&rest[..end])
}

// ---------------------------------------------------------------------------
// ELF64
// ---------------------------------------------------------------------------

const EM_X86_64: u16 = 62;
const SHT_NOBITS: u32 = 8;
const ELF_SHDR_SIZE: u64 = 64;

fn parse_elf64(b: &[u8], kind: SectionKind) -> Result<SectionData, DumpError> {
    // e_ident: [4]=EI_CLASS(2=64bit), [5]=EI_DATA(1=little endian)
    let ident = slice(b, 0, 16)?;
    if ident[4] != 2 {
        return Err(DumpError::UnsupportedFormat("ELF64 ではありません（32bit ELF）".into()));
    }
    if ident[5] != 1 {
        return Err(DumpError::UnsupportedFormat("リトルエンディアンの ELF のみ対応です".into()));
    }
    let machine = u16_at(b, 18)?;
    if machine != EM_X86_64 {
        return Err(DumpError::UnsupportedFormat(format!(
            "x86-64 ではありません (e_machine={})",
            machine
        )));
    }

    let shoff = u64_at(b, 40)?;
    let shentsize = u64::from(u16_at(b, 58)?);
    let shnum = u64::from(u16_at(b, 60)?);
    let shstrndx = u64::from(u16_at(b, 62)?);

    if shoff == 0 || shnum == 0 {
        return Err(malformed("セクションヘッダテーブルがありません（strip / sstrip 済みの可能性）"));
    }
    if shentsize < ELF_SHDR_SIZE {
        return Err(malformed("e_shentsize が小さすぎます"));
    }
    if shstrndx >= shnum {
        return Err(malformed("e_shstrndx が範囲外です"));
    }

    // セクション名文字列テーブル
    let shdr = |i: u64| -> Result<u64, DumpError> {
        shoff
            .checked_add(i.checked_mul(shentsize).ok_or_else(|| malformed("overflow"))?)
            .ok_or_else(|| malformed("overflow"))
    };
    let str_hdr = shdr(shstrndx)?;
    let str_off = u64_at(b, str_hdr + 24)?;
    let str_size = u64_at(b, str_hdr + 32)?;
    let strtab = slice(b, str_off, str_size)?;

    let want = kind.section_name().as_bytes();
    for i in 0..shnum {
        let h = shdr(i)?;
        let name_off = u64::from(u32_at(b, h)?);
        let name = cstr_at(strtab, name_off)?;
        if name != want {
            continue;
        }

        let sh_type = u32_at(b, h + 4)?;
        let addr = u64_at(b, h + 16)?;
        let off = u64_at(b, h + 24)?;
        let size = u64_at(b, h + 32)?;

        let bytes = if sh_type == SHT_NOBITS {
            Vec::new() // ファイル上に実体なし
        } else {
            slice(b, off, size)?.to_vec()
        };
        return Ok(SectionData {
            format: Format::Elf64,
            name: kind.section_name().to_string(),
            address: addr,
            file_offset: off,
            bytes,
        });
    }
    Err(DumpError::SectionNotFound(kind.section_name().to_string()))
}

// ---------------------------------------------------------------------------
// PE32+ (64bit PE)
// ---------------------------------------------------------------------------

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;
const PE32_PLUS_MAGIC: u16 = 0x020B;
const PE_SECTION_HDR_SIZE: u64 = 40;

fn parse_pe64(b: &[u8], kind: SectionKind) -> Result<SectionData, DumpError> {
    let pe_off = u64::from(u32_at(b, 0x3C)?); // e_lfanew
    if slice(b, pe_off, 4)? != b"PE\0\0" {
        return Err(DumpError::UnsupportedFormat("PE シグネチャが見つかりません".into()));
    }

    // COFF File Header (20 bytes)
    let coff = pe_off + 4;
    let machine = u16_at(b, coff)?;
    if machine != IMAGE_FILE_MACHINE_AMD64 {
        return Err(DumpError::UnsupportedFormat(format!(
            "x64 ではありません (Machine=0x{:04X})",
            machine
        )));
    }
    let num_sections = u64::from(u16_at(b, coff + 2)?);
    let opt_size = u64::from(u16_at(b, coff + 16)?);

    // Optional Header
    let opt = coff + 20;
    let magic = u16_at(b, opt)?;
    if magic != PE32_PLUS_MAGIC {
        return Err(DumpError::UnsupportedFormat(format!(
            "PE32+ (64bit) ではありません (Magic=0x{:04X})",
            magic
        )));
    }
    let image_base = u64_at(b, opt + 24)?;

    // Section Table
    let table = opt + opt_size;
    let want = kind.section_name().as_bytes();
    for i in 0..num_sections {
        let h = table + i * PE_SECTION_HDR_SIZE;
        let raw_name = slice(b, h, 8)?;
        let end = raw_name.iter().position(|&c| c == 0).unwrap_or(8);
        if &raw_name[..end] != want {
            continue;
        }

        let virtual_size = u64::from(u32_at(b, h + 8)?);
        let virtual_address = u64::from(u32_at(b, h + 12)?);
        let raw_size = u64::from(u32_at(b, h + 16)?);
        let raw_ptr = u64::from(u32_at(b, h + 20)?);

        // raw_size はファイルアライメントで切り上げられているため、
        // VirtualSize があればそちらを上限にしてパディングを除く
        let size = if virtual_size != 0 { virtual_size.min(raw_size) } else { raw_size };
        let bytes = if raw_ptr == 0 { Vec::new() } else { slice(b, raw_ptr, size)?.to_vec() };

        return Ok(SectionData {
            format: Format::Pe64,
            name: kind.section_name().to_string(),
            address: image_base.wrapping_add(virtual_address),
            file_offset: raw_ptr,
            bytes,
        });
    }
    Err(DumpError::SectionNotFound(kind.section_name().to_string()))
}

// ---------------------------------------------------------------------------
// テスト
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;

    /// 最小の ELF64 を組み立てる: .text(0x400000) と .data(0x600000)
    fn make_elf(machine: u16, class: u8) -> Vec<u8> {
        let text: Vec<u8> = (0u8..20).collect(); // 20バイト → 2行目は端数
        let data = b"Hello, data!".to_vec();
        let shstr = b"\0.text\0.data\0.shstrtab\0".to_vec();

        let mut f = vec![0u8; 64];
        f[0..4].copy_from_slice(b"\x7FELF");
        f[4] = class;
        f[5] = 1;
        f[6] = 1;
        f[18..20].copy_from_slice(&machine.to_le_bytes());

        let text_off = f.len() as u64;
        f.extend(&text);
        let data_off = f.len() as u64;
        f.extend(&data);
        let str_off = f.len() as u64;
        f.extend(&shstr);
        while f.len() % 8 != 0 {
            f.push(0);
        }
        let shoff = f.len() as u64;

        let mut sh = |name: u32, ty: u32, addr: u64, off: u64, size: u64| {
            let mut h = vec![0u8; 64];
            h[0..4].copy_from_slice(&name.to_le_bytes());
            h[4..8].copy_from_slice(&ty.to_le_bytes());
            h[16..24].copy_from_slice(&addr.to_le_bytes());
            h[24..32].copy_from_slice(&off.to_le_bytes());
            h[32..40].copy_from_slice(&size.to_le_bytes());
            f.extend(h);
        };
        sh(0, 0, 0, 0, 0);
        sh(1, 1, 0x400000, text_off, text.len() as u64);
        sh(7, 1, 0x600000, data_off, data.len() as u64);
        sh(13, 3, 0, str_off, shstr.len() as u64);

        f[40..48].copy_from_slice(&shoff.to_le_bytes());
        f[58..60].copy_from_slice(&64u16.to_le_bytes());
        f[60..62].copy_from_slice(&4u16.to_le_bytes());
        f[62..64].copy_from_slice(&3u16.to_le_bytes());
        f
    }

    /// 最小の PE32+ を組み立てる: .text(RVA 0x1000) と .data(RVA 0x2000)
    fn make_pe(machine: u16, magic: u16) -> Vec<u8> {
        let mut f = vec![0u8; 0x80];
        f[0..2].copy_from_slice(b"MZ");
        f[0x3C..0x40].copy_from_slice(&0x80u32.to_le_bytes());
        f.extend(b"PE\0\0");
        let mut coff = vec![0u8; 20];
        coff[0..2].copy_from_slice(&machine.to_le_bytes());
        coff[2..4].copy_from_slice(&2u16.to_le_bytes());
        coff[16..18].copy_from_slice(&0xF0u16.to_le_bytes());
        f.extend(coff);
        let mut opt = vec![0u8; 0xF0];
        opt[0..2].copy_from_slice(&magic.to_le_bytes());
        opt[24..32].copy_from_slice(&0x1_4000_0000u64.to_le_bytes());
        f.extend(opt);

        let sec = |name: &[u8], vsize: u32, va: u32, rsize: u32, rptr: u32| {
            let mut h = vec![0u8; 40];
            h[..name.len()].copy_from_slice(name);
            h[8..12].copy_from_slice(&vsize.to_le_bytes());
            h[12..16].copy_from_slice(&va.to_le_bytes());
            h[16..20].copy_from_slice(&rsize.to_le_bytes());
            h[20..24].copy_from_slice(&rptr.to_le_bytes());
            h
        };
        // ファイルアライメント 0x200 想定。.text は実サイズ 18、raw は 0x200
        f.extend(sec(b".text", 18, 0x1000, 0x200, 0x400));
        f.extend(sec(b".data", 8, 0x2000, 0x200, 0x600));
        f.resize(0x400, 0);
        let mut text = vec![0xCCu8; 0x200];
        for (i, b) in text.iter_mut().take(18).enumerate() {
            *b = 0x90 + i as u8;
        }
        f.extend(text);
        let mut data = vec![0u8; 0x200];
        data[..8].copy_from_slice(b"PEdata!!");
        f.extend(data);
        f
    }

    #[test]
    fn elf_text_and_data() {
        let img = make_elf(EM_X86_64, 2);
        let t = parse_section(&img, SectionKind::Text).unwrap();
        assert_eq!(t.address, 0x400000);
        assert_eq!(t.bytes, (0u8..20).collect::<Vec<_>>());
        let d = parse_section(&img, SectionKind::Data).unwrap();
        assert_eq!(d.bytes, b"Hello, data!");

        let s = format_hexdump(&t);
        let lines: Vec<_> = s.lines().collect();
        assert_eq!(lines.len(), 3); // ヘッダ + 16バイト行 + 端数行
        assert!(lines[1].starts_with("0000000000400000  00 01 02"));
        assert!(lines[2].starts_with("0000000000400010  10 11 12 13"));
    }

    #[test]
    fn pe_text_and_data() {
        let img = make_pe(IMAGE_FILE_MACHINE_AMD64, PE32_PLUS_MAGIC);
        let t = parse_section(&img, SectionKind::Text).unwrap();
        assert_eq!(t.address, 0x1_4000_1000);
        assert_eq!(t.bytes.len(), 18); // パディングは含まない
        assert_eq!(t.bytes[0], 0x90);
        let d = parse_section(&img, SectionKind::Data).unwrap();
        assert_eq!(d.bytes, b"PEdata!!");
    }

    #[test]
    fn rejects_non_x64_and_32bit() {
        assert!(matches!(
            parse_section(&make_elf(3, 2), SectionKind::Text),
            Err(DumpError::UnsupportedFormat(_))
        ));
        assert!(matches!(
            parse_section(&make_elf(EM_X86_64, 1), SectionKind::Text),
            Err(DumpError::UnsupportedFormat(_))
        ));
        assert!(matches!(
            parse_section(&make_pe(0x14C, PE32_PLUS_MAGIC), SectionKind::Text),
            Err(DumpError::UnsupportedFormat(_))
        ));
        assert!(matches!(
            parse_section(&make_pe(IMAGE_FILE_MACHINE_AMD64, 0x10B), SectionKind::Text),
            Err(DumpError::UnsupportedFormat(_))
        ));
        assert!(matches!(
            parse_section(b"not a binary", SectionKind::Text),
            Err(DumpError::UnsupportedFormat(_))
        ));
    }

    #[test]
    fn truncated_file_does_not_panic() {
        let img = make_elf(EM_X86_64, 2);
        for n in 0..img.len() {
            let _ = parse_section(&img[..n], SectionKind::Text);
        }
        let img = make_pe(IMAGE_FILE_MACHINE_AMD64, PE32_PLUS_MAGIC);
        for n in 0..img.len() {
            let _ = parse_section(&img[..n], SectionKind::Data);
        }
    }

    #[test]
    fn section_arg_parsing() {
        assert_eq!(SectionKind::parse("text").unwrap(), SectionKind::Text);
        assert_eq!(SectionKind::parse(".DATA").unwrap(), SectionKind::Data);
        assert!(SectionKind::parse("bss").is_err());
    }
}