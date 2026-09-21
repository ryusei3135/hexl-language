use super::*;

fn sanitize_label_part(
    part: &str
) -> String {
    let mut sanitized = String::new();
    for ch in part.chars() {
        if matches!(ch, 'a'..='z' | 'A'..='Z' | '0'..='9' | '_') {
            sanitized.push(ch);
        }
    }
    if sanitized.is_empty() {
        sanitized.push('a');
    }
    sanitized
}

fn join_sanitized_parts(
    parts: &[&str]
) -> String {
    parts
        .iter()
        .filter(|part| !part.is_empty())
        .map(|part| sanitize_label_part(part))
        .collect::<Vec<_>>()
        .join("_")
}

/// 通常の関数ラベルを生成する。
/// 記号は無視して、英字・数字・`_`だけを残す。
pub fn emit_fn_name_id(base: &str) -> String {
    let parts: Vec<&str> = base.split("::").collect();
    format!("hexl_{}_fn", join_sanitized_parts(&parts))
}

/// ジェネリクス関数のラベルを生成する。
/// 型引数の情報もラベルに含めるが、記号は無視して英字・数字・`_`だけを残す。
pub fn emit_generic_fn_name_id(
    base: &str, 
    generic_args: &[String]
) -> String {
    let base_label = join_sanitized_parts(
        &base
            .split("::")
            .collect::<Vec<_>>()
        );
    let generic_label = generic_args
        .iter()
        .map(|arg| sanitize_label_part(arg))
        .collect::<Vec<_>>()
        .join("_");

    if generic_label.is_empty() {
        format!("hexl_{}_fn", base_label)
    } else {
        format!("hexl_{}_{}_gen", base_label, generic_label)
    }
}