use super::*;


#[repr(C)]
pub struct Parser {
    pub chars: *mut u8,
    pub chars_len: usize,
    pub pos: usize,
    pub group_count: usize,
}

#[repr(C)]
enum OpKind {
    NONE,
    SOME,
}

#[repr(C)]
struct CharOpt {
    value: u8,
    kind: OpKind,
}


unsafe extern "C" {
    // C
    fn peek(this: *mut Parser) -> CharOpt;
    fn peek2(this: *mut Parser) -> CharOpt;
    fn bump(this: *mut Parser) -> CharOpt;
    fn match_chr(this: *mut Parser, chr: u8) -> u8;
    fn match_chr_2(this: *mut Parser, chr: u8) -> u8;
    fn unmatch_bump(this: *mut Parser, chr: u8) -> u8;
}

#[link(name = "regex", kind = "static")]
unsafe extern "C" {
    // Asm
    fn change_byte_chr(this: *mut Parser) -> u8;
}

macro_rules! IsBoolean {
    ($func:expr) => {
        $func == 1
    };
}


impl Parser {
    pub fn new(pattern: &str) -> Self {
        Self {
            chars: pattern.as_ptr() as *mut u8,
            chars_len: pattern.len() as usize,
            pos: 0, 
            group_count: 0 
        }
    }

    pub fn parse_alt(&mut self) -> Result<Node, RegexError> {
        unsafe {
            let mut branches = vec![self.parse_concat()?];
            while IsBoolean!(match_chr(self, b'|')) {
                bump(self);
                branches.push(self.parse_concat()?);
            }
            if branches.len() == 1 {
                Ok(branches.pop().unwrap())
            } else {
                Ok(Node::Alt(branches))
            }
        }
    }

    fn parse_concat(&mut self) -> Result<Node, RegexError> {
        unsafe {
            let mut nodes = Vec::new();
            while matches!(peek(self).kind, OpKind::SOME) {
                let c: u8 = peek(self).value;
                if c == b'|' || c == b')' {
                    break;
                }
                nodes.push(self.parse_repeat()?);
            }
            Ok(Node::Concat(nodes))
        }
    }

    fn parse_repeat(&mut self) -> Result<Node, RegexError> {
        unsafe {
            let atom = self.parse_atom()?;
            if matches!(peek(self).kind, OpKind::NONE) {
                return Ok(atom);
            }
            match peek(self).value {
                b'*' => {
                    bump(self);
                    Ok(Node::Repeat(Box::new(atom), 0, None))
                }
                b'+' => {
                    bump(self);
                    Ok(Node::Repeat(Box::new(atom), 1, None))
                }
                b'?' => {
                    bump(self);
                    Ok(Node::Repeat(Box::new(atom), 0, Some(1)))
                }
                b'{' => self.parse_bound(atom),
                _ => Ok(atom),
            }
        }
    }

    fn parse_bound(&mut self, atom: Node) -> Result<Node, RegexError> {
        unsafe {
            let checkpoint = self.pos;
            bump(self); // '{'
            let mut min_s = String::new();
            while matches!(peek(self).kind, OpKind::SOME) {
                let c: u8 = peek(self).value;
                if is_byte_digit(c) == 1 {
                    min_s.push(c as char);
                    bump(self);
                } else {
                    break;
                }
            }
            if min_s.is_empty() {
                // "{" が数量子として不正 -> リテラルの '{' として扱う
                self.pos = checkpoint + 1;
                return Ok(Node::Concat(vec![atom, Node::Char('{')]));
            }
            let min: usize = min_s.parse().unwrap();
            let max: Option<usize> =
                if IsBoolean!(match_chr(self, b',')) {
                        bump(self);
                        let mut max_s = String::new();
                        while matches!(peek(self).kind, OpKind::SOME) {
                            let c: u8 = peek(self).value;
                            if is_byte_digit(c) == 1 {
                                max_s.push(c as char);
                                bump(self);
                            } else {
                                break;
                            }
                        }
                        if max_s.is_empty() { None } else { Some(max_s.parse().unwrap()) }
                } else {
                    Some(min)
                };
            if IsBoolean!(unmatch_bump(self, b'}')) {
                return Err(RegexError("'{' に対応する '}' がありません".into()));
            }
            Ok(Node::Repeat(Box::new(atom), min, max))
        }
    }

    fn parse_atom(&mut self) -> Result<Node, RegexError> {
        unsafe {
            if matches!(bump(self).kind, OpKind::NONE) {
                return Err(RegexError("パターンが予期せず終了しました".into()));
            }
            match peek(self).value {
                b'(' => {
                    let mut capturing = true;
                    if IsBoolean!(match_chr(self, b'?'))
                        && IsBoolean!(match_chr_2(self, b':'))
                    {
                        self.pos += 2;
                        capturing = false;
                    }
                    let idx = if capturing {
                        self.group_count += 1;
                        Some(self.group_count)
                    } else {
                        None
                    };
                    let inner = self.parse_alt()?;
                    if IsBoolean!(unmatch_bump(self, b')')) {
                        return Err(RegexError("'(' に対応する ')' がありません".into()));
                    }
                    match idx {
                        Some(i) => Ok(Node::Group(Box::new(inner), i)),
                        None => Ok(inner),
                    }
                }
                b'.' => Ok(Node::Any),
                b'^' => Ok(Node::Start),
                b'$' => Ok(Node::End),
                b'[' => self.parse_class(),
                b'\\' => self.parse_escape(),
                c => Ok(Node::Char(c as char)),
            }
        }
    }

    fn parse_escape(&mut self) -> Result<Node, RegexError> {
        unsafe {
            if matches!(bump(self).kind, OpKind::NONE) {
                return Err(RegexError("末尾がバックスラッシュで終わっています".into()));
            }

            // エスケープされる文字
            if matches!(peek(self).kind, OpKind::NONE) {
                return Err(RegexError(
                    "末尾がバックスラッシュで終わっています".into()
                ));
            }

            let c = peek(self).value;

            // エスケープ対象も消費する
            bump(self);

            match c {
                b'd' => Ok(Node::Class(vec![('0', '9')], false)),
                b'D' => Ok(Node::Class(vec![('0', '9')], true)),
                b'w' => Ok(Node::Class(
                    vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
                    false,
                )),
                b'W' => Ok(Node::Class(
                    vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
                    true,
                )),
                b's' => Ok(Node::Class(
                    vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
                    false,
                )),
                b'S' => Ok(Node::Class(
                    vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
                    true,
                )),
                b'n' => Ok(Node::Char('\n')),
                b't' => Ok(Node::Char('\t')),
                b'r' => Ok(Node::Char('\r')),
                c => Ok(Node::Char(c as char)), // \. \* \\ など、そのままリテラル化
            }
        }
    }

    fn parse_class(&mut self) -> Result<Node, RegexError> {
        unsafe {
            let mut negated = false;
            if IsBoolean!(match_chr(self, b'^')) {
                negated = true;
                bump(self);
            }
            let mut ranges = Vec::new();
            let mut first = true;
            loop {
                if IsBoolean!(match_chr(self, b']')) && first != true {
                    bump(self);
                    break;
                } else if matches!(peek(self).kind, OpKind::NONE) {
                    return Err(RegexError("'[' に対応する ']' がありません".into()))
                }
                first = false;
                if IsBoolean!(match_chr(self, b'\\'))
                    && matches!(peek2(self).kind, OpKind::SOME)
                    && matches!(peek2(self).value, b'd' | b'w' | b's')
                {
                    bump(self);
                    let kind: u8 = 
                        if matches!(bump(self).kind, OpKind::SOME) {
                            peek(self).value
                        } else {
                            panic!()
                        };
                    ranges.extend(shorthand_class_ranges(kind));
                    continue;
                }
                let c1 = self.parse_class_char()?;
                if IsBoolean!(match_chr(self, b'-'))
                    && matches!(peek2(self).kind, OpKind::SOME)
                    && peek2(self).value != b']'
                {
                    bump(self); // '-'
                    let c2 = self.parse_class_char()?;
                    ranges.push((c1, c2));
                } else {
                    ranges.push((c1, c1));
                }
            }
            Ok(Node::Class(ranges, negated))
        }
    }

    fn parse_class_char(&mut self) -> Result<char, RegexError> {
        unsafe {
            if matches!(bump(self).kind, OpKind::NONE) {
                return Err(RegexError("'[' に対応する ']' がありません".into()));
            }
            match peek(self).value {
                b'\\' => {
                    if matches!(bump(self).kind, OpKind::NONE) {
                        return Err(RegexError("末尾がバックスラッシュで終わっています".into()));
                    }
                    // c/all.h
                    Ok(change_byte_chr(self) as char)
                }
                c => Ok(c as char)
            }
        }
    }
}

fn shorthand_class_ranges(kind: u8) -> Vec<(char, char)> {
    match kind {
        b'd' => vec![('0', '9')],
        b'w' => vec![('a', 'z'), ('A', 'Z'), ('0', '9'), ('_', '_')],
        b's' => vec![(' ', ' '), ('\t', '\t'), ('\n', '\n'), ('\r', '\r')],
        _ => vec![],
    }
}