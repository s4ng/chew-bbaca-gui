//! 작은 공용 헬퍼.

use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

/// 타임스탬프는 전부 UTC RFC3339 문자열로 저장한다.
/// (로컬 시각 변환은 표시 직전에 프런트에서 한다 — DB 에 타임존을 섞지 않는다.)
pub fn now_iso() -> String {
    OffsetDateTime::now_utc()
        .format(&Rfc3339)
        .unwrap_or_else(|_| String::from("1970-01-01T00:00:00Z"))
}

/// POSIX 셸 작은따옴표 인용.
///
/// 사용자 경로에는 공백·한글·괄호가 흔히 들어간다. WSL 로 넘기는 모든 인자는
/// 예외 없이 이 함수를 거친다.
pub fn sh_quote(s: &str) -> String {
    format!("'{}'", s.replace('\'', r"'\''"))
}

/// 파일명/디렉터리명으로 안전한 slug 로 바꾼다 (스키마 ID 생성용).
pub fn slugify(s: &str) -> String {
    let mut out = String::new();
    for ch in s.chars() {
        if ch.is_ascii_alphanumeric() || ch == '-' || ch == '_' {
            out.push(ch);
        } else if ch.is_whitespace() || ch == '.' {
            out.push('_');
        } else if !ch.is_ascii() {
            // 한글 스키마 이름을 허용하되 파일명에는 넣지 않는다.
            out.push('x');
        }
    }
    let trimmed = out.trim_matches('_').to_string();
    if trimmed.is_empty() {
        "schema".to_string()
    } else {
        trimmed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quotes_paths_with_spaces_and_quotes() {
        assert_eq!(sh_quote("/home/u/my data"), "'/home/u/my data'");
        assert_eq!(sh_quote("it's"), r"'it'\''s'");
    }

    #[test]
    fn slugify_falls_back_when_empty() {
        assert_eq!(slugify("   "), "schema");
        assert_eq!(slugify("Listeria 2026"), "Listeria_2026");
    }
}
