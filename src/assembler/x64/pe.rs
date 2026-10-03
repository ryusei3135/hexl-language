use super::emitter::{Emitter, FixupKind, Section};

// ============================================================
// PE (Portable Executable) format constants
// ============================================================
//
// Windows 用の最小構成 PE32+ (x64) 実行ファイルを組み立てるモジュール。
// elf.rs の write_executable と同じ考え方で、.text/.data を単一の
// RWX セクションにそのまま詰め込み、Emitter が計算したラベル/フィクス
// アップを PE のアドレス空間 (ImageBase + RVA) で解決する。
//
// インポートテーブルは持たない (kernel32.dll 等への動的リンクは行わない)。
// そのため `_start` からは `syscall` 命令による直接システムコール呼び出し
// などクローズドな処理のみが行える。外部 DLL 関数を呼びたい場合は、
// 別途インポートディレクトリの実装が必要になる。

const IMAGE_DOS_SIGNATURE: u16 = 0x5A4D; // "MZ"
const IMAGE_NT_SIGNATURE: u32 = 0x0000_4550; // "PE\0\0"

const IMAGE_FILE_MACHINE_AMD64: u16 = 0x8664;

const IMAGE_FILE_RELOCS_STRIPPED: u16 = 0x0001;
const IMAGE_FILE_EXECUTABLE_IMAGE: u16 = 0x0002;
const IMAGE_FILE_LARGE_ADDRESS_AWARE: u16 = 0x0020;

const IMAGE_OPTIONAL_HDR64_MAGIC: u16 = 0x20b;

const IMAGE_SUBSYSTEM_WINDOWS_CUI: u16 = 3;

const IMAGE_SCN_CNT_CODE: u32 = 0x0000_0020;
const IMAGE_SCN_CNT_INITIALIZED_DATA: u32 = 0x0000_0040;
const IMAGE_SCN_MEM_EXECUTE: u32 = 0x2000_0000;
const IMAGE_SCN_MEM_READ: u32 = 0x4000_0000;
const IMAGE_SCN_MEM_WRITE: u32 = 0x8000_0000;

const IMAGE_NUMBEROF_DIRECTORY_ENTRIES: u32 = 16;

/// x64 EXE のデフォルト ImageBase (MSVC リンカのデフォルトと同じ値)
const IMAGE_BASE: u64 = 0x1_4000_0000;

const SECTION_ALIGNMENT: u32 = 0x1000;
const FILE_ALIGNMENT: u32 = 0x200;

fn align_up(value: u32, align: u32) -> u32 {
    if align == 0 {
        return value;
    }

    let remainder = value % align;

    if remainder == 0 {
        value
    } else {
        value + (align - remainder)
    }
}

// ============================================================
// 実行ファイル (Windows .exe)
// ============================================================

/// Emitter の結果から、単一セクション (.text) に .text/.data を
/// そのまま詰め込んだ最小構成の PE32+ (x64) 実行ファイルを組み立てる。
///
/// `.section test` が使われた場合は、名前 `test` の独立したセクション
/// (とそのセクションヘッダ) を .text の後ろに追加する。
///
/// elf.rs の `write_executable` と同様、外部リンクは行わないため、
/// 未解決の外部シンボル (`.extern`) が残っている場合はエラーになる。
pub fn write_executable(emitter: &Emitter) -> Result<Vec<u8>, String> {
    const SECTION_RVA: u32 = SECTION_ALIGNMENT; // 先頭ページ (headers) の直後に配置

    let text_off = 0usize;
    let data_off = emitter.text.len();

    let mut text = emitter.text.clone();
    let mut data = emitter.data.clone();
    let mut test = emitter.test.clone();

    let uses_test = emitter.uses_test;

    // .text/.data をまとめたセクションの実サイズ
    let main_len = (text.len() + data.len()) as u32;

    // test セクションは .text セクションの次のページから始める。
    // 以降のアドレス計算は SECTION_RVA からの相対値 (test_off) で扱う。
    let test_rva = SECTION_RVA + align_up(main_len, SECTION_ALIGNMENT);
    let test_off = (test_rva - SECTION_RVA) as usize;

    let resolve = |symbol: &str| -> Option<u64> {
        if let Some(&off) = emitter.text_labels.get(symbol) {
            return Some(IMAGE_BASE + SECTION_RVA as u64 + text_off as u64 + off as u64);
        }

        if let Some(&off) = emitter.data_labels.get(symbol) {
            return Some(IMAGE_BASE + SECTION_RVA as u64 + data_off as u64 + off as u64);
        }

        if let Some(&off) = emitter.test_labels.get(symbol) {
            return Some(IMAGE_BASE + SECTION_RVA as u64 + test_off as u64 + off as u64);
        }

        None
    };

    for fixup in &emitter.fixups {
        let target = resolve(&fixup.symbol).ok_or_else(|| {
            format!(
                "実行ファイルを直接生成できません: シンボル `{}` が未定義です \
                 (外部シンボルには対応していません)",
                fixup.symbol
            )
        })?;

        let section_local_base = match fixup.section {
            Section::Text => text_off,
            Section::Data => data_off,
            Section::Test => test_off,
        };

        let fixup_addr =
            IMAGE_BASE + SECTION_RVA as u64 + section_local_base as u64 + fixup.offset as u64;

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

    let mut section_raw = Vec::with_capacity(text.len() + data.len());
    section_raw.extend_from_slice(&text);
    section_raw.extend_from_slice(&data);

    let entry_rva = match emitter.text_labels.get("_start") {
        Some(&off) => SECTION_RVA + off as u32,
        None => SECTION_RVA,
    };

    // --------------------------------------------------------
    // ヘッダサイズの計算
    // --------------------------------------------------------

    const DOS_HEADER_SIZE: u32 = 64;
    const PE_SIG_SIZE: u32 = 4;
    const COFF_HEADER_SIZE: u32 = 20;
    const OPT_HEADER_FIXED_SIZE: u32 = 112;
    const DATA_DIR_SIZE: u32 = IMAGE_NUMBEROF_DIRECTORY_ENTRIES * 8;
    const OPT_HEADER_SIZE: u32 = OPT_HEADER_FIXED_SIZE + DATA_DIR_SIZE;
    const SECTION_HEADER_SIZE: u32 = 40;
    let num_sections: u16 = if uses_test { 2 } else { 1 };

    let headers_size_raw = DOS_HEADER_SIZE
        + PE_SIG_SIZE
        + COFF_HEADER_SIZE
        + OPT_HEADER_SIZE
        + SECTION_HEADER_SIZE * (num_sections as u32);

    let size_of_headers = align_up(headers_size_raw, FILE_ALIGNMENT);

    let size_of_raw_data = align_up(section_raw.len() as u32, FILE_ALIGNMENT);
    let virtual_size = section_raw.len() as u32;

    // test セクション (使う場合) の大きさ
    let test_virtual_size = test.len() as u32;
    let test_raw_size = align_up(test_virtual_size, FILE_ALIGNMENT);

    let size_of_image = if uses_test {
        align_up(
            test_rva + align_up(test_virtual_size, SECTION_ALIGNMENT),
            SECTION_ALIGNMENT,
        )
    } else {
        align_up(
            SECTION_RVA + align_up(virtual_size, SECTION_ALIGNMENT),
            SECTION_ALIGNMENT,
        )
    };

    // --------------------------------------------------------
    // DOS header (MZ) — スタブは省略し、直後に PE ヘッダを置く
    // --------------------------------------------------------

    let mut file: Vec<u8> = Vec::with_capacity(size_of_headers as usize + size_of_raw_data as usize);

    file.extend_from_slice(&IMAGE_DOS_SIGNATURE.to_le_bytes()); // e_magic
    file.extend(std::iter::repeat(0u8).take(58)); // e_cblp 〜 e_res2 (未使用フィールドは0埋め)
    file.extend_from_slice(&DOS_HEADER_SIZE.to_le_bytes()); // e_lfanew: PEヘッダはDOSヘッダ直後
    debug_assert_eq!(file.len(), DOS_HEADER_SIZE as usize);

    // --------------------------------------------------------
    // PE signature
    // --------------------------------------------------------

    file.extend_from_slice(&IMAGE_NT_SIGNATURE.to_le_bytes());

    // --------------------------------------------------------
    // COFF file header
    // --------------------------------------------------------

    file.extend_from_slice(&IMAGE_FILE_MACHINE_AMD64.to_le_bytes());
    file.extend_from_slice(&num_sections.to_le_bytes());
    file.extend_from_slice(&0u32.to_le_bytes()); // TimeDateStamp
    file.extend_from_slice(&0u32.to_le_bytes()); // PointerToSymbolTable
    file.extend_from_slice(&0u32.to_le_bytes()); // NumberOfSymbols
    file.extend_from_slice(&(OPT_HEADER_SIZE as u16).to_le_bytes()); // SizeOfOptionalHeader

    let characteristics: u16 =
        IMAGE_FILE_RELOCS_STRIPPED | IMAGE_FILE_EXECUTABLE_IMAGE | IMAGE_FILE_LARGE_ADDRESS_AWARE;
    file.extend_from_slice(&characteristics.to_le_bytes());

    // --------------------------------------------------------
    // Optional header (PE32+)
    // --------------------------------------------------------

    file.extend_from_slice(&IMAGE_OPTIONAL_HDR64_MAGIC.to_le_bytes());
    file.push(0); // MajorLinkerVersion
    file.push(0); // MinorLinkerVersion
    file.extend_from_slice(&size_of_raw_data.to_le_bytes()); // SizeOfCode (概算値)
    let size_of_initialized_data = if uses_test { test_raw_size } else { 0 };
    file.extend_from_slice(&size_of_initialized_data.to_le_bytes()); // SizeOfInitializedData
    file.extend_from_slice(&0u32.to_le_bytes()); // SizeOfUninitializedData
    file.extend_from_slice(&entry_rva.to_le_bytes()); // AddressOfEntryPoint
    file.extend_from_slice(&SECTION_RVA.to_le_bytes()); // BaseOfCode
    file.extend_from_slice(&IMAGE_BASE.to_le_bytes()); // ImageBase
    file.extend_from_slice(&SECTION_ALIGNMENT.to_le_bytes());
    file.extend_from_slice(&FILE_ALIGNMENT.to_le_bytes());
    file.extend_from_slice(&6u16.to_le_bytes()); // MajorOperatingSystemVersion
    file.extend_from_slice(&0u16.to_le_bytes()); // MinorOperatingSystemVersion
    file.extend_from_slice(&0u16.to_le_bytes()); // MajorImageVersion
    file.extend_from_slice(&0u16.to_le_bytes()); // MinorImageVersion
    file.extend_from_slice(&6u16.to_le_bytes()); // MajorSubsystemVersion (Windows Vista以降)
    file.extend_from_slice(&0u16.to_le_bytes()); // MinorSubsystemVersion
    file.extend_from_slice(&0u32.to_le_bytes()); // Win32VersionValue
    file.extend_from_slice(&size_of_image.to_le_bytes()); // SizeOfImage
    file.extend_from_slice(&size_of_headers.to_le_bytes()); // SizeOfHeaders
    file.extend_from_slice(&0u32.to_le_bytes()); // CheckSum
    file.extend_from_slice(&IMAGE_SUBSYSTEM_WINDOWS_CUI.to_le_bytes()); // Subsystem (コンソール)
    file.extend_from_slice(&0u16.to_le_bytes()); // DllCharacteristics
    file.extend_from_slice(&0x0010_0000u64.to_le_bytes()); // SizeOfStackReserve (1MiB)
    file.extend_from_slice(&0x0000_1000u64.to_le_bytes()); // SizeOfStackCommit
    file.extend_from_slice(&0x0010_0000u64.to_le_bytes()); // SizeOfHeapReserve (1MiB)
    file.extend_from_slice(&0x0000_1000u64.to_le_bytes()); // SizeOfHeapCommit
    file.extend_from_slice(&0u32.to_le_bytes()); // LoaderFlags
    file.extend_from_slice(&IMAGE_NUMBEROF_DIRECTORY_ENTRIES.to_le_bytes()); // NumberOfRvaAndSizes

    // データディレクトリ (インポート/エクスポート等は未使用なのですべて0)
    for _ in 0..IMAGE_NUMBEROF_DIRECTORY_ENTRIES {
        file.extend_from_slice(&0u32.to_le_bytes()); // VirtualAddress
        file.extend_from_slice(&0u32.to_le_bytes()); // Size
    }

    // --------------------------------------------------------
    // Section header (.text — code と data をまとめて RWX で配置)
    // --------------------------------------------------------

    let mut name = [0u8; 8];
    name[..5].copy_from_slice(b".text");
    file.extend_from_slice(&name);

    file.extend_from_slice(&virtual_size.to_le_bytes()); // VirtualSize
    file.extend_from_slice(&SECTION_RVA.to_le_bytes()); // VirtualAddress
    file.extend_from_slice(&size_of_raw_data.to_le_bytes()); // SizeOfRawData
    file.extend_from_slice(&size_of_headers.to_le_bytes()); // PointerToRawData
    file.extend_from_slice(&0u32.to_le_bytes()); // PointerToRelocations
    file.extend_from_slice(&0u32.to_le_bytes()); // PointerToLinenumbers
    file.extend_from_slice(&0u16.to_le_bytes()); // NumberOfRelocations
    file.extend_from_slice(&0u16.to_le_bytes()); // NumberOfLinenumbers

    let section_characteristics =
        IMAGE_SCN_CNT_CODE | IMAGE_SCN_MEM_EXECUTE | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE;
    file.extend_from_slice(&section_characteristics.to_le_bytes());

    // --------------------------------------------------------
    // Section header (test — `.section test` を使ったときだけ)
    // --------------------------------------------------------

    if uses_test {
        let mut name = [0u8; 8];
        name[..4].copy_from_slice(b"test");
        file.extend_from_slice(&name);

        file.extend_from_slice(&test_virtual_size.to_le_bytes()); // VirtualSize
        file.extend_from_slice(&test_rva.to_le_bytes()); // VirtualAddress
        file.extend_from_slice(&test_raw_size.to_le_bytes()); // SizeOfRawData
        file.extend_from_slice(&(size_of_headers + size_of_raw_data).to_le_bytes()); // PointerToRawData
        file.extend_from_slice(&0u32.to_le_bytes()); // PointerToRelocations
        file.extend_from_slice(&0u32.to_le_bytes()); // PointerToLinenumbers
        file.extend_from_slice(&0u16.to_le_bytes()); // NumberOfRelocations
        file.extend_from_slice(&0u16.to_le_bytes()); // NumberOfLinenumbers

        let test_characteristics =
            IMAGE_SCN_CNT_INITIALIZED_DATA | IMAGE_SCN_MEM_READ | IMAGE_SCN_MEM_WRITE;
        file.extend_from_slice(&test_characteristics.to_le_bytes());
    }

    debug_assert_eq!(file.len(), headers_size_raw as usize);

    // ヘッダ全体を FileAlignment まで0埋め
    file.resize(size_of_headers as usize, 0);

    // --------------------------------------------------------
    // セクション本体 (.text + .data)
    // --------------------------------------------------------

    file.extend_from_slice(&section_raw);
    file.resize(size_of_headers as usize + size_of_raw_data as usize, 0);

    // --------------------------------------------------------
    // セクション本体 (test)
    // --------------------------------------------------------

    if uses_test {
        file.extend_from_slice(&test);
        file.resize(
            size_of_headers as usize + size_of_raw_data as usize + test_raw_size as usize,
            0,
        );
    }

    Ok(file)
}
