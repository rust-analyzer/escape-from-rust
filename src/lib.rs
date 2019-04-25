#![allow(unused)]
//! Utilities for turning string and char literals into values they represent.

use std::str::Chars;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub enum UnescapeCharError {
    ZeroChars,
    MoreThanOneChar,

    LoneSlash,
    InvalidEscape,
    BareCarriageReturn,
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
    let res = scan_char_escape(first_char, &mut chars, '\'')?;
    if chars.next().is_some() {
        return Err(UnescapeCharError::MoreThanOneChar);
    }
    Ok(res)
}

pub fn unescape_str<F>(src: &str, callback: &mut F)
where
    F: FnMut(Range<usize>, Result<char, UnescapeCharError>),
{
    let initial_len = src.len();
    let mut chars = src.chars();
    while let Some(first_char) = chars.next() {
        let start = initial_len - chars.as_str().len() - first_char.len_utf8();
        let escaped_char = match first_char {
            '\\' => {
                let (second_char, third_char) = {
                    let mut chars = chars.clone();
                    (chars.next(), chars.next())
                };
                match (second_char, third_char) {
                    (Some('\n'), _) | (Some('\r'), Some('\n')) => {
                        skip_ascii_whitespace(&mut chars);
                        continue;
                    }
                    _ => scan_char_escape(first_char, &mut chars, '"'),
                }
            }
            '\n' => Ok('\n'),
            '\r' => {
                let second_char = chars.clone().next();
                if second_char == Some('\n') {
                    chars.next();
                    Ok('\n')
                } else {
                    scan_char_escape(first_char, &mut chars, '"')
                }
            }
            _ => scan_char_escape(first_char, &mut chars, '"'),
        };
        let end = initial_len - chars.as_str().len();
        callback(start..end, escaped_char);
    }

    fn skip_ascii_whitespace(chars: &mut Chars<'_>) {
        let str = chars.as_str();
        let first_non_space = str
            .bytes()
            .position(|b| b != b' ' && b != b'\t' && b != b'\n' && b != b'\r')
            .unwrap_or(str.len());
        *chars = str[first_non_space..].chars()
    }
}

fn scan_char_escape(
    first_char: char,
    chars: &mut Chars<'_>,
    quote: char,
) -> Result<char, UnescapeCharError> {
    if first_char != '\\' {
        return match first_char {
            '\t' | '\n' => Err(UnescapeCharError::EscapeOnlyChar),
            '\r' => Err(if chars.clone().next() == Some('\n') {
                UnescapeCharError::EscapeOnlyChar
            } else {
                UnescapeCharError::BareCarriageReturn
            }),
            '\'' if quote == '\'' => Err(UnescapeCharError::EscapeOnlyChar),
            '"' if quote == '"' => Err(UnescapeCharError::EscapeOnlyChar),
            _ => Ok(first_char),
        };
    }

    let second_char = chars.next().ok_or(UnescapeCharError::LoneSlash)?;

    let res = match second_char {
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

            value as char
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
                        break std::char::from_u32(value).ok_or_else(|| {
                            if value > 0x10FFFF {
                                UnescapeCharError::OutOfRangeUnicodeEscape
                            } else {
                                UnescapeCharError::LoneSurrogateUnicodeEscape
                            }
                        })?;
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
    Ok(res)
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
        check("\t", UnescapeCharError::EscapeOnlyChar);
        check("'", UnescapeCharError::EscapeOnlyChar);
        check("\r", UnescapeCharError::BareCarriageReturn);

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

    #[test]
    fn test_unescape_str_good() {
        fn check(literal_text: &str, expected: &str) {
            let mut buf = Ok(String::with_capacity(literal_text.len()));
            unescape_str(literal_text, &mut |range, c| {
                if let Ok(b) = &mut buf {
                    match c {
                        Ok(c) => b.push(c),
                        Err(e) => buf = Err((range, e)),
                    }
                }
            });
            let buf = buf.as_ref().map(|it| it.as_ref());
            assert_eq!(buf, Ok(expected))
        }

        check("foo", "foo");
        check("", "");
        check(" \n\r\n", " \n\n");

        check("hello \\\n     world", "hello world");
        check("hello \\\r\n     world", "hello world");
        check("thread's", "thread's")
    }
}
