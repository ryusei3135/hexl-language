//! ドキュメントの管理、`#include` の解決、ワークスペース内の `.hexl` ファイルの走査

use crate::scan::{scan, Include, ScanResult, TokKind};
use crate::util::{path_to_uri, uri_to_path};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::SystemTime;

pub struct Doc {
    pub uri: String,
    pub path: Option<PathBuf>,
    pub lines: Vec<Vec<char>>,
    pub r: ScanResult,
}

impl Doc {
    pub fn new(uri: &str, text: &str) -> Self {
        let lines = text
            .split('\n')
            .map(|l| l.trim_end_matches('\r').chars().collect())
            .collect();
        Doc { uri: uri.to_string(), path: uri_to_path(uri), lines, r: scan(text) }
    }

    /// カーソル(UTF-16 の列)より前の、その行の文字列
    pub fn line_prefix(&self, line: usize, ch16: usize) -> Vec<char> {
        let Some(l) = self.lines.get(line) else { return Vec::new() };
        let idx = crate::util::utf16_to_char_idx(l, ch16);
        l[..idx].to_vec()
    }

    /// カーソル位置のトークンの添字(名前 / 予約語 / プリプロセッサ / 文字列)
    pub fn tok_idx_at(&self, line: usize, ch: usize) -> Option<usize> {
        let hit = |strict: bool| {
            self.r.tkns.iter().position(|t| {
                t.line == line
                    && t.col <= ch
                    && (if strict { ch < t.col + t.len } else { ch == t.col + t.len })
                    && matches!(
                        t.kind,
                        TokKind::Name | TokKind::Keyword | TokKind::Preproc | TokKind::Str
                    )
            })
        };
        hit(true).or_else(|| hit(false))
    }

    pub fn file_name(&self) -> String {
        self.path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| self.uri.clone())
    }
}

#[derive(Default)]
pub struct Index {
    pub open: HashMap<String, Rc<Doc>>,
    disk: HashMap<PathBuf, (SystemTime, Rc<Doc>)>,
    pub roots: Vec<PathBuf>,
    pub library: Option<PathBuf>,
    ws_files: Option<Vec<PathBuf>>,
}

impl Index {
    pub fn set_open(&mut self, uri: &str, text: &str) -> Rc<Doc> {
        let d = Rc::new(Doc::new(uri, text));
        self.open.insert(uri.to_string(), Rc::clone(&d));
        d
    }

    pub fn close(&mut self, uri: &str) {
        self.open.remove(uri);
    }

    pub fn doc(&mut self, uri: &str) -> Option<Rc<Doc>> {
        if let Some(d) = self.open.get(uri) {
            return Some(Rc::clone(d));
        }
        let path = uri_to_path(uri)?;
        self.load_path(&path)
    }

    /// 開いているドキュメントを優先し、なければディスクから読む(更新時刻でキャッシュ)
    pub fn load_path(&mut self, path: &Path) -> Option<Rc<Doc>> {
        if let Some(d) = self.open.values().find(|d| d.path.as_deref() == Some(path)) {
            return Some(Rc::clone(d));
        }
        let mtime = std::fs::metadata(path).ok()?.modified().ok()?;
        if let Some((t, d)) = self.disk.get(path) {
            if *t == mtime {
                return Some(Rc::clone(d));
            }
        }
        let text = std::fs::read_to_string(path).ok()?;
        let d = Rc::new(Doc::new(&path_to_uri(path), &text));
        self.disk.insert(path.to_path_buf(), (mtime, Rc::clone(&d)));
        Some(d)
    }

    /// 標準ライブラリ(`std::"..."`)を探すディレクトリ
    fn library_dirs(&self, from: Option<&Path>) -> Vec<PathBuf> {
        let mut v: Vec<PathBuf> = Vec::new();
        if let Some(l) = &self.library {
            v.push(l.clone());
        }
        if let Ok(l) = std::env::var("HEXL_LIBRARY") {
            v.push(PathBuf::from(l));
        }
        for r in &self.roots {
            v.push(r.join("library"));
        }
        if let Some(dir) = from.and_then(|p| p.parent()) {
            for a in dir.ancestors().take(8) {
                v.push(a.join("library"));
            }
        }
        v
    }

    /// `#include` が指すファイルを探す。
    /// 通常のパスは本来「コンパイルを始めたファイルのディレクトリ(ROOT)」基準だが、
    /// LSP からは起点が分からないので、このファイルのディレクトリとその親、
    /// ワークスペースのルートを順に試す。
    pub fn resolve_include(&self, from: Option<&Path>, inc: &Include) -> Option<PathBuf> {
        if inc.is_std {
            return self
                .library_dirs(from)
                .into_iter()
                .map(|d| d.join(&inc.path))
                .find(|p| p.is_file());
        }
        let rel = Path::new(&inc.path);
        if rel.is_absolute() {
            return rel.is_file().then(|| rel.to_path_buf());
        }
        let mut bases: Vec<PathBuf> = Vec::new();
        if let Some(dir) = from.and_then(|p| p.parent()) {
            bases.extend(dir.ancestors().take(8).map(Path::to_path_buf));
        }
        bases.extend(self.roots.iter().cloned());
        bases.into_iter().map(|b| b.join(rel)).find(|p| p.is_file())
    }

    /// 直接の `#include`(解決できたものだけ。`.c` / `.h` は解析しない)
    pub fn direct_includes(&mut self, doc: &Doc) -> Vec<(Include, Rc<Doc>)> {
        let mut output = Vec::new();
        for inc in &doc.r.includes {
            if !inc.path.ends_with(".hexl") {
                continue;
            }
            if let Some(p) = self.resolve_include(doc.path.as_deref(), inc) {
                if let Some(d) = self.load_path(&p) {
                    output.push((inc.clone(), d));
                }
            }
        }
        output
    }

    /// 自分自身 + `#include` を再帰でたどった全ドキュメント(重複なし)。
    /// 各要素の `Option<String>` は、そこへ入った最初の `#include` のモジュール名
    pub fn scope(&mut self, doc: &Rc<Doc>) -> Vec<(Option<String>, Rc<Doc>)> {
        let mut seen: HashSet<String> = HashSet::new();
        seen.insert(doc.uri.clone());
        let mut output = vec![(None, Rc::clone(doc))];
        let mut queue: Vec<(Option<String>, Rc<Doc>, usize)> = vec![(None, Rc::clone(doc), 0)];
        while let Some((_, d, depth)) = queue.pop() {
            if depth >= 6 || output.len() > 80 {
                continue;
            }
            for (inc, child) in self.direct_includes(&d) {
                if seen.insert(child.uri.clone()) {
                    output.push((Some(inc.module.clone()), Rc::clone(&child)));
                    queue.push((Some(inc.module), child, depth + 1));
                }
            }
        }
        output
    }

    pub fn scope_docs(&mut self, doc: &Rc<Doc>) -> Vec<Rc<Doc>> {
        self.scope(doc).into_iter().map(|(_, d)| d).collect()
    }

    /// ワークスペース内の `.hexl` ファイル
    pub fn workspace_files(&mut self) -> Vec<PathBuf> {
        if self.ws_files.is_none() {
            let mut files = Vec::new();
            for r in self.roots.clone() {
                walk(&r, &mut files, 0);
            }
            self.ws_files = Some(files);
        }
        self.ws_files.clone().unwrap_or_default()
    }

    pub fn invalidate_workspace(&mut self) {
        self.ws_files = None;
    }

    /// 開いている全ドキュメント + ワークスペースの全 `.hexl` + 現在のスコープ
    pub fn all_docs(&mut self, current: Option<&Rc<Doc>>) -> Vec<Rc<Doc>> {
        let mut seen: HashSet<String> = HashSet::new();
        let mut output: Vec<Rc<Doc>> = Vec::new();
        let mut add = |d: Rc<Doc>, output: &mut Vec<Rc<Doc>>| {
            if seen.insert(d.uri.clone()) {
                output.push(d);
            }
        };
        if let Some(c) = current {
            for d in self.scope_docs(c) {
                add(d, &mut output);
            }
        }
        let opens: Vec<Rc<Doc>> = self.open.values().cloned().collect();
        for d in opens {
            add(d, &mut output);
        }
        for p in self.workspace_files() {
            if let Some(d) = self.load_path(&p) {
                add(d, &mut output);
            }
        }
        output
    }
}

fn walk(dir: &Path, output: &mut Vec<PathBuf>, depth: usize) {
    if depth > 12 || output.len() > 3000 {
        return;
    }
    let Ok(rd) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = rd.flatten().collect();
    entries.sort_by_key(|e| e.file_name());
    for e in entries {
        let name = e.file_name().to_string_lossy().into_owned();
        let p = e.path();
        if p.is_dir() {
            if name.starts_with('.') || name == "target" || name == "node_modules" {
                continue;
            }
            walk(&p, output, depth + 1);
        } else if p.extension().map_or(false, |x| x == "hexl") {
            output.push(p);
        }
    }
}
