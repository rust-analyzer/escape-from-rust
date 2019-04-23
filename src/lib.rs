//! Utilities for turning string and char literals into values they represent.

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
    let literal_bytes = literal_text.as_bytes();
    let &first_byte = literal_bytes.get(0).ok_or(UnescapeCharError::ZeroChars)?;

    if first_byte != b'\\' {
        return match first_byte {
            b'\t' | b'\n' | b'\r' | b'\'' => Err(UnescapeCharError::EscapeOnlyChar),
            _ => {
                let mut chars = literal_text.chars();
                let res = chars.next().unwrap();
                if chars.next().is_some() {
                    return Err(UnescapeCharError::MoreThanOneChar);
                }
                Ok(res)
            }
        };
    }

    let &second_byte = literal_bytes.get(1).ok_or(UnescapeCharError::LoneSlash)?;

    let simple_escape = match second_byte {
        b'"' => '"',
        b'n' => '\n',
        b'r' => '\r',
        b't' => '\t',
        b'\\' => '\\',
        b'\'' => '\'',
        b'0' => '\0',

        b'x' => {
            let hi = literal_bytes
                .get(2)
                .and_then(|&byte| to_hex_digit(byte))
                .ok_or(UnescapeCharError::InvalidHexEscape)?;
            let lo = literal_bytes
                .get(3)
                .and_then(|&byte| to_hex_digit(byte))
                .ok_or(UnescapeCharError::InvalidHexEscape)?;
            let value = hi.checked_mul(16).unwrap().checked_add(lo).unwrap();
            if value > 0x7f {
                return Err(UnescapeCharError::OutOfRangeHexEscape);
            }
            if literal_bytes.len() > 4 {
                return Err(UnescapeCharError::MoreThanOneChar);
            }
            return Ok(value as char);
        }

        b'u' => {
            if literal_bytes.get(2) != Some(&b'{') {
                return Err(UnescapeCharError::InvalidUnicodeEscape);
            }

            match literal_bytes.get(3).ok_or(UnescapeCharError::UnclosedUnicodeEscape)? {
                b'_' => return Err(UnescapeCharError::LeadingUnderscoreUnicodeEscape),
                b'}' => return Err(UnescapeCharError::EmptyUnicodeEscape),
                _ => (),
            }

            let mut value: u32 = 0;
            let mut no_closing_brace = Err(UnescapeCharError::UnclosedUnicodeEscape);
            let mut n_digits = 0;
            for (i, &byte) in literal_bytes[3..].iter().enumerate() {
                match byte {
                    b'_' => continue,
                    b'}' => {
                        if i != literal_bytes[3..].len() - 1 {
                            return Err(UnescapeCharError::MoreThanOneChar);
                        }
                        no_closing_brace = Ok(());
                        break;
                    }
                    _ => {
                        let digit =
                            to_hex_digit(byte).ok_or(UnescapeCharError::InvalidUnicodeEscape)?;
                        n_digits += 1;
                        if n_digits > 6 {
                            return Err(UnescapeCharError::OverlongUnicodeEscape);
                        }

                        let digit = digit as u32;
                        value = value.checked_mul(16).unwrap().checked_add(digit).unwrap();
                    }
                }
            }
            no_closing_brace?;

            return std::char::from_u32(value).ok_or_else(|| {
                if value > 0x10FFFF {
                    UnescapeCharError::OutOfRangeUnicodeEscape
                } else {
                    UnescapeCharError::LoneSurrogateUnicodeEscape
                }
            });
        }
        _ => return Err(UnescapeCharError::InvalidEscape),
    };

    if literal_bytes.len() > 2 {
        return Err(UnescapeCharError::MoreThanOneChar);
    }
    Ok(simple_escape)
}

pub struct UnescapeStrErrorInfo {
    _src_pos: usize,
    _error: UnescapeCharError,
}

pub fn unescape_str<F>(_src: &str, _buf: &mut String, _on_error: &mut F)
where
    F: FnMut(&mut String, UnescapeStrErrorInfo),
{

}

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
