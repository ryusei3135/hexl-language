//! PE32+ (x86-64 Windows 実行ファイル) の書き出し部品。
//!
//! レイアウトの決定は `link.rs` が行い、ここはその結果をヘッダのバイト列にする部分と、
//! DLL からのインポート (`.idata`) を組み立てる部分だけを担当する。
//!
//! 出力する PE の方針:
//!
//! * 固定アドレスのイメージ (`RELOCS_STRIPPED`、ベースリロケーションなし)
//! * セクションは `.text` / `.rdata` / `.data` (GOT + データ + BSS) / `.idata` の最大 4 本
//! * SectionAlignment = 0x1000、FileAlignment = 0x200

pub(super) const SECTION_ALIGN: u64 = 0x1000;
pub(super) const FILE_ALIGN: u64 = 0x200;

const DOS_HEADER_SIZE: u64 = 0x40;
const PE_SIG_SIZE: u64 = 4;
const COFF_HEADER_SIZE: u64 = 20;
const OPT_HEADER_SIZE: u64 = 240; // PE32+ の固定部 112 + データディレクトリ 16 × 8
const SECTION_HEADER_SIZE: u64 = 40;

pub(super) const SCN_TEXT: u32 = 0x6000_0020; // CODE | MEM_EXECUTE | MEM_READ
pub(super) const SCN_RDATA: u32 = 0x4000_0040; // INITIALIZED_DATA | MEM_READ
pub(super) const SCN_DATA: u32 = 0xC000_0040; // INITIALIZED_DATA | MEM_READ | MEM_WRITE

/// ヘッダ全体 (DOS ヘッダ + PE ヘッダ + セクションヘッダ) の、アラインメント前のサイズ。
pub(super) fn headers_size(nsections: usize) -> u64 {
    DOS_HEADER_SIZE
        + PE_SIG_SIZE
        + COFF_HEADER_SIZE
        + OPT_HEADER_SIZE
        + SECTION_HEADER_SIZE * nsections as u64
}

/// Windows のサブシステム。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PeSubsystem {
    /// コンソールアプリケーション (`IMAGE_SUBSYSTEM_WINDOWS_CUI`)
    #[default]
    Console,
    /// GUI アプリケーション (`IMAGE_SUBSYSTEM_WINDOWS_GUI`)
    Gui,
}

impl PeSubsystem {
    fn code(self) -> u16 {
        match self {
            PeSubsystem::Console => 3,
            PeSubsystem::Gui => 2,
        }
    }
}

/// 出力する PE セクション 1 本分。
#[derive(Debug, Clone)]
pub(super) struct PeSection {
    pub name: &'static str,
    pub vsize: u32,
    pub rva: u32,
    pub rawsize: u32,
    /// `rawsize == 0` のときは 0
    pub rawptr: u32,
    pub characteristics: u32,
}

/// `write_headers` に渡す、レイアウト確定後の情報。
#[derive(Debug)]
pub(super) struct PeHeaderInfo<'a> {
    pub image_base: u64,
    pub entry_rva: u32,
    pub sections: &'a [PeSection],
    pub size_of_headers: u32,
    pub size_of_image: u32,
    pub size_of_code: u32,
    pub size_of_initialized_data: u32,
    pub size_of_uninitialized_data: u32,
    pub base_of_code: u32,
    pub subsystem: PeSubsystem,
    /// (RVA, サイズ)。無ければ (0, 0)
    pub import_dir: (u32, u32),
    pub iat: (u32, u32),
}

fn put16(h: &mut Vec<u8>, v: u16) {
    h.extend_from_slice(&v.to_le_bytes());
}
fn put32(h: &mut Vec<u8>, v: u32) {
    h.extend_from_slice(&v.to_le_bytes());
}
fn put64(h: &mut Vec<u8>, v: u64) {
    h.extend_from_slice(&v.to_le_bytes());
}

/// イメージ先頭にヘッダを書き込む。
pub(super) fn write_headers(image: &mut [u8], info: &PeHeaderInfo) {
    let mut h: Vec<u8> = Vec::new();

    // DOS ヘッダ (e_lfanew だけが意味を持つ)
    h.extend_from_slice(b"MZ");
    h.resize(0x3c, 0);
    put32(&mut h, DOS_HEADER_SIZE as u32); // e_lfanew
    debug_assert_eq!(h.len() as u64, DOS_HEADER_SIZE);

    // PE シグネチャ + COFF ファイルヘッダ
    h.extend_from_slice(b"PE\0\0");
    put16(&mut h, 0x8664); // Machine = AMD64
    put16(&mut h, info.sections.len() as u16);
    put32(&mut h, 0); // TimeDateStamp (再現可能ビルドのため 0)
    put32(&mut h, 0); // PointerToSymbolTable
    put32(&mut h, 0); // NumberOfSymbols
    put16(&mut h, OPT_HEADER_SIZE as u16);
    // EXECUTABLE_IMAGE | RELOCS_STRIPPED | LARGE_ADDRESS_AWARE
    put16(&mut h, 0x0002 | 0x0001 | 0x0020);

    // オプションヘッダ (PE32+)
    put16(&mut h, 0x20b);
    h.extend_from_slice(&[0, 0]); // Major/MinorLinkerVersion
    put32(&mut h, info.size_of_code);
    put32(&mut h, info.size_of_initialized_data);
    put32(&mut h, info.size_of_uninitialized_data);
    put32(&mut h, info.entry_rva);
    put32(&mut h, info.base_of_code);
    put64(&mut h, info.image_base);
    put32(&mut h, SECTION_ALIGN as u32);
    put32(&mut h, FILE_ALIGN as u32);
    put16(&mut h, 6); // MajorOperatingSystemVersion
    put16(&mut h, 0);
    put16(&mut h, 0); // Major/MinorImageVersion
    put16(&mut h, 0);
    put16(&mut h, 6); // MajorSubsystemVersion
    put16(&mut h, 0);
    put32(&mut h, 0); // Win32VersionValue
    put32(&mut h, info.size_of_image);
    put32(&mut h, info.size_of_headers);
    put32(&mut h, 0); // CheckSum
    put16(&mut h, info.subsystem.code());
    put16(&mut h, 0); // DllCharacteristics (固定アドレスなので DYNAMIC_BASE なし)
    put64(&mut h, 0x10_0000); // SizeOfStackReserve
    put64(&mut h, 0x1000); // SizeOfStackCommit
    put64(&mut h, 0x10_0000); // SizeOfHeapReserve
    put64(&mut h, 0x1000); // SizeOfHeapCommit
    put32(&mut h, 0); // LoaderFlags
    put32(&mut h, 16); // NumberOfRvaAndSizes

    // データディレクトリ (16 個): 1 = import, 12 = IAT
    for i in 0..16 {
        let (rva, size) = match i {
            1 => info.import_dir,
            12 => info.iat,
            _ => (0, 0),
        };
        put32(&mut h, rva);
        put32(&mut h, size);
    }

    // セクションヘッダ
    for s in info.sections {
        let mut name = [0u8; 8];
        name[..s.name.len()].copy_from_slice(s.name.as_bytes());
        h.extend_from_slice(&name);
        put32(&mut h, s.vsize);
        put32(&mut h, s.rva);
        put32(&mut h, s.rawsize);
        put32(&mut h, s.rawptr);
        put32(&mut h, 0); // PointerToRelocations
        put32(&mut h, 0); // PointerToLinenumbers
        put16(&mut h, 0);
        put16(&mut h, 0);
        put32(&mut h, s.characteristics);
    }

    image[..h.len()].copy_from_slice(&h);
}

// ---------------------------------------------------------------------------
// インポート
// ---------------------------------------------------------------------------

/// 1 つの DLL から取り込むシンボルの一覧。
///
/// リンク時に実際に参照されたシンボルだけが import テーブルに載る。
/// シンボル `foo` を取り込むと、リンカは次の 2 つを自動で定義する。
///
/// * `__imp_foo` — IAT スロット (ローダが関数アドレスを書き込む 8 バイト) のアドレス。
///   `call *__imp_foo(%rip)` のように間接呼び出しで使う。
/// * `foo` — `jmp *[IAT スロット]` だけのスタブ。`call foo` のように直接呼び出せる。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DllImport {
    /// DLL 名 (例: `"kernel32.dll"`)
    pub dll: String,
    /// 取り込む関数名 (例: `["ExitProcess", "GetStdHandle", "WriteFile"]`)
    pub symbols: Vec<String>,
}

impl DllImport {
    pub fn new<S: Into<String>>(
        dll: impl Into<String>,
        symbols: impl IntoIterator<Item = S>,
    ) -> Self {
        DllImport {
            dll: dll.into(),
            symbols: symbols.into_iter().map(Into::into).collect(),
        }
    }
}

/// 組み立て済みの `.idata`。
pub(super) struct BuiltImports {
    pub bytes: Vec<u8>,
    /// インポートディレクトリテーブルの (RVA, サイズ)
    pub dir: (u32, u32),
    /// IAT 全体の (RVA, サイズ)
    pub iat: (u32, u32),
    /// 入力の並び順 (DLL ごとに平坦化) に対応する、各シンボルの (名前, IAT スロットの RVA)
    pub slots: Vec<(String, u32)>,
}

fn align_even(n: usize) -> usize {
    n + (n & 1)
}

/// `.idata` を組み立てる。`rva` はこのセクションの先頭の RVA。
///
/// 内容の長さは `rva` に依存しないので、サイズだけ知りたいときは `rva = 0` で呼んでよい。
pub(super) fn build_imports(dlls: &[(String, Vec<String>)], rva: u32) -> BuiltImports {
    let ndll = dlls.len();
    let desc_size = 20 * (ndll + 1);
    // ILT / IAT: DLL ごとに (シンボル数 + 1) 個の 8 バイトエントリ (末尾は 0)
    let table_size: usize = dlls.iter().map(|(_, s)| 8 * (s.len() + 1)).sum();
    let ilt_off = desc_size;
    let iat_off = ilt_off + table_size;
    let names_off = iat_off + table_size;

    // 名前領域: シンボルごとに hint(2) + 名前 + NUL (偶数境界)、DLL 名 + NUL (偶数境界)
    let mut names: Vec<u8> = Vec::new();
    let mut hint_rvas: Vec<Vec<u32>> = Vec::new();
    let mut dll_name_rvas: Vec<u32> = Vec::new();
    for (dll, syms) in dlls {
        let mut v = Vec::new();
        for s in syms {
            v.push(rva + (names_off + names.len()) as u32);
            names.extend_from_slice(&[0, 0]);
            names.extend_from_slice(s.as_bytes());
            names.push(0);
            if names.len() % 2 != 0 {
                names.push(0);
            }
        }
        hint_rvas.push(v);
        dll_name_rvas.push(rva + (names_off + names.len()) as u32);
        names.extend_from_slice(dll.as_bytes());
        names.push(0);
        names.resize(align_even(names.len()), 0);
    }

    let total = names_off + names.len();
    let mut out = vec![0u8; total];
    let mut slots = Vec::new();

    let mut ilt_cur = ilt_off;
    let mut iat_cur = iat_off;
    for (di, (_, syms)) in dlls.iter().enumerate() {
        // ディレクトリエントリ: OriginalFirstThunk, TimeDateStamp, ForwarderChain, Name, FirstThunk
        let d = 20 * di;
        out[d..d + 4].copy_from_slice(&(rva + ilt_cur as u32).to_le_bytes());
        out[d + 12..d + 16].copy_from_slice(&dll_name_rvas[di].to_le_bytes());
        out[d + 16..d + 20].copy_from_slice(&(rva + iat_cur as u32).to_le_bytes());
        for (si, s) in syms.iter().enumerate() {
            let entry = (hint_rvas[di][si] as u64).to_le_bytes();
            out[ilt_cur..ilt_cur + 8].copy_from_slice(&entry);
            out[iat_cur..iat_cur + 8].copy_from_slice(&entry);
            slots.push((s.clone(), rva + iat_cur as u32));
            ilt_cur += 8;
            iat_cur += 8;
        }
        ilt_cur += 8; // 終端
        iat_cur += 8;
    }
    out[names_off..].copy_from_slice(&names);

    BuiltImports {
        bytes: out,
        dir: (rva, desc_size as u32),
        iat: (rva + iat_off as u32, table_size as u32),
        slots,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn import_table_has_expected_shape() {
        let dlls = vec![(
            "kernel32.dll".to_string(),
            vec!["ExitProcess".to_string(), "WriteFile".to_string()],
        )];
        let a = build_imports(&dlls, 0x3000);
        let b = build_imports(&dlls, 0);
        assert_eq!(a.bytes.len(), b.bytes.len());
        assert_eq!(a.slots.len(), 2);
        // IAT は 2 シンボル + 終端 = 24 バイト
        assert_eq!(a.iat.1, 24);
        assert_eq!(a.slots[1].1, a.slots[0].1 + 8);
        // ディレクトリは DLL 1 + 終端 = 40 バイト
        assert_eq!(a.dir, (0x3000, 40));
    }

    #[test]
    fn header_size_is_consistent() {
        assert_eq!(headers_size(0), 0x40 + 4 + 20 + 240);
        assert_eq!(headers_size(3), 0x40 + 4 + 20 + 240 + 120);
    }
}
