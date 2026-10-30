//! URI/パス変換と UTF-16 位置の補助関数

use std::path::{Path, PathBuf};

fn hex(c: u8) -> Option<u8> {
    match c {
        b'0'..=b'9' => Some(c - b'0'),
        b'a'..=b'f' => Some(c - b'a' + 10),
        b'A'..=b'F' => Some(c - b'A' + 10),
        _ => None,
    }
}

fn percent_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let (Some(h), Some(l)) = (hex(b[i + 1]), hex(b[i + 2])) {
                out.push(h * 16 + l);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

pub fn uri_to_path(uri: &str) -> Option<PathBuf> {
    let rest = uri.strip_prefix("file://")?;
    // `file://host/path` は扱わない(ローカルのみ)
    let mut p = percent_decode(rest);
    // `/C:/dir` -> `C:/dir`
    let b = p.as_bytes();
    if b.len() >= 3 && b[0] == b'/' && b[2] == b':' && b[1].is_ascii_alphabetic() {
        p.remove(0);
    }
    Some(PathBuf::from(p))
}

pub fn path_to_uri(path: &Path) -> String {
    let s = path.to_string_lossy().replace('\\', "/");
    let mut out = String::from("file://");
    let b = s.as_bytes();
    if b.len() >= 2 && b[1] == b':' {
        out.push('/');
    }
    for &c in s.as_bytes() {
        if c.is_ascii_alphanumeric() || b"-._~/:".contains(&c) {
            out.push(c as char);
        } else {
            out.push_str(&format!("%{:02X}", c));
        }
    }
    out
}

/// UTF-16 の列 -> `chars` の添字
pub fn utf16_to_char_idx(chars: &[char], col16: usize) -> usize {
    let mut n = 0;
    for (i, c) in chars.iter().enumerate() {
        if n >= col16 {
            return i;
        }
        n += c.len_utf16();
    }
    chars.len()
}

pub fn is_ident_start(c: char) -> bool {
    c.is_ascii_alphabetic() || c == '_'
}
pub fn is_ident_char(c: char) -> bool {
    c.is_ascii_alphanumeric() || c == '_'
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn uri_roundtrip() {
        let p = uri_to_path("file:///home/a%20b/x.hexl").unwrap();
        assert_eq!(p, PathBuf::from("/home/a b/x.hexl"));
        assert_eq!(path_to_uri(&p), "file:///home/a%20b/x.hexl");
    }

    #[test]
    fn windows_drive() {
        let p = uri_to_path("file:///C:/proj/a.hexl").unwrap();
        assert_eq!(p, PathBuf::from("C:/proj/a.hexl"));
    }

    #[test]
    fn utf16_cols() {
        let cs: Vec<char> = "a😀b".chars().collect();
        assert_eq!(utf16_to_char_idx(&cs, 0), 0);
        assert_eq!(utf16_to_char_idx(&cs, 1), 1);
        assert_eq!(utf16_to_char_idx(&cs, 3), 2);
    }
}
