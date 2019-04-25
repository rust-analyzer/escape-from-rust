//! Utilities for validating  string and char literals and turning them into
//! values they represent.

use std::str::Chars;
use std::ops::Range;

#[derive(Debug, PartialEq, Eq)]
pub(crate) enum EscapeError {
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

/// Takes a contents of a char literal (without quotes), and returns an
/// unescaped char or an error
pub(crate) fn unescape_char(literal_text: &str) -> Result<char, EscapeError> {
    let mut chars = literal_text.chars();
    let first_char = chars.next().ok_or(EscapeError::ZeroChars)?;
    let res = scan_escape(first_char, &mut chars, Mode::Char)?;
    if chars.next().is_some() {
        return Err(EscapeError::MoreThanOneChar);
    }
    Ok(res)
}

/// Takes a contents of a string literal (without quotes) and produces a
/// sequence of escaped characters or errors.
pub(crate) fn unescape_str<F>(src: &str, callback: &mut F)
where
    F: FnMut(Range<usize>, Result<char, EscapeError>),
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
                    _ => scan_escape(first_char, &mut chars, Mode::Str),
                }
            }
            '\n' => Ok('\n'),
            '\r' => {
                let second_char = chars.clone().next();
                if second_char == Some('\n') {
                    chars.next();
                    Ok('\n')
                } else {
                    scan_escape(first_char, &mut chars, Mode::Str)
                }
            }
            _ => scan_escape(first_char, &mut chars, Mode::Str),
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

#[derive(Clone, Copy)]
enum Mode {
    Char,
    Str
}

impl Mode {
    fn is_char(self) -> bool {
        match self {
            Mode::Char => true,
            Mode::Str => false,
        }
    }

    fn is_str(self) -> bool {
        match self {
            Mode::Char => false,
            Mode::Str => true,
        }
    }
}

fn scan_escape(
    first_char: char,
    chars: &mut Chars<'_>,
    mode: Mode,
) -> Result<char, EscapeError> {
    if first_char != '\\' {
        return match first_char {
            '\t' | '\n' => Err(EscapeError::EscapeOnlyChar),
            '\r' => Err(if chars.clone().next() == Some('\n') {
                EscapeError::EscapeOnlyChar
            } else {
                EscapeError::BareCarriageReturn
            }),
            '\'' if mode.is_char() => Err(EscapeError::EscapeOnlyChar),
            '"' if mode.is_str() => Err(EscapeError::EscapeOnlyChar),
            _ => Ok(first_char),
        };
    }

    let second_char = chars.next().ok_or(EscapeError::LoneSlash)?;

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
                .ok_or(EscapeError::InvalidHexEscape)?;
            let lo = chars
                .next()
                .and_then(|c| c.to_digit(16))
                .ok_or(EscapeError::InvalidHexEscape)?;
            let value = hi.checked_mul(16).unwrap().checked_add(lo).unwrap();

            if value > 0x7f {
                return Err(EscapeError::OutOfRangeHexEscape);
            }
            let value = value as u8;

            value as char
        }

        'u' => {
            if chars.next() != Some('{') {
                return Err(EscapeError::InvalidUnicodeEscape);
            }

            let mut n_digits = 1;
            let mut value: u32 =
                match chars.next().ok_or(EscapeError::UnclosedUnicodeEscape)? {
                    '_' => return Err(EscapeError::LeadingUnderscoreUnicodeEscape),
                    '}' => return Err(EscapeError::EmptyUnicodeEscape),
                    c => c.to_digit(16).ok_or(EscapeError::InvalidUnicodeEscape)?,
                };

            loop {
                match chars.next() {
                    None => return Err(EscapeError::UnclosedUnicodeEscape),
                    Some('_') => continue,
                    Some('}') => {
                        break std::char::from_u32(value).ok_or_else(|| {
                            if value > 0x10FFFF {
                                EscapeError::OutOfRangeUnicodeEscape
                            } else {
                                EscapeError::LoneSurrogateUnicodeEscape
                            }
                        })?;
                    }
                    Some(c) => {
                        let digit =
                            c.to_digit(16).ok_or(EscapeError::InvalidUnicodeEscape)?;
                        n_digits += 1;
                        if n_digits > 6 {
                            return Err(EscapeError::OverlongUnicodeEscape);
                        }

                        let digit = digit as u32;
                        value = value.checked_mul(16).unwrap().checked_add(digit).unwrap();
                    }
                };
            }
        }
        _ => return Err(EscapeError::InvalidEscape),
    };
    Ok(res)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_unescape_char_bad() {
        fn check(literal_text: &str, expected_error: EscapeError) {
            let actual_result = unescape_char(literal_text);
            assert_eq!(actual_result, Err(expected_error));
        }

        check("", EscapeError::ZeroChars);
        check(r"\", EscapeError::LoneSlash);

        check("\n", EscapeError::EscapeOnlyChar);
        check("\r\n", EscapeError::EscapeOnlyChar);
        check("\t", EscapeError::EscapeOnlyChar);
        check("'", EscapeError::EscapeOnlyChar);
        check("\r", EscapeError::BareCarriageReturn);

        check("spam", EscapeError::MoreThanOneChar);
        check(r"\x0ff", EscapeError::MoreThanOneChar);
        check(r#"\"a"#, EscapeError::MoreThanOneChar);
        check(r"\na", EscapeError::MoreThanOneChar);
        check(r"\ra", EscapeError::MoreThanOneChar);
        check(r"\ta", EscapeError::MoreThanOneChar);
        check(r"\\a", EscapeError::MoreThanOneChar);
        check(r"\'a", EscapeError::MoreThanOneChar);
        check(r"\0a", EscapeError::MoreThanOneChar);
        check(r"\u{0}x", EscapeError::MoreThanOneChar);
        check(r"\u{1F63b}}", EscapeError::MoreThanOneChar);

        check(r"\v", EscapeError::InvalidEscape);
        check(r"\💩", EscapeError::InvalidEscape);
        check(r"\●",  EscapeError::InvalidEscape);

        check(r"\x", EscapeError::InvalidHexEscape);
        check(r"\x0", EscapeError::InvalidHexEscape);
        check(r"\xa", EscapeError::InvalidHexEscape);
        check(r"\xf", EscapeError::InvalidHexEscape);
        check(r"\xx", EscapeError::InvalidHexEscape);
        check(r"\xы", EscapeError::InvalidHexEscape);
        check(r"\x🦀", EscapeError::InvalidHexEscape);
        check(r"\xtt", EscapeError::InvalidHexEscape);
        check(r"\xff", EscapeError::OutOfRangeHexEscape);
        check(r"\xFF", EscapeError::OutOfRangeHexEscape);
        check(r"\x80", EscapeError::OutOfRangeHexEscape);

        check(r"\u", EscapeError::InvalidUnicodeEscape);
        check(r"\u[0123]", EscapeError::InvalidUnicodeEscape);
        check(r"\u{", EscapeError::UnclosedUnicodeEscape);
        check(r"\u{0000", EscapeError::UnclosedUnicodeEscape);
        check(r"\u{}", EscapeError::EmptyUnicodeEscape);
        check(r"\u{_0000}", EscapeError::LeadingUnderscoreUnicodeEscape);
        check(r"\u{0000000}", EscapeError::OverlongUnicodeEscape);
        check(r"\u{FFFFFF}", EscapeError::OutOfRangeUnicodeEscape);
        check(r"\u{ffffff}", EscapeError::OutOfRangeUnicodeEscape);
        check(r"\u{ffffff}", EscapeError::OutOfRangeUnicodeEscape);

        check(r"\u{DC00}", EscapeError::LoneSurrogateUnicodeEscape);
        check(r"\u{DDDD}", EscapeError::LoneSurrogateUnicodeEscape);
        check(r"\u{DFFF}", EscapeError::LoneSurrogateUnicodeEscape);

        check(r"\u{D800}", EscapeError::LoneSurrogateUnicodeEscape);
        check(r"\u{DAAA}", EscapeError::LoneSurrogateUnicodeEscape);
        check(r"\u{DBFF}", EscapeError::LoneSurrogateUnicodeEscape);
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
