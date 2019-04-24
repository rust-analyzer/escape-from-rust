//! Utilities for turning string and char literals into values they represent.

use std::str::Chars;

#[derive(Debug, PartialEq, Eq)]
pub enum UnescapeCharError {
    ZeroChars,
    MoreThanOneChar,

    LoneSlash,
    InvalidEscape,
    EscapeOnlyChar,

    InvalidHexEscape,
    OutOfRangeHexEscape,

    InvalidUnicodeEscape,
    EmptyUnicodeEscape,
    UnclosedUnicodeEscape,
    LeadingUnderscoreUnicodeEscape,
    OverlongUnicodeEscape,
    LoneSurrogateUnicodeEscape,
    OutOfRangeUnicodeEscape,
}

pub fn unescape_char(literal_text: &str) -> Result<char, UnescapeCharError> {
    let mut chars = literal_text.chars();
    let first_char = chars.next().ok_or(UnescapeCharError::ZeroChars)?;

    if first_char != '\\' {
        return match first_char {
            '\t' | '\n' | '\r' | '\'' => Err(UnescapeCharError::EscapeOnlyChar),
            _ => {
                if chars.next().is_some() {
                    return Err(UnescapeCharError::MoreThanOneChar);
                }
                Ok(first_char)
            }
        };
    }

    let second_char = chars.next().ok_or(UnescapeCharError::LoneSlash)?;

    let simple_escape = match second_char {
        '"' => '"',
        'n' => '\n',
        'r' => '\r',
        't' => '\t',
        '\\' => '\\',
        '\'' => '\'',
        '0' => '\0',

        'x' => {
            let hi = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or(UnescapeCharError::InvalidHexEscape)?;
            let lo = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or(UnescapeCharError::InvalidHexEscape)?;
            let value = hi.checked_mul(16).unwrap().checked_add(lo).unwrap();

            if value > 0x7f {
                return Err(UnescapeCharError::OutOfRangeHexEscape);
            }
            let value = value as u8;

            if chars.next().is_some() {
                return Err(UnescapeCharError::MoreThanOneChar);
            }
            return Ok(value as char);
        }

        'u' => {
            if chars.next() != Some('{') {
                return Err(UnescapeCharError::InvalidUnicodeEscape);
            }

            let mut n_digits = 1;
            let mut value: u32 =
                match chars.next().ok_or(UnescapeCharError::UnclosedUnicodeEscape)? {
                    '_' => return Err(UnescapeCharError::LeadingUnderscoreUnicodeEscape),
                    '}' => return Err(UnescapeCharError::EmptyUnicodeEscape),
                    c => c.to_digit(16).ok_or(UnescapeCharError::InvalidUnicodeEscape)?,
                };

            loop {
                match chars.next() {
                    None => return Err(UnescapeCharError::UnclosedUnicodeEscape),
                    Some('_') => continue,
                    Some('}') => {
                        if chars.next().is_some() {
                            return Err(UnescapeCharError::MoreThanOneChar);
                        }
                        return std::char::from_u32(value).ok_or_else(|| {
                            if value > 0x10FFFF {
                                UnescapeCharError::OutOfRangeUnicodeEscape
                            } else {
                                UnescapeCharError::LoneSurrogateUnicodeEscape
                            }
                        });
                    }
                    Some(c) => {
                        let digit =
                            c.to_digit(16).ok_or(UnescapeCharError::InvalidUnicodeEscape)?;
                        n_digits += 1;
                        if n_digits > 6 {
                            return Err(UnescapeCharError::OverlongUnicodeEscape);
                        }

                        let digit = digit as u32;
                        value = value.checked_mul(16).unwrap().checked_add(digit).unwrap();
                    }
                };
            }
        }
        _ => return Err(UnescapeCharError::InvalidEscape),
    };

    if chars.next().is_some() {
        return Err(UnescapeCharError::MoreThanOneChar);
    }
    Ok(simple_escape)
}

pub struct UnescapeStrErrorInfo {
    src_pos: usize,
    error: UnescapeCharError,
}

pub fn unescape_str<F>(src: &str, buf: &mut String, on_error: &mut F)
where
    F: FnMut(&mut String, UnescapeStrErrorInfo),
{
    let initial_len = src.len();
    let mut chars = src.chars();
    loop {
        if chars.as_str().starts_with("\\\n") {
            chars.next();
            chars.next();
            skip_ascii_whitespace(&mut chars);
            continue;
        }
        if chars.as_str().starts_with("\\\r\n") {
            chars.next();
            skip_ascii_whitespace(&mut chars);
            continue;
        }
        match scan_char_escape(&mut chars) {
            Ok(c) => buf.push(c),
            Err(error) => {
                let err_info =
                    UnescapeStrErrorInfo { src_pos: initial_len - chars.as_str().len(), error };
                on_error(buf, err_info)
            }
        }
    }

    fn skip_ascii_whitespace(chars: &mut Chars) {
        let str = chars.as_str();
        let first_non_space = str
            .bytes()
            .position(|b| b != b' ' && b != b'\t' && b != b'\n' && b != b'\r')
            .unwrap_or(str.len());
        *chars = str[first_non_space..].chars()
    }
}

fn scan_char_escape(chars: &mut Chars) -> Result<char, UnescapeCharError> {
    Ok('x')
}

/*
/// Parses a string representing a string literal into its final form. Does unescaping.
pub fn str_lit(lit: &str, diag: Option<(Span, &Handler)>) -> String {
    debug!("str_lit: given {}", lit.escape_default());
    let mut res = String::with_capacity(lit.len());

    let error = |i| format!("lexer should have rejected {} at {}", lit, i);

    /// Eat everything up to a non-whitespace.
    fn eat<'a>(it: &mut iter::Peekable<str::CharIndices<'a>>) {
        loop {
            match it.peek().map(|x| x.1) {
                Some(' ') | Some('\n') | Some('\r') | Some('\t') => {
                    it.next();
                },
                _ => { break; }
            }
        }
    }

    let mut chars = lit.char_indices().peekable();
    while let Some((i, c)) = chars.next() {
        match c {
            '\\' => {
                let ch = chars.peek().unwrap_or_else(|| {
                    panic!("{}", error(i))
                }).1;

                if ch == '\n' {
                    eat(&mut chars);
                } else if ch == '\r' {
                    chars.next();
                    let ch = chars.peek().unwrap_or_else(|| {
                        panic!("{}", error(i))
                    }).1;

                    if ch != '\n' {
                        panic!("lexer accepted bare CR");
                    }
                    eat(&mut chars);
                } else {
                    // otherwise, a normal escape
                    let (c, n) = char_lit(&lit[i..], diag);
                    for _ in 0..n - 1 { // we don't need to move past the first \
                        chars.next();
                    }
                    res.push(c);
                }
            },
            '\r' => {
                let ch = chars.peek().unwrap_or_else(|| {
                    panic!("{}", error(i))
                }).1;

                if ch != '\n' {
                    panic!("lexer accepted bare CR");
                }
                chars.next();
                res.push('\n');
            }
            c => res.push(c),
        }
    }

    res.shrink_to_fit(); // probably not going to do anything, unless there was an escape.
    debug!("parse_str_lit: returning {}", res);
    res
}

*/

pub enum UnescapeByteError {}

pub fn unescape_byte(_literal_text: &str) -> Result<u8, UnescapeByteError> {
    Ok(b'x')
}

pub struct UnescapeByteStrErrorInfo {
    _src_pos: usize,
    _error: UnescapeCharError,
}

pub fn unescape_byte_str<F>(_src: &str, _buf: &mut Vec<u8>, _on_error: &mut F)
where
    F: FnMut(&mut Vec<u8>, UnescapeByteStrErrorInfo),
{

}

fn to_hex_digit(byte: u8) -> Option<u8> {
    let res = match byte {
        b'0'..=b'9' => byte - b'0',
        b'a'..=b'f' => 10 + byte - b'a',
        b'A'..=b'F' => 10 + byte - b'A',
        _ => return None,
    };
    Some(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unescape_char_bad() {
        fn check(literal_text: &str, expected_error: UnescapeCharError) {
            let actual_result = unescape_char(literal_text);
            assert_eq!(actual_result, Err(expected_error));
        }

        check("", UnescapeCharError::ZeroChars);
        check(r"\", UnescapeCharError::LoneSlash);

        check("\n", UnescapeCharError::EscapeOnlyChar);
        check("\r\n", UnescapeCharError::EscapeOnlyChar);
        check("'", UnescapeCharError::EscapeOnlyChar);
        check("\t", UnescapeCharError::EscapeOnlyChar);

        check("spam", UnescapeCharError::MoreThanOneChar);
        check(r"\x0ff", UnescapeCharError::MoreThanOneChar);
        check(r#"\"a"#, UnescapeCharError::MoreThanOneChar);
        check(r"\na", UnescapeCharError::MoreThanOneChar);
        check(r"\ra", UnescapeCharError::MoreThanOneChar);
        check(r"\ta", UnescapeCharError::MoreThanOneChar);
        check(r"\\a", UnescapeCharError::MoreThanOneChar);
        check(r"\'a", UnescapeCharError::MoreThanOneChar);
        check(r"\0a", UnescapeCharError::MoreThanOneChar);
        check(r"\u{0}x", UnescapeCharError::MoreThanOneChar);
        check(r"\u{1F63b}}", UnescapeCharError::MoreThanOneChar);

        check(r"\v", UnescapeCharError::InvalidEscape);
        check(r"\💩", UnescapeCharError::InvalidEscape);

        check(r"\x", UnescapeCharError::InvalidHexEscape);
        check(r"\x0", UnescapeCharError::InvalidHexEscape);
        check(r"\xa", UnescapeCharError::InvalidHexEscape);
        check(r"\xf", UnescapeCharError::InvalidHexEscape);
        check(r"\xx", UnescapeCharError::InvalidHexEscape);
        check(r"\xы", UnescapeCharError::InvalidHexEscape);
        check(r"\x🦀", UnescapeCharError::InvalidHexEscape);
        check(r"\xtt", UnescapeCharError::InvalidHexEscape);
        check(r"\xff", UnescapeCharError::OutOfRangeHexEscape);
        check(r"\xFF", UnescapeCharError::OutOfRangeHexEscape);
        check(r"\x80", UnescapeCharError::OutOfRangeHexEscape);

        check(r"\u", UnescapeCharError::InvalidUnicodeEscape);
        check(r"\u[0123]", UnescapeCharError::InvalidUnicodeEscape);
        check(r"\u{", UnescapeCharError::UnclosedUnicodeEscape);
        check(r"\u{0000", UnescapeCharError::UnclosedUnicodeEscape);
        check(r"\u{}", UnescapeCharError::EmptyUnicodeEscape);
        check(r"\u{_0000}", UnescapeCharError::LeadingUnderscoreUnicodeEscape);
        check(r"\u{0000000}", UnescapeCharError::OverlongUnicodeEscape);
        check(r"\u{FFFFFF}", UnescapeCharError::OutOfRangeUnicodeEscape);
        check(r"\u{ffffff}", UnescapeCharError::OutOfRangeUnicodeEscape);
        check(r"\u{ffffff}", UnescapeCharError::OutOfRangeUnicodeEscape);

        check(r"\u{DC00}", UnescapeCharError::LoneSurrogateUnicodeEscape);
        check(r"\u{DDDD}", UnescapeCharError::LoneSurrogateUnicodeEscape);
        check(r"\u{DFFF}", UnescapeCharError::LoneSurrogateUnicodeEscape);

        check(r"\u{D800}", UnescapeCharError::LoneSurrogateUnicodeEscape);
        check(r"\u{DAAA}", UnescapeCharError::LoneSurrogateUnicodeEscape);
        check(r"\u{DBFF}", UnescapeCharError::LoneSurrogateUnicodeEscape);
    }

    #[test]
    fn test_unescape_char_good() {
        fn check(literal_text: &str, expected_char: char) {
            let actual_result = unescape_char(literal_text);
            assert_eq!(actual_result, Ok(expected_char));
        }

        check("a", 'a');
        check("ы", 'ы');
        check("🦀", '🦀');

        check(r#"\""#, '"');
        check(r"\n", '\n');
        check(r"\r", '\r');
        check(r"\t", '\t');
        check(r"\\", '\\');
        check(r"\'", '\'');
        check(r"\0", '\0');

        check(r"\x00", '\0');
        check(r"\x5a", 'Z');
        check(r"\x5A", 'Z');
        check(r"\x7f", 127 as char);

        check(r"\u{0}", '\0');
        check(r"\u{000000}", '\0');
        check(r"\u{41}", 'A');
        check(r"\u{0041}", 'A');
        check(r"\u{00_41}", 'A');
        check(r"\u{4__1__}", 'A');
        check(r"\u{1F63b}", '😻');
    }
}
