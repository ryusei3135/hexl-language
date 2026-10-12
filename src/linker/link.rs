//! リンカ本体。
//!
//! 処理の流れ:
//!
//! 1. セクション選択 (`SHF_ALLOC` のみ採用、COMDAT の重複を除去)
//! 2. グローバルシンボルの解決 (強/弱/COMMON のルールに従う)
//! 3. レイアウト決定 (`.text` / `.rodata` / `.got` + `.data` / `.bss` の 3 セグメント)
//! 4. リロケーション適用 (オーバーフロー検査付き)
//! 5. 実行ファイルの書き出し (ELF の ET_EXEC、または PE32+)
//!
//! 入力は ELF64 / COFF (x86-64) のオブジェクトを混在させて渡せる (COFF は [`super::coff`] が
//! 内部表現へ変換する)。出力形式は [`LinkOptions::format`] で選ぶ。

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::coff;
use super::elf::*;
use super::error::{LinkError, Result, UndefinedRef};
use super::pe::{self, DllImport, PeHeaderInfo, PeSection, PeSubsystem};

/// 出力の既定ロードアドレス
pub const DEFAULT_BASE_ADDRESS: u64 = 0x40_0000;

const PAGE_SIZE: u64 = 0x1000;
const EHDR_SIZE: u64 = 64;
const PHDR_SIZE: u64 = 56;
const PT_LOAD: u32 = 1;
const PF_X: u32 = 1;
const PF_W: u32 = 2;
const PF_R: u32 = 4;

const TEXT: usize = 0;
const RODATA: usize = 1;
const DATA: usize = 2;
const BSS: usize = 3;

/// 出力する実行ファイルの形式。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum OutputFormat {
    /// ELF64 実行ファイル (ET_EXEC)
    #[default]
    Elf,
    /// PE32+ 実行ファイル (Windows x64)
    Pe,
}

/// リンクの設定。
#[derive(Debug, Clone)]
pub struct LinkOptions {
    /// エントリポイントのシンボル名 (既定: `_start`)
    pub entry: String,
    /// ロードアドレス。ELF では 4096 の倍数、PE では 0x10000 の倍数であること
    pub base_address: u64,
    /// 出力形式 (既定: ELF)
    pub format: OutputFormat,
    /// PE のサブシステム (PE 出力のときだけ使う。既定: コンソール)
    pub subsystem: PeSubsystem,
    /// PE が DLL から取り込む関数 (PE 出力のときだけ使える)。
    /// 実際に参照されたものだけが import テーブルに載る。詳しくは [`DllImport`]。
    pub imports: Vec<DllImport>,
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            entry: "_start".to_string(),
            base_address: DEFAULT_BASE_ADDRESS,
            format: OutputFormat::Elf,
            subsystem: PeSubsystem::Console,
            imports: Vec::new(),
        }
    }
}

impl LinkOptions {
    /// PE32+ (Windows x64) を出力する設定。ほかは既定値。
    pub fn pe() -> Self {
        LinkOptions {
            format: OutputFormat::Pe,
            ..Default::default()
        }
    }
}

/// 出力に含まれるセクション (マップ表示用)。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSection {
    pub name: &'static str,
    pub addr: u64,
    /// ファイル内オフセット (`.bss` はファイルに実体が無いので参考値)
    pub file_offset: u64,
    pub size: u64,
}

/// 最終アドレスが確定したグローバルシンボル。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutputSymbol {
    pub name: String,
    pub addr: u64,
}

/// リンク結果。
#[derive(Debug)]
pub struct LinkOutput {
    /// 実行ファイル (ELF または PE) のバイト列
    pub image: Vec<u8>,
    /// `image` の形式
    pub format: OutputFormat,
    /// エントリポイントの仮想アドレス
    pub entry: u64,
    pub sections: Vec<OutputSection>,
    /// アドレス順 (同一アドレスは名前順) に整列したシンボル
    pub symbols: Vec<OutputSymbol>,
}

impl LinkOutput {
    /// ファイルに書き出す (ELF を Unix で書くときは実行権限 0755 を付ける)。
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let io_err = |source| LinkError::Io {
            path: path.display().to_string(),
            source,
        };
        std::fs::write(path, &self.image).map_err(io_err)?;
        #[cfg(unix)]
        if self.format == OutputFormat::Elf {
            use std::os::unix::fs::PermissionsExt;
            std::fs::set_permissions(path, std::fs::Permissions::from_mode(0o755))
                .map_err(io_err)?;
        }
        Ok(())
    }
}

// ---------------------------------------------------------------------------
// セクション選択
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Text,
    Rodata,
    Data,
    Bss,
}

const KINDS: [Kind; 4] = [Kind::Text, Kind::Rodata, Kind::Data, Kind::Bss];

impl Kind {
    fn idx(self) -> usize {
        self as usize
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Fate {
    /// 出力に載せない (デバッグ情報・シンボルテーブル・リロケーション等)
    Drop,
    /// 重複した COMDAT グループのため破棄
    Discarded,
    Keep(Kind),
}

fn classify(obj: &ElfObject, sec: &SectionHeader) -> Result<Fate> {
    if sec.flags & SHF_ALLOC == 0 {
        return Ok(Fate::Drop);
    }
    match sec.sh_type {
        SHT_NULL | SHT_SYMTAB | SHT_STRTAB | SHT_RELA | SHT_REL | SHT_GROUP
        | SHT_SYMTAB_SHNDX => return Ok(Fate::Drop),
        _ => {}
    }
    if sec.flags & SHF_TLS != 0 {
        return Err(LinkError::Unsupported {
            object: obj.name.clone(),
            message: format!("thread-local section `{}'", sec.name),
        });
    }
    let kind = if sec.sh_type == SHT_NOBITS {
        Kind::Bss
    } else if sec.flags & SHF_EXECINSTR != 0 {
        Kind::Text
    } else if sec.flags & SHF_WRITE != 0 {
        Kind::Data
    } else {
        Kind::Rodata
    };
    Ok(Fate::Keep(kind))
}

fn plan_sections(objects: &[ElfObject]) -> Result<Vec<Vec<Fate>>> {
    let mut seen_groups: HashSet<&str> = HashSet::new();
    let mut plan = Vec::with_capacity(objects.len());
    for obj in objects {
        let mut fates = Vec::with_capacity(obj.sections.len());
        for sec in &obj.sections {
            fates.push(classify(obj, sec)?);
        }
        // 先に現れた COMDAT グループだけを残す
        for g in &obj.groups {
            if g.comdat && !g.signature.is_empty() && !seen_groups.insert(g.signature.as_str()) {
                for &m in &g.members {
                    fates[m as usize] = Fate::Discarded;
                }
            }
        }
        plan.push(fates);
    }
    Ok(plan)
}

// ---------------------------------------------------------------------------
// シンボル解決
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, Copy)]
enum Def {
    Undefined,
    Defined {
        obj: usize,
        shndx: u16,
        value: u64,
        weak: bool,
    },
    /// リンカが合成したシンボル (`_end` など)
    Absolute(u64),
    Common {
        size: u64,
        align: u64,
    },
}

#[derive(Debug)]
struct GlobalSym {
    name: String,
    def: Def,
}

#[derive(Debug, Default)]
struct SymTable {
    index: HashMap<String, usize>,
    syms: Vec<GlobalSym>,
}

impl SymTable {
    fn slot(&mut self, name: &str) -> usize {
        if let Some(&i) = self.index.get(name) {
            return i;
        }
        let i = self.syms.len();
        self.syms.push(GlobalSym {
            name: name.to_string(),
            def: Def::Undefined,
        });
        self.index.insert(name.to_string(), i);
        i
    }

    fn merge(&mut self, gi: usize, new: Def, objects: &[ElfObject]) -> Result<()> {
        let slot = &mut self.syms[gi];
        slot.def = match (slot.def, new) {
            (Def::Undefined, n) => n,
            (Def::Common { size: s1, align: a1 }, Def::Common { size: s2, align: a2 }) => {
                Def::Common {
                    size: s1.max(s2),
                    align: a1.max(a2),
                }
            }
            // 定義は COMMON に勝つ
            (Def::Common { .. }, n @ Def::Defined { .. }) => n,
            (old @ Def::Defined { .. }, Def::Common { .. }) => old,
            // 強い定義は弱い定義に勝つ。弱同士は先着。
            (Def::Defined { weak: true, .. }, n @ Def::Defined { weak: false, .. }) => n,
            (old @ Def::Defined { weak: true, .. }, Def::Defined { weak: true, .. }) => old,
            (old @ Def::Defined { weak: false, .. }, Def::Defined { weak: true, .. }) => old,
            (
                Def::Defined {
                    obj: o1,
                    weak: false,
                    ..
                },
                Def::Defined {
                    obj: o2,
                    weak: false,
                    ..
                },
            ) => {
                return Err(LinkError::DuplicateSymbol {
                    name: slot.name.clone(),
                    first: objects[o1].name.clone(),
                    second: objects[o2].name.clone(),
                })
            }
            (old, _) => old,
        };
        Ok(())
    }

    /// 参照されていて未定義のときだけ、リンカ合成シンボルとして定義する。
    fn define_synthetic(&mut self, name: &str, addr: u64) {
        if let Some(&i) = self.index.get(name) {
            if matches!(self.syms[i].def, Def::Undefined) {
                self.syms[i].def = Def::Absolute(addr);
            }
        }
    }
}

fn build_symbol_table(objects: &[ElfObject], plan: &[Vec<Fate>]) -> Result<SymTable> {
    let mut table = SymTable::default();
    for (oi, obj) in objects.iter().enumerate() {
        for sym in obj.symbols.iter().skip(1) {
            if sym.bind == STB_LOCAL
                || sym.name.is_empty()
                || sym.typ == STT_FILE
                || sym.typ == STT_SECTION
            {
                continue;
            }
            let weak = sym.bind == STB_WEAK;
            let gi = table.slot(&sym.name);
            let new = match sym.shndx {
                SHN_UNDEF => continue,
                SHN_COMMON => Def::Common {
                    size: sym.size,
                    align: sym.value.max(1),
                },
                SHN_ABS => Def::Defined {
                    obj: oi,
                    shndx: SHN_ABS,
                    value: sym.value,
                    weak,
                },
                idx => {
                    // 採用されなかったセクション (破棄された COMDAT 等) の定義は無視
                    if !matches!(plan[oi][idx as usize], Fate::Keep(_)) {
                        continue;
                    }
                    Def::Defined {
                        obj: oi,
                        shndx: idx,
                        value: sym.value,
                        weak,
                    }
                }
            };
            table.merge(gi, new, objects)?;
        }
    }
    Ok(table)
}

// ---------------------------------------------------------------------------
// レイアウト
// ---------------------------------------------------------------------------

fn align_up(v: u64, a: u64) -> u64 {
    let a = a.max(1);
    v.div_ceil(a) * a
}

#[derive(Debug)]
struct Segment {
    flags: u32,
    offset: u64,
    vaddr: u64,
    filesz: u64,
    memsz: u64,
}

#[derive(Debug)]
struct Layout {
    /// 入力セクションごとの仮想アドレス (採用されなかったものは 0)
    sec_addr: Vec<Vec<u64>>,
    /// 入力セクションごとのファイルオフセット
    sec_foff: Vec<Vec<u64>>,
    /// COMMON シンボル (グローバル番号 → アドレス)
    common_addr: HashMap<usize, u64>,
    got_addr: u64,
    got_foff: u64,
    segments: Vec<Segment>,
    file_size: u64,
    etext: u64,
    edata: u64,
    bss_start: u64,
    end: u64,
    out_sections: Vec<OutputSection>,
    /// PE 出力のときだけ Some
    pe: Option<PeLayout>,
}

/// PE 固有のレイアウト結果。
#[derive(Debug)]
struct PeLayout {
    sections: Vec<PeSection>,
    size_of_headers: u32,
    size_of_image: u32,
    size_of_code: u32,
    size_of_init: u32,
    size_of_uninit: u32,
    base_of_code: u32,
    /// `.idata` の RVA とファイルオフセット (無ければ 0)
    idata_rva: u32,
    idata_foff: u64,
    /// import スタブ領域の仮想アドレスとファイルオフセット (`.text` の末尾)
    thunk_addr: u64,
    thunk_foff: u64,
}

/// 種別ごとのセクション連結結果 (ELF / PE 共通)。
struct Merged {
    size: [u64; 4],
    align: [u64; 4],
    /// 入力セクションごとの、種別内でのオフセット
    sec_off: Vec<Vec<u64>>,
    /// COMMON シンボル (グローバル番号 → .bss 内オフセット)
    common_off: HashMap<usize, u64>,
}

fn merge_kinds(objects: &[ElfObject], plan: &[Vec<Fate>], table: &SymTable) -> Merged {
    let mut size = [0u64; 4];
    let mut align = [1u64; 4];
    let mut sec_off: Vec<Vec<u64>> = objects
        .iter()
        .map(|o| vec![0u64; o.sections.len()])
        .collect();

    // 種別ごとに入力順で連結する
    for kind in KINDS {
        let k = kind.idx();
        for (oi, obj) in objects.iter().enumerate() {
            for (si, sec) in obj.sections.iter().enumerate() {
                if plan[oi][si] != Fate::Keep(kind) {
                    continue;
                }
                let a = sec.align.max(1);
                size[k] = align_up(size[k], a);
                sec_off[oi][si] = size[k];
                size[k] += sec.size;
                align[k] = align[k].max(a);
            }
        }
    }

    // COMMON シンボルは .bss の末尾に割り当てる
    let mut common_off: HashMap<usize, u64> = HashMap::new();
    for (gi, g) in table.syms.iter().enumerate() {
        if let Def::Common { size: csz, align: cal } = g.def {
            size[BSS] = align_up(size[BSS], cal);
            common_off.insert(gi, size[BSS]);
            size[BSS] += csz;
            align[BSS] = align[BSS].max(cal);
        }
    }

    Merged {
        size,
        align,
        sec_off,
        common_off,
    }
}

/// PE セクションを順に並べていくための作業用構造体。
struct PeBuilder {
    sections: Vec<PeSection>,
    rva: u64,
    foff: u64,
}

impl PeBuilder {
    /// 末尾にセクションを追加し、(RVA, ファイルオフセット) を返す。
    fn place(&mut self, name: &'static str, vsize: u64, rawsize: u64, characteristics: u32) -> (u64, u64) {
        let rva = self.rva;
        let foff = self.foff;
        let raw = align_up(rawsize, pe::FILE_ALIGN);
        self.sections.push(PeSection {
            name,
            vsize: vsize as u32,
            rva: rva as u32,
            rawsize: raw as u32,
            rawptr: if raw == 0 { 0 } else { foff as u32 },
            characteristics,
        });
        self.rva = align_up(rva + vsize.max(1), pe::SECTION_ALIGN);
        self.foff = foff + raw;
        (rva, foff)
    }
}

impl Layout {
    fn compute(
        objects: &[ElfObject],
        plan: &[Vec<Fate>],
        table: &SymTable,
        got_count: usize,
        base: u64,
    ) -> Layout {
        let Merged {
            size,
            align,
            sec_off,
            common_off,
        } = merge_kinds(objects, plan, table);

        let got_size = 8 * got_count as u64;
        let has_ro = size[RODATA] > 0;
        let has_rw = size[DATA] > 0 || size[BSS] > 0 || got_size > 0;
        let nph = 1 + has_ro as u64 + has_rw as u64;
        let hdr_end = EHDR_SIZE + PHDR_SIZE * nph;

        let mut kind_foff = [0u64; 4];
        let mut segments = Vec::new();

        // セグメント 0: ELF ヘッダ + プログラムヘッダ + .text (R+X)
        kind_foff[TEXT] = align_up(hdr_end, align[TEXT]);
        let text_end = kind_foff[TEXT] + size[TEXT];
        segments.push(Segment {
            flags: PF_R | PF_X,
            offset: 0,
            vaddr: base,
            filesz: text_end,
            memsz: text_end,
        });
        let mut file_size = text_end;

        // セグメント 1: .rodata (R)
        if has_ro {
            let off = align_up(file_size, PAGE_SIZE.max(align[RODATA]));
            kind_foff[RODATA] = off;
            segments.push(Segment {
                flags: PF_R,
                offset: off,
                vaddr: base + off,
                filesz: size[RODATA],
                memsz: size[RODATA],
            });
            file_size = off + size[RODATA];
        } else {
            kind_foff[RODATA] = file_size;
        }

        // セグメント 2: .got + .data + .bss (R+W)
        let (mut got_foff, mut bss_start, mut end) = (file_size, file_size, file_size);
        if has_rw {
            let start = align_up(file_size, PAGE_SIZE);
            got_foff = start;
            let data_off = align_up(start + got_size, align[DATA]);
            let data_end = data_off + size[DATA];
            let bss_off = align_up(
                if size[DATA] > 0 {
                    data_end
                } else {
                    start + got_size
                },
                align[BSS],
            );
            kind_foff[DATA] = data_off;
            kind_foff[BSS] = bss_off;
            let filesz = if size[DATA] > 0 {
                data_end - start
            } else {
                got_size
            };
            let memsz = if size[BSS] > 0 {
                bss_off + size[BSS] - start
            } else {
                filesz
            };
            segments.push(Segment {
                flags: PF_R | PF_W,
                offset: start,
                vaddr: base + start,
                filesz,
                memsz,
            });
            file_size = start + filesz;
            bss_start = bss_off;
            end = start + memsz;
        } else {
            kind_foff[DATA] = file_size;
            kind_foff[BSS] = file_size;
        }

        let mut sec_addr = Vec::with_capacity(objects.len());
        let mut sec_foff = Vec::with_capacity(objects.len());
        for (oi, obj) in objects.iter().enumerate() {
            let mut addrs = vec![0u64; obj.sections.len()];
            let mut offs = vec![0u64; obj.sections.len()];
            for si in 0..obj.sections.len() {
                if let Fate::Keep(kind) = plan[oi][si] {
                    let foff = kind_foff[kind.idx()] + sec_off[oi][si];
                    offs[si] = foff;
                    addrs[si] = base + foff;
                }
            }
            sec_addr.push(addrs);
            sec_foff.push(offs);
        }

        let common_addr = common_off
            .into_iter()
            .map(|(gi, off)| (gi, base + kind_foff[BSS] + off))
            .collect();

        let mut out_sections = Vec::new();
        let mut push = |name: &'static str, foff: u64, sz: u64| {
            if sz > 0 {
                out_sections.push(OutputSection {
                    name,
                    addr: base + foff,
                    file_offset: foff,
                    size: sz,
                });
            }
        };
        push(".text", kind_foff[TEXT], size[TEXT]);
        push(".rodata", kind_foff[RODATA], size[RODATA]);
        push(".got", got_foff, got_size);
        push(".data", kind_foff[DATA], size[DATA]);
        push(".bss", kind_foff[BSS], size[BSS]);

        Layout {
            sec_addr,
            sec_foff,
            common_addr,
            got_addr: base + got_foff,
            got_foff,
            segments,
            file_size,
            etext: base + text_end,
            edata: base + file_size,
            bss_start: base + bss_start,
            end: base + end,
            out_sections,
            pe: None,
        }
    }

    /// PE32+ 用のレイアウト。
    ///
    /// `.text` (+ import スタブ) / `.rdata` / `.data` (GOT + データ + BSS) / `.idata` を
    /// 0x1000 境界の RVA に並べる。ファイル上は 0x200 境界。
    fn compute_pe(
        objects: &[ElfObject],
        plan: &[Vec<Fate>],
        table: &SymTable,
        got_count: usize,
        base: u64,
        n_thunks: usize,
        idata_size: u64,
    ) -> Result<Layout> {
        let Merged {
            size,
            align,
            sec_off,
            common_off,
        } = merge_kinds(objects, plan, table);
        let too_big = |what: &str| LinkError::Unsupported {
            object: "(output)".to_string(),
            message: what.to_string(),
        };
        for k in KINDS {
            if align[k.idx()] > pe::SECTION_ALIGN {
                return Err(too_big(&format!(
                    "section alignment {:#x} exceeds the PE section alignment ({:#x})",
                    align[k.idx()],
                    pe::SECTION_ALIGN
                )));
            }
        }

        // import スタブ (jmp *[rip+disp32], 6 バイト) は .text の末尾に置く
        let thunk_off = align_up(size[TEXT], 16);
        let text_total = if n_thunks > 0 {
            thunk_off + 6 * n_thunks as u64
        } else {
            size[TEXT]
        };
        let got_size = 8 * got_count as u64;
        let has_text = text_total > 0;
        let has_ro = size[RODATA] > 0;
        let has_rw = size[DATA] > 0 || size[BSS] > 0 || got_size > 0;
        let has_idata = idata_size > 0;

        let nsec = has_text as usize + has_ro as usize + has_rw as usize + has_idata as usize;
        let size_of_headers = align_up(pe::headers_size(nsec), pe::FILE_ALIGN);
        let mut b = PeBuilder {
            sections: Vec::new(),
            rva: align_up(size_of_headers, pe::SECTION_ALIGN),
            foff: size_of_headers,
        };

        // .data の内部配置: GOT → .data → .bss (BSS はファイル上の実体なし)
        let data_rel = align_up(got_size, align[DATA]);
        let data_end = data_rel + size[DATA];
        let (rw_filesz, bss_rel) = if size[DATA] > 0 {
            (data_end, align_up(data_end, align[BSS]))
        } else {
            (got_size, align_up(got_size, align[BSS]))
        };
        let rw_vsize = if size[BSS] > 0 {
            bss_rel + size[BSS]
        } else {
            rw_filesz
        };

        let (mut text, mut rodata, mut rw, mut idata) = ((0, 0), (0, 0), (0, 0), (0, 0));
        if has_text {
            text = b.place(".text", text_total, text_total, pe::SCN_TEXT);
        }
        if has_ro {
            rodata = b.place(".rdata", size[RODATA], size[RODATA], pe::SCN_RDATA);
        }
        if has_rw {
            rw = b.place(".data", rw_vsize, rw_filesz, pe::SCN_DATA);
        }
        if has_idata {
            idata = b.place(".idata", idata_size, idata_size, pe::SCN_DATA);
        }
        if b.rva > u32::MAX as u64 || b.foff > u32::MAX as u64 || base.checked_add(b.rva).is_none() {
            return Err(too_big("output image is too large for PE"));
        }

        // 出力に載らなかった種別 (サイズ 0 のセクションだけがある場合) はイメージ末尾を指す
        let mut kind_rva = [b.rva; 4];
        let mut kind_foff = [b.foff; 4];
        if has_text {
            kind_rva[TEXT] = text.0;
            kind_foff[TEXT] = text.1;
        }
        if has_ro {
            kind_rva[RODATA] = rodata.0;
            kind_foff[RODATA] = rodata.1;
        }
        if has_rw {
            kind_rva[DATA] = rw.0 + data_rel;
            kind_foff[DATA] = rw.1 + data_rel;
            kind_rva[BSS] = rw.0 + bss_rel;
            kind_foff[BSS] = rw.1 + bss_rel;
        }

        let mut sec_addr = Vec::with_capacity(objects.len());
        let mut sec_foff = Vec::with_capacity(objects.len());
        for (oi, obj) in objects.iter().enumerate() {
            let mut addrs = vec![0u64; obj.sections.len()];
            let mut offs = vec![0u64; obj.sections.len()];
            for si in 0..obj.sections.len() {
                if let Fate::Keep(kind) = plan[oi][si] {
                    addrs[si] = base + kind_rva[kind.idx()] + sec_off[oi][si];
                    offs[si] = kind_foff[kind.idx()] + sec_off[oi][si];
                }
            }
            sec_addr.push(addrs);
            sec_foff.push(offs);
        }
        let common_addr = common_off
            .into_iter()
            .map(|(gi, off)| (gi, base + kind_rva[BSS] + off))
            .collect();

        let mut out_sections = Vec::new();
        let mut push = |name: &'static str, rva: u64, foff: u64, sz: u64| {
            if sz > 0 {
                out_sections.push(OutputSection {
                    name,
                    addr: base + rva,
                    file_offset: foff,
                    size: sz,
                });
            }
        };
        push(".text", text.0, text.1, size[TEXT]);
        push(".rdata", rodata.0, rodata.1, size[RODATA]);
        push(".got", rw.0, rw.1, got_size);
        push(".data", kind_rva[DATA], kind_foff[DATA], size[DATA]);
        push(".bss", kind_rva[BSS], kind_foff[BSS], size[BSS]);
        push(".idata", idata.0, idata.1, idata_size);

        let etext = base + text.0 + size[TEXT];
        let (edata, bss_start, end) = if has_rw {
            (
                base + rw.0 + rw_filesz,
                base + rw.0 + bss_rel,
                base + rw.0 + rw_vsize,
            )
        } else {
            (etext, etext, etext)
        };
        let size_of_init: u64 = b
            .sections
            .iter()
            .filter(|s| s.name != ".text")
            .map(|s| s.rawsize as u64)
            .sum();

        let pe = PeLayout {
            size_of_headers: size_of_headers as u32,
            size_of_image: b.rva as u32,
            size_of_code: align_up(text_total, pe::FILE_ALIGN) as u32,
            size_of_init: size_of_init as u32,
            size_of_uninit: size[BSS] as u32,
            base_of_code: if has_text { text.0 as u32 } else { 0 },
            idata_rva: idata.0 as u32,
            idata_foff: idata.1,
            thunk_addr: base + text.0 + thunk_off,
            thunk_foff: text.1 + thunk_off,
            sections: b.sections,
        };
        Ok(Layout {
            sec_addr,
            sec_foff,
            common_addr,
            got_addr: base + rw.0,
            got_foff: rw.1,
            segments: Vec::new(),
            file_size: b.foff,
            etext,
            edata,
            bss_start,
            end,
            out_sections,
            pe: Some(pe),
        })
    }
}

// ---------------------------------------------------------------------------
// アドレス解決 / リロケーション
// ---------------------------------------------------------------------------

enum SymAddr {
    Addr(u64),
    Undefined,
}

struct Resolver<'a> {
    table: &'a SymTable,
    layout: &'a Layout,
}

impl Resolver<'_> {
    fn def_addr(&self, gi: usize) -> Option<u64> {
        match self.table.syms[gi].def {
            Def::Undefined => None,
            Def::Absolute(a) => Some(a),
            Def::Defined {
                obj, shndx, value, ..
            } => Some(if shndx == SHN_ABS {
                value
            } else {
                self.layout.sec_addr[obj][shndx as usize] + value
            }),
            Def::Common { .. } => self.layout.common_addr.get(&gi).copied(),
        }
    }

    fn local_addr(&self, oi: usize, sym: &Symbol) -> u64 {
        match sym.shndx {
            SHN_UNDEF | SHN_COMMON => 0,
            SHN_ABS => sym.value,
            idx => self.layout.sec_addr[oi][idx as usize] + sym.value,
        }
    }

    fn sym_addr(&self, oi: usize, sym: &Symbol) -> SymAddr {
        if sym.bind == STB_LOCAL || sym.name.is_empty() {
            return SymAddr::Addr(self.local_addr(oi, sym));
        }
        match self.table.index.get(&sym.name) {
            Some(&gi) => match self.def_addr(gi) {
                Some(a) => SymAddr::Addr(a),
                // 弱い未定義参照は 0 に解決される
                None if sym.bind == STB_WEAK => SymAddr::Addr(0),
                None => SymAddr::Undefined,
            },
            None => SymAddr::Addr(0),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum GotKey {
    Local(usize, u32),
    Global(usize),
}

fn got_key(table: &SymTable, oi: usize, sym_idx: u32, sym: &Symbol) -> GotKey {
    if sym.bind != STB_LOCAL {
        if let Some(&gi) = table.index.get(&sym.name) {
            return GotKey::Global(gi);
        }
    }
    GotKey::Local(oi, sym_idx)
}

fn is_got_reloc(t: u32) -> bool {
    matches!(
        t,
        R_X86_64_GOTPCREL | R_X86_64_GOTPCRELX | R_X86_64_REX_GOTPCRELX
    )
}

fn reloc_width(t: u32) -> Option<u64> {
    match t {
        R_X86_64_NONE => Some(0),
        R_X86_64_64 | R_X86_64_PC64 => Some(8),
        R_PE_ADDR32NB
        | R_X86_64_PC32
        | R_X86_64_PLT32
        | R_X86_64_32
        | R_X86_64_32S
        | R_X86_64_GOTPCREL
        | R_X86_64_GOTPCRELX
        | R_X86_64_REX_GOTPCRELX => Some(4),
        _ => None,
    }
}

fn reloc_name(t: u32) -> &'static str {
    match t {
        R_X86_64_NONE => "R_X86_64_NONE",
        R_X86_64_64 => "R_X86_64_64",
        R_X86_64_PC32 => "R_X86_64_PC32",
        R_X86_64_PLT32 => "R_X86_64_PLT32",
        R_X86_64_GOTPCREL => "R_X86_64_GOTPCREL",
        R_X86_64_32 => "R_X86_64_32",
        R_X86_64_32S => "R_X86_64_32S",
        R_X86_64_PC64 => "R_X86_64_PC64",
        R_X86_64_GOTPCRELX => "R_X86_64_GOTPCRELX",
        R_X86_64_REX_GOTPCRELX => "R_X86_64_REX_GOTPCRELX",
        R_PE_ADDR32NB => "IMAGE_REL_AMD64_ADDR32NB",
        _ => "R_X86_64_?",
    }
}

fn display_sym(obj: &ElfObject, sym: &Symbol) -> String {
    if !sym.name.is_empty() {
        sym.name.clone()
    } else if sym.typ == STT_SECTION && (sym.shndx as usize) < obj.sections.len() {
        obj.sections[sym.shndx as usize].name.clone()
    } else {
        "<anonymous>".to_string()
    }
}

fn fits_i32(v: i128) -> bool {
    v >= i32::MIN as i128 && v <= i32::MAX as i128
}

// ---------------------------------------------------------------------------
// 公開 API
// ---------------------------------------------------------------------------

/// オブジェクトファイルを集めて 1 つの実行ファイルにリンクする。
#[derive(Debug)]
pub struct Linker {
    options: LinkOptions,
    objects: Vec<ElfObject>,
}

impl Default for Linker {
    fn default() -> Self {
        Self::new()
    }
}

impl Linker {
    pub fn new() -> Self {
        Self::with_options(LinkOptions::default())
    }

    pub fn with_options(options: LinkOptions) -> Self {
        Linker {
            options,
            objects: Vec::new(),
        }
    }

    /// メモリ上のオブジェクトファイルを追加する。
    ///
    /// ELF64 か COFF (x86-64) かは内容から自動で判定する。出力形式とは独立なので、
    /// たとえば COFF の入力から ELF を出力することもできる。
    pub fn add_object(&mut self, name: impl Into<String>, data: Vec<u8>) -> Result<()> {
        let obj = if !data.starts_with(b"\x7fELF") && coff::looks_like_coff(&data) {
            coff::parse(name, data)?
        } else {
            // ELF でも COFF でもないものは、ELF パーサのエラーとして報告する
            ElfObject::parse(name, data)?
        };
        self.objects.push(obj);
        Ok(())
    }

    /// ファイルからオブジェクトファイルを読み込んで追加する。
    pub fn add_object_file(&mut self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let data = std::fs::read(path).map_err(|source| LinkError::Io {
            path: path.display().to_string(),
            source,
        })?;
        self.add_object(path.display().to_string(), data)
    }

    /// リンクを実行する。
    pub fn link(self) -> Result<LinkOutput> {
        let Linker { options, objects } = self;
        let is_pe = options.format == OutputFormat::Pe;
        let base_unit = if is_pe { 0x1_0000 } else { PAGE_SIZE };
        if options.base_address % base_unit != 0 {
            return Err(LinkError::InvalidOption(format!(
                "base address {:#x} is not a multiple of {base_unit:#x}",
                options.base_address
            )));
        }
        if !is_pe && !options.imports.is_empty() {
            return Err(LinkError::InvalidOption(
                "DLL imports can only be used with PE output".to_string(),
            ));
        }
        let base = options.base_address;

        // 1-2. セクション選択とシンボル解決
        let plan = plan_sections(&objects)?;
        let mut table = build_symbol_table(&objects, &plan)?;

        // GOT エントリが必要なシンボルを集める (出現順に番号付け)
        let mut got_slots: HashMap<GotKey, usize> = HashMap::new();
        for (oi, obj) in objects.iter().enumerate() {
            for (si, relocs) in obj.relocs.iter().enumerate() {
                if !matches!(plan[oi][si], Fate::Keep(_)) {
                    continue;
                }
                for r in relocs.iter().filter(|r| is_got_reloc(r.typ)) {
                    let key = got_key(&table, oi, r.sym, &obj.symbols[r.sym as usize]);
                    let n = got_slots.len();
                    got_slots.entry(key).or_insert(n);
                }
            }
        }

        // PE: 実際に参照された DLL インポートだけを集める。
        //   `foo` が未定義 → `call foo` 用のスタブが要る / `__imp_foo` が未定義 → IAT スロットだけ要る
        let is_undef = |t: &SymTable, n: &str| {
            t.index
                .get(n)
                .is_some_and(|&i| matches!(t.syms[i].def, Def::Undefined))
        };
        let mut used_imports: Vec<(String, Vec<String>)> = Vec::new();
        let mut thunk_syms: Vec<String> = Vec::new();
        {
            let mut seen: HashSet<&str> = HashSet::new();
            for d in &options.imports {
                let mut syms = Vec::new();
                for name in &d.symbols {
                    // 同じ名前が複数の DLL に載っていたら最初のものを使う
                    if !seen.insert(name.as_str()) {
                        continue;
                    }
                    let direct = is_undef(&table, name);
                    if direct || is_undef(&table, &format!("__imp_{name}")) {
                        syms.push(name.clone());
                        if direct {
                            thunk_syms.push(name.clone());
                        }
                    }
                }
                if !syms.is_empty() {
                    used_imports.push((d.dll.clone(), syms));
                }
            }
        }
        let idata_size = if used_imports.is_empty() {
            0
        } else {
            pe::build_imports(&used_imports, 0).bytes.len() as u64
        };

        // 3. レイアウト
        let layout = if is_pe {
            Layout::compute_pe(
                &objects,
                &plan,
                &table,
                got_slots.len(),
                base,
                thunk_syms.len(),
                idata_size,
            )?
        } else {
            Layout::compute(&objects, &plan, &table, got_slots.len(), base)
        };
        table.define_synthetic("_GLOBAL_OFFSET_TABLE_", layout.got_addr);
        table.define_synthetic("_etext", layout.etext);
        table.define_synthetic("etext", layout.etext);
        table.define_synthetic("_edata", layout.edata);
        table.define_synthetic("edata", layout.edata);
        table.define_synthetic("__bss_start", layout.bss_start);
        table.define_synthetic("_end", layout.end);
        table.define_synthetic("end", layout.end);
        let built_imports = match &layout.pe {
            Some(pl) if !used_imports.is_empty() => {
                let built = pe::build_imports(&used_imports, pl.idata_rva);
                for (name, rva) in &built.slots {
                    table.define_synthetic(&format!("__imp_{name}"), base + *rva as u64);
                }
                for (i, name) in thunk_syms.iter().enumerate() {
                    table.define_synthetic(name, pl.thunk_addr + 6 * i as u64);
                }
                Some(built)
            }
            _ => None,
        };

        // 4. セクション内容をイメージにコピー
        let mut image = vec![0u8; layout.file_size as usize];
        for (oi, obj) in objects.iter().enumerate() {
            for (si, sec) in obj.sections.iter().enumerate() {
                if let Fate::Keep(kind) = plan[oi][si] {
                    if kind != Kind::Bss && sec.size > 0 {
                        let off = layout.sec_foff[oi][si] as usize;
                        image[off..off + sec.size as usize].copy_from_slice(obj.section_data(si));
                    }
                }
            }
        }

        // PE: import テーブルとスタブ (jmp *[rip+disp32]) をイメージに書き込む
        if let (Some(pl), Some(built)) = (&layout.pe, &built_imports) {
            let off = pl.idata_foff as usize;
            image[off..off + built.bytes.len()].copy_from_slice(&built.bytes);
            let iat_of: HashMap<&str, u32> =
                built.slots.iter().map(|(n, r)| (n.as_str(), *r)).collect();
            for (i, name) in thunk_syms.iter().enumerate() {
                let at = pl.thunk_foff as usize + 6 * i;
                let thunk_addr = pl.thunk_addr + 6 * i as u64;
                let slot_addr = base + iat_of[name.as_str()] as u64;
                let disp = slot_addr as i64 - (thunk_addr as i64 + 6);
                image[at] = 0xFF;
                image[at + 1] = 0x25;
                image[at + 2..at + 6].copy_from_slice(&(disp as i32).to_le_bytes());
            }
        }

        // 5. リロケーション
        let res = Resolver {
            table: &table,
            layout: &layout,
        };
        let mut undefined: Vec<UndefinedRef> = Vec::new();
        let mut undefined_seen: HashSet<(String, usize)> = HashSet::new();

        for (oi, obj) in objects.iter().enumerate() {
            for (si, relocs) in obj.relocs.iter().enumerate() {
                let Fate::Keep(kind) = plan[oi][si] else {
                    continue;
                };
                if relocs.is_empty() {
                    continue;
                }
                let sec = &obj.sections[si];
                if kind == Kind::Bss {
                    return Err(LinkError::Parse {
                        object: obj.name.clone(),
                        message: format!("relocations against NOBITS section `{}'", sec.name),
                    });
                }

                for r in relocs {
                    let sym = &obj.symbols[r.sym as usize];
                    if sym.typ == STT_TLS || sym.typ == STT_GNU_IFUNC {
                        return Err(LinkError::Unsupported {
                            object: obj.name.clone(),
                            message: format!("TLS/IFUNC symbol `{}'", sym.name),
                        });
                    }
                    let Some(width) = reloc_width(r.typ) else {
                        return Err(LinkError::Unsupported {
                            object: obj.name.clone(),
                            message: format!(
                                "relocation type {} in section `{}'",
                                r.typ, sec.name
                            ),
                        });
                    };
                    if r.offset.checked_add(width).is_none_or(|e| e > sec.size) {
                        return Err(LinkError::Parse {
                            object: obj.name.clone(),
                            message: format!(
                                "relocation offset {:#x} is outside section `{}'",
                                r.offset, sec.name
                            ),
                        });
                    }

                    let s = match res.sym_addr(oi, sym) {
                        SymAddr::Addr(a) => a as i128,
                        SymAddr::Undefined => {
                            if undefined_seen.insert((sym.name.clone(), oi)) {
                                undefined.push(UndefinedRef {
                                    symbol: sym.name.clone(),
                                    object: obj.name.clone(),
                                });
                            }
                            continue;
                        }
                    };
                    let a = r.addend as i128;
                    let p = (layout.sec_addr[oi][si] + r.offset) as i128;
                    let at = (layout.sec_foff[oi][si] + r.offset) as usize;
                    let overflow = |value: i128| LinkError::RelocationOverflow {
                        object: obj.name.clone(),
                        section: sec.name.clone(),
                        offset: r.offset,
                        reloc: reloc_name(r.typ),
                        symbol: display_sym(obj, sym),
                        value,
                    };

                    match r.typ {
                        R_X86_64_NONE => {}
                        R_X86_64_64 => {
                            image[at..at + 8].copy_from_slice(&((s + a) as u64).to_le_bytes());
                        }
                        R_X86_64_PC64 => {
                            image[at..at + 8].copy_from_slice(&((s + a - p) as u64).to_le_bytes());
                        }
                        // 静的リンクなので PLT は不要。PLT32 は PC32 と同じ計算になる。
                        R_X86_64_PC32 | R_X86_64_PLT32 => {
                            let v = s + a - p;
                            if !fits_i32(v) {
                                return Err(overflow(v));
                            }
                            image[at..at + 4].copy_from_slice(&(v as i32).to_le_bytes());
                        }
                        R_X86_64_32 => {
                            let v = s + a;
                            if v < 0 || v > u32::MAX as i128 {
                                return Err(overflow(v));
                            }
                            image[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
                        }
                        R_X86_64_32S => {
                            let v = s + a;
                            if !fits_i32(v) {
                                return Err(overflow(v));
                            }
                            image[at..at + 4].copy_from_slice(&(v as i32).to_le_bytes());
                        }
                        // COFF の ADDR32NB: イメージベースからの 32 ビット RVA
                        R_PE_ADDR32NB => {
                            let v = s + a - base as i128;
                            if v < 0 || v > u32::MAX as i128 {
                                return Err(overflow(v));
                            }
                            image[at..at + 4].copy_from_slice(&(v as u32).to_le_bytes());
                        }
                        // GOT 経由のロード: スロットにシンボルの絶対アドレスを置き、
                        // 命令にはスロットへの PC 相対オフセットを書く (緩和はしない)。
                        _ => {
                            let key = got_key(&table, oi, r.sym, sym);
                            let slot = got_slots[&key] as u64;
                            let slot_addr = layout.got_addr + 8 * slot;
                            let goff = (layout.got_foff + 8 * slot) as usize;
                            image[goff..goff + 8].copy_from_slice(&(s as u64).to_le_bytes());
                            let v = slot_addr as i128 + a - p;
                            if !fits_i32(v) {
                                return Err(overflow(v));
                            }
                            image[at..at + 4].copy_from_slice(&(v as i32).to_le_bytes());
                        }
                    }
                }
            }
        }
        if !undefined.is_empty() {
            return Err(LinkError::UndefinedSymbols(undefined));
        }

        // 6. エントリポイントと ELF ヘッダ
        let entry = table
            .index
            .get(&options.entry)
            .and_then(|&gi| res.def_addr(gi))
            .ok_or_else(|| LinkError::EntryNotFound(options.entry.clone()))?;
        match &layout.pe {
            Some(pl) => {
                let entry_rva = entry
                    .checked_sub(base)
                    .filter(|&r| r <= u32::MAX as u64)
                    .ok_or_else(|| {
                        LinkError::InvalidOption(format!(
                            "entry address {entry:#x} is outside the image"
                        ))
                    })?;
                pe::write_headers(
                    &mut image,
                    &PeHeaderInfo {
                        image_base: base,
                        entry_rva: entry_rva as u32,
                        sections: &pl.sections,
                        size_of_headers: pl.size_of_headers,
                        size_of_image: pl.size_of_image,
                        size_of_code: pl.size_of_code,
                        size_of_initialized_data: pl.size_of_init,
                        size_of_uninitialized_data: pl.size_of_uninit,
                        base_of_code: pl.base_of_code,
                        subsystem: options.subsystem,
                        import_dir: built_imports.as_ref().map_or((0, 0), |b| b.dir),
                        iat: built_imports.as_ref().map_or((0, 0), |b| b.iat),
                    },
                );
            }
            None => write_headers(&mut image, &layout, entry),
        }

        let mut symbols: Vec<OutputSymbol> = (0..table.syms.len())
            .filter_map(|gi| {
                res.def_addr(gi).map(|addr| OutputSymbol {
                    name: table.syms[gi].name.clone(),
                    addr,
                })
            })
            .collect();
        symbols.sort_by(|a, b| a.addr.cmp(&b.addr).then_with(|| a.name.cmp(&b.name)));

        Ok(LinkOutput {
            image,
            format: options.format,
            entry,
            sections: layout.out_sections.clone(),
            symbols,
        })
    }
}

fn write_headers(image: &mut [u8], layout: &Layout, entry: u64) {
    let mut h: Vec<u8> = Vec::new();
    // e_ident
    h.extend_from_slice(b"\x7fELF");
    h.extend_from_slice(&[2, 1, 1, 0]); // 64bit, LE, version 1, SYSV
    h.extend_from_slice(&[0u8; 8]);
    h.extend_from_slice(&2u16.to_le_bytes()); // e_type = ET_EXEC
    h.extend_from_slice(&62u16.to_le_bytes()); // e_machine = x86-64
    h.extend_from_slice(&1u32.to_le_bytes()); // e_version
    h.extend_from_slice(&entry.to_le_bytes());
    h.extend_from_slice(&EHDR_SIZE.to_le_bytes()); // e_phoff
    h.extend_from_slice(&0u64.to_le_bytes()); // e_shoff
    h.extend_from_slice(&0u32.to_le_bytes()); // e_flags
    h.extend_from_slice(&(EHDR_SIZE as u16).to_le_bytes());
    h.extend_from_slice(&(PHDR_SIZE as u16).to_le_bytes());
    h.extend_from_slice(&(layout.segments.len() as u16).to_le_bytes());
    h.extend_from_slice(&64u16.to_le_bytes()); // e_shentsize
    h.extend_from_slice(&0u16.to_le_bytes()); // e_shnum
    h.extend_from_slice(&0u16.to_le_bytes()); // e_shstrndx
    debug_assert_eq!(h.len() as u64, EHDR_SIZE);

    for seg in &layout.segments {
        h.extend_from_slice(&PT_LOAD.to_le_bytes());
        h.extend_from_slice(&seg.flags.to_le_bytes());
        h.extend_from_slice(&seg.offset.to_le_bytes());
        h.extend_from_slice(&seg.vaddr.to_le_bytes());
        h.extend_from_slice(&seg.vaddr.to_le_bytes()); // p_paddr
        h.extend_from_slice(&seg.filesz.to_le_bytes());
        h.extend_from_slice(&seg.memsz.to_le_bytes());
        h.extend_from_slice(&PAGE_SIZE.to_le_bytes());
    }
    image[..h.len()].copy_from_slice(&h);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn align_up_works() {
        assert_eq!(align_up(0, 8), 0);
        assert_eq!(align_up(1, 8), 8);
        assert_eq!(align_up(8, 8), 8);
        assert_eq!(align_up(9, 16), 16);
        assert_eq!(align_up(5, 0), 5);
        assert_eq!(align_up(5, 1), 5);
    }

    #[test]
    fn rejects_unaligned_base_address() {
        let opts = LinkOptions {
            entry: "_start".into(),
            base_address: 0x400123,
            ..Default::default()
        };
        let err = Linker::with_options(opts).link().unwrap_err();
        assert!(matches!(err, LinkError::InvalidOption(_)));
    }

    fn pe_options_with_import() -> LinkOptions {
        let mut o = LinkOptions::pe();
        o.imports.push(DllImport::new("kernel32.dll", ["foo"]));
        o
    }

    #[test]
    fn pe_rejects_base_not_multiple_of_64k() {
        let mut o = LinkOptions::pe();
        o.base_address = 0x401000; // ELF なら 4K 境界で通るが、PE は 64K 境界が必要
        let err = Linker::with_options(o).link().unwrap_err();
        assert!(matches!(err, LinkError::InvalidOption(_)));
    }

    #[test]
    fn imports_are_rejected_for_elf_output() {
        let mut o = LinkOptions::default();
        o.imports.push(DllImport::new("kernel32.dll", ["foo"]));
        let err = Linker::with_options(o).link().unwrap_err();
        assert!(matches!(err, LinkError::InvalidOption(_)));
    }

    #[test]
    fn pe_without_import_leaves_symbol_undefined() {
        let mut l = Linker::with_options(LinkOptions::pe());
        l.add_object("tiny.obj", coff::tests::tiny_object()).unwrap();
        let err = l.link().unwrap_err();
        assert!(matches!(err, LinkError::UndefinedSymbols(_)));
    }

    #[test]
    fn pe_links_coff_object_through_import_thunk() {
        let mut l = Linker::with_options(pe_options_with_import());
        l.add_object("tiny.obj", coff::tests::tiny_object()).unwrap();
        let out = l.link().unwrap();
        let img = &out.image;
        assert_eq!(out.format, OutputFormat::Pe);

        // DOS / PE ヘッダ
        assert_eq!(&img[0..2], b"MZ");
        let lfanew = u32::from_le_bytes(img[0x3c..0x40].try_into().unwrap()) as usize;
        assert_eq!(&img[lfanew..lfanew + 4], b"PE\0\0");
        assert_eq!(u16::from_le_bytes(img[lfanew + 4..lfanew + 6].try_into().unwrap()), 0x8664);
        // 全セクションの RVA / ファイルオフセットが境界に揃っている
        let nsec = u16::from_le_bytes(img[lfanew + 6..lfanew + 8].try_into().unwrap()) as usize;
        assert_eq!(nsec, 2); // .text + .idata
        assert_eq!(img.len() % 0x200, 0);

        // `call foo` の飛び先が import スタブ (FF 25) を指している
        let text = out.sections.iter().find(|s| s.name == ".text").unwrap();
        assert_eq!(text.addr, DEFAULT_BASE_ADDRESS + 0x1000);
        let at = text.file_offset as usize;
        assert_eq!(img[at], 0xE8);
        let rel = i32::from_le_bytes(img[at + 1..at + 5].try_into().unwrap());
        let target = (text.addr as i64 + 5 + rel as i64) as u64;
        let foo = out.symbols.iter().find(|s| s.name == "foo").unwrap().addr;
        assert_eq!(target, foo);
        let thunk = (text.file_offset + (foo - text.addr)) as usize;
        assert_eq!(&img[thunk..thunk + 2], &[0xFF, 0x25]);
        assert_eq!(out.entry, text.addr);
    }
}
