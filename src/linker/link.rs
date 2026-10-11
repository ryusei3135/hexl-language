//! リンカ本体。
//!
//! 処理の流れ:
//!
//! 1. セクション選択 (`SHF_ALLOC` のみ採用、COMDAT の重複を除去)
//! 2. グローバルシンボルの解決 (強/弱/COMMON のルールに従う)
//! 3. レイアウト決定 (`.text` / `.rodata` / `.got` + `.data` / `.bss` の 3 セグメント)
//! 4. リロケーション適用 (オーバーフロー検査付き)
//! 5. ELF 実行ファイル (ET_EXEC) の書き出し

use std::collections::{HashMap, HashSet};
use std::path::Path;

use super::elf::*;
use super::error::{LinkError, Result, UndefinedRef};

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

/// リンクの設定。
#[derive(Debug, Clone)]
pub struct LinkOptions {
    /// エントリポイントのシンボル名 (既定: `_start`)
    pub entry: String,
    /// ロードアドレス (4096 の倍数であること)
    pub base_address: u64,
}

impl Default for LinkOptions {
    fn default() -> Self {
        LinkOptions {
            entry: "_start".to_string(),
            base_address: DEFAULT_BASE_ADDRESS,
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
    /// ELF 実行ファイルのバイト列
    pub image: Vec<u8>,
    /// エントリポイントの仮想アドレス
    pub entry: u64,
    pub sections: Vec<OutputSection>,
    /// アドレス順 (同一アドレスは名前順) に整列したシンボル
    pub symbols: Vec<OutputSymbol>,
}

impl LinkOutput {
    /// ファイルに書き出す (Unix では実行権限 0755 を付ける)。
    pub fn write_to_file(&self, path: impl AsRef<Path>) -> Result<()> {
        let path = path.as_ref();
        let io_err = |source| LinkError::Io {
            path: path.display().to_string(),
            source,
        };
        std::fs::write(path, &self.image).map_err(io_err)?;
        #[cfg(unix)]
        {
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
}

impl Layout {
    fn compute(
        objects: &[ElfObject],
        plan: &[Vec<Fate>],
        table: &SymTable,
        got_count: usize,
        base: u64,
    ) -> Layout {
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
        }
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
        R_X86_64_PC32
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
    pub fn add_object(&mut self, name: impl Into<String>, data: Vec<u8>) -> Result<()> {
        self.objects.push(ElfObject::parse(name, data)?);
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
        if options.base_address % PAGE_SIZE != 0 {
            return Err(LinkError::InvalidOption(format!(
                "base address {:#x} is not a multiple of {PAGE_SIZE:#x}",
                options.base_address
            )));
        }

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

        // 3. レイアウト
        let layout = Layout::compute(
            &objects,
            &plan,
            &table,
            got_slots.len(),
            options.base_address,
        );
        table.define_synthetic("_GLOBAL_OFFSET_TABLE_", layout.got_addr);
        table.define_synthetic("_etext", layout.etext);
        table.define_synthetic("etext", layout.etext);
        table.define_synthetic("_edata", layout.edata);
        table.define_synthetic("edata", layout.edata);
        table.define_synthetic("__bss_start", layout.bss_start);
        table.define_synthetic("_end", layout.end);
        table.define_synthetic("end", layout.end);

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
        write_headers(&mut image, &layout, entry);

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
        };
        let err = Linker::with_options(opts).link().unwrap_err();
        assert!(matches!(err, LinkError::InvalidOption(_)));
    }
}
