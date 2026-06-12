//! Modified UTF-8 encoding and decoding utilities.

/// Error that occurs when decoding Modified UTF-8 fails.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Error {
    /// Input ended unexpectedly; at position `pos`, at least `expected` more bytes are needed.
    UnexpectedEof { pos: usize, expected: usize },
    /// Invalid start byte `byte` at position `pos`.
    InvalidStartByte { pos: usize, byte: u8 },
    /// Invalid continuation byte `byte` at position `pos` for a sequence that started at `start_pos`.
    InvalidContinuation {
        start_pos: usize,
        pos: usize,
        byte: u8,
    },
    /// Overlong encoding of length `len` bytes starting at `start_pos`.
    OverlongEncoding { start_pos: usize, len: usize },
    /// Unpaired surrogate code point; sequence starts at `start_pos` and is `len` bytes long.
    LoneSurrogate { start_pos: usize, len: usize },
}

impl std::fmt::Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::UnexpectedEof { pos, expected } => {
                write!(
                    f,
                    "unexpected end of file at byte index {}, expected at least {} more bytes",
                    pos, expected
                )
            }
            Self::InvalidStartByte { pos, byte } => {
                write!(
                    f,
                    "invalid modified utf-8 start byte 0x{:02X} at byte index {}",
                    byte, pos
                )
            }
            Self::InvalidContinuation {
                start_pos,
                pos,
                byte,
            } => {
                write!(
                    f,
                    "invalid continuation byte 0x{:02X} at byte index {}, sequence started at {}",
                    byte, pos, start_pos
                )
            }
            Self::OverlongEncoding { start_pos, len } => {
                write!(
                    f,
                    "overlong encoding of {} bytes at byte index {}",
                    len, start_pos
                )
            }
            Self::LoneSurrogate { start_pos, len } => {
                write!(
                    f,
                    "lone surrogate of {} bytes at byte index {} is not allowed",
                    len, start_pos
                )
            }
        }
    }
}

impl std::error::Error for Error {}

impl Error {
    /// Returns the number of valid bytes before the first invalid byte.
    pub fn valid_up_to(&self) -> usize {
        match self {
            Self::UnexpectedEof { pos, .. } => *pos,
            Self::InvalidStartByte { pos, .. } => *pos,
            Self::InvalidContinuation { start_pos, .. } => *start_pos,
            Self::OverlongEncoding { start_pos, .. } => *start_pos,
            Self::LoneSurrogate { start_pos, .. } => *start_pos,
        }
    }

    /// Returns the length of the byte sequence that caused the error, if known.
    pub fn error_len(&self) -> Option<usize> {
        match self {
            Self::UnexpectedEof { .. } => None,
            Self::InvalidStartByte { .. } => Some(1),
            Self::InvalidContinuation { .. } => Some(1),
            Self::OverlongEncoding { len, .. } => Some(*len),
            Self::LoneSurrogate { len, .. } => Some(*len),
        }
    }
}

fn encoded_len(s: &str) -> usize {
    let mut len = 0;
    for ch in s.chars() {
        len += match ch {
            '\0' => 2,
            _ if (ch as u32) <= 0x007F => 1,
            _ if (ch as u32) <= 0x07FF => 2,
            _ if (ch as u32) <= 0xFFFF => 3,
            _ => 6,
        }
    }
    len
}

/// Encodes a string into Modified UTF-8 bytes.
pub fn encode(s: &str) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(encoded_len(s));

    for ch in s.chars() {
        let code = ch as u32;
        if ch == '\0' {
            bytes.extend_from_slice(&[0b1100_0000, 0b1000_0000]);
        } else if code <= 0x007F {
            bytes.push(ch as u8);
        } else if code <= 0x07FF {
            bytes.extend_from_slice(&[
                (0b1100_0000 | (code >> 6)) as u8,
                (0b1000_0000 | (code & 0b0011_1111)) as u8,
            ]);
        } else if code <= 0xFFFF {
            bytes.extend_from_slice(&[
                (0b1110_0000 | (code >> 12)) as u8,
                (0b1000_0000 | ((code >> 6) & 0b0011_1111)) as u8,
                (0b1000_0000 | (code & 0b0011_1111)) as u8,
            ]);
        } else {
            let offset = code - 0x10000;
            let high = 0xD800 | (offset >> 10);
            let low = 0xDC00 | (offset & 0x3FF);

            bytes.extend_from_slice(&[
                (0b1110_0000 | (high >> 12)) as u8,
                (0b1000_0000 | ((high >> 6) & 0b0011_1111)) as u8,
                (0b1000_0000 | (high & 0b0011_1111)) as u8,
                (0b1110_0000 | (low >> 12)) as u8,
                (0b1000_0000 | ((low >> 6) & 0b0011_1111)) as u8,
                (0b1000_0000 | (low & 0b0011_1111)) as u8,
            ]);
        }
    }

    bytes
}

fn is_continuation_byte(b: u8) -> bool {
    (b & 0b1100_0000) == 0b1000_0000
}

/// Decodes Modified UTF-8 bytes back into a string.
pub fn decode(bytes: &[u8]) -> Result<String, Error> {
    let mut result = String::new();
    let mut i = 0;
    let len = bytes.len();

    while i < len {
        let b0 = bytes[i];
        if b0 == 0x00 {
            return Err(Error::InvalidStartByte { pos: i, byte: b0 });
        } else if (b0 & 0b1000_0000) == 0b0000_0000 {
            result.push(b0 as char);
            i += 1;
            continue;
        } else if (b0 & 0b1110_0000) == 0b1100_0000 {
            if i + 1 >= len {
                return Err(Error::UnexpectedEof {
                    pos: i,
                    expected: 1,
                });
            }
            let b1 = bytes[i + 1];
            if !is_continuation_byte(b1) {
                return Err(Error::InvalidContinuation {
                    start_pos: i,
                    pos: i + 1,
                    byte: b1,
                });
            }

            let code = ((b0 & 0b0001_1111) as u32) << 6 | (b1 & 0b0011_1111) as u32;

            if code != 0x0000 && code <= 0x007F {
                return Err(Error::OverlongEncoding {
                    start_pos: i,
                    len: 2,
                });
            }
            result.push(char::from_u32(code).unwrap());
            i += 2;
            continue;
        } else if (b0 & 0b1111_0000) == 0b1110_0000 {
            if i + 1 >= len {
                return Err(Error::UnexpectedEof {
                    pos: i,
                    expected: 2,
                });
            }
            let b1 = bytes[i + 1];
            if !is_continuation_byte(b1) {
                return Err(Error::InvalidContinuation {
                    start_pos: i,
                    pos: i + 1,
                    byte: b1,
                });
            }

            if i + 2 >= len {
                return Err(Error::UnexpectedEof {
                    pos: i,
                    expected: 1,
                });
            }
            let b2 = bytes[i + 2];
            if !is_continuation_byte(b2) {
                return Err(Error::InvalidContinuation {
                    start_pos: i,
                    pos: i + 2,
                    byte: b2,
                });
            }

            let code = ((b0 & 0b0000_1111) as u32) << 12
                | ((b1 & 0b0011_1111) as u32) << 6
                | (b2 & 0b0011_1111) as u32;

            if code <= 0x07FF {
                return Err(Error::OverlongEncoding {
                    start_pos: i,
                    len: 3,
                });
            }

            if (0xD800..=0xDBFF).contains(&code) {
                if i + 3 >= len {
                    return Err(Error::LoneSurrogate {
                        start_pos: i,
                        len: 3,
                    });
                }
                let b3 = bytes[i + 3];
                if (b3 & 0b1111_0000) != 0b1110_0000 {
                    return Err(Error::InvalidStartByte {
                        pos: i + 3,
                        byte: b3,
                    });
                }

                if i + 4 >= len {
                    return Err(Error::UnexpectedEof {
                        pos: i + 3,
                        expected: 2,
                    });
                }
                let b4 = bytes[i + 4];
                if !is_continuation_byte(b4) {
                    return Err(Error::InvalidContinuation {
                        start_pos: i + 3,
                        pos: i + 4,
                        byte: b4,
                    });
                }

                if i + 5 >= len {
                    return Err(Error::UnexpectedEof {
                        pos: i + 3,
                        expected: 1,
                    });
                }
                let b5 = bytes[i + 5];
                if !is_continuation_byte(b5) {
                    return Err(Error::InvalidContinuation {
                        start_pos: i + 3,
                        pos: i + 5,
                        byte: b5,
                    });
                }

                let high = code;
                let low = ((b3 & 0b0000_1111) as u32) << 12
                    | ((b4 & 0b0011_1111) as u32) << 6
                    | (b5 & 0b0011_1111) as u32;

                if !(0xDC00..=0xDFFF).contains(&low) {
                    return Err(Error::LoneSurrogate {
                        start_pos: i,
                        len: 3,
                    });
                }

                let code = ((high as u32 - 0xD800) << 10) + (low as u32 - 0xDC00) + 0x10000;

                if code <= 0xFFFF {
                    return Err(Error::OverlongEncoding {
                        start_pos: i,
                        len: 6,
                    });
                }

                result.push(char::from_u32(code).unwrap());
                i += 6;
                continue;
            } else if (0xDC00..=0xDFFF).contains(&code) {
                return Err(Error::LoneSurrogate {
                    start_pos: i,
                    len: 3,
                });
            } else {
                result.push(char::from_u32(code).unwrap());
                i += 3;
                continue;
            }
        } else {
            return Err(Error::InvalidStartByte { pos: i, byte: b0 });
        }
    }

    Ok(result)
}

/// Decodes Modified UTF-8 bytes back into a string, replacing invalid sequences with the Unicode replacement character (U+FFFD).
pub fn decode_lossy(bytes: &[u8]) -> String {
    let mut result = String::new();
    let mut i = 0;
    let len = bytes.len();

    while i < len {
        let b0 = bytes[i];
        if b0 == 0x00 {
            result.push('\u{FFFD}');
            i += 1;
            continue;
        } else if (b0 & 0b1000_0000) == 0b0000_0000 {
            result.push(b0 as char);
            i += 1;
            continue;
        } else if (b0 & 0b1110_0000) == 0b1100_0000 {
            if i + 1 >= len {
                result.push('\u{FFFD}');
                i += 1;
                continue;
            }
            let b1 = bytes[i + 1];
            if !is_continuation_byte(b1) {
                result.push('\u{FFFD}');
                i += 1;
                continue;
            }

            let code = ((b0 & 0b0001_1111) as u32) << 6 | (b1 & 0b0011_1111) as u32;

            if code != 0x0000 && code <= 0x007F {
                result.push('\u{FFFD}');
                i += 2;
                continue;
            }
            result.push(char::from_u32(code).unwrap());
            i += 2;
            continue;
        } else if (b0 & 0b1111_0000) == 0b1110_0000 {
            if i + 1 >= len {
                result.push('\u{FFFD}');
                i += 1;
                continue;
            }
            let b1 = bytes[i + 1];
            if !is_continuation_byte(b1) {
                result.push('\u{FFFD}');
                i += 1;
                continue;
            }

            if i + 2 >= len {
                result.push('\u{FFFD}');
                i += 2;
                continue;
            }
            let b2 = bytes[i + 2];
            if !is_continuation_byte(b2) {
                result.push('\u{FFFD}');
                i += 2;
                continue;
            }

            let code = ((b0 & 0b0000_1111) as u32) << 12
                | ((b1 & 0b0011_1111) as u32) << 6
                | (b2 & 0b0011_1111) as u32;

            if code <= 0x07FF {
                result.push('\u{FFFD}');
                i += 3;
                continue;
            }

            if (0xD800..=0xDBFF).contains(&code) {
                if i + 3 >= len {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }
                let b3 = bytes[i + 3];
                if (b3 & 0b1111_0000) != 0b1110_0000 {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }

                if i + 4 >= len {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }
                let b4 = bytes[i + 4];
                if !is_continuation_byte(b4) {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }

                if i + 5 >= len {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }
                let b5 = bytes[i + 5];
                if !is_continuation_byte(b5) {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }

                let high = code;
                let low = ((b3 & 0b0000_1111) as u32) << 12
                    | ((b4 & 0b0011_1111) as u32) << 6
                    | (b5 & 0b0011_1111) as u32;

                if !(0xDC00..=0xDFFF).contains(&low) {
                    result.push('\u{FFFD}');
                    i += 3;
                    continue;
                }

                let code = ((high as u32 - 0xD800) << 10) + (low as u32 - 0xDC00) + 0x10000;

                if code <= 0xFFFF {
                    result.push_str("\u{FFFD}\u{FFFD}");
                    i += 6;
                    continue;
                }

                result.push(char::from_u32(code).unwrap());
                i += 6;
                continue;
            } else if (0xDC00..=0xDFFF).contains(&code) {
                result.push('\u{FFFD}');
                i += 3;
                continue;
            } else {
                result.push(char::from_u32(code).unwrap());
                i += 3;
                continue;
            }
        } else {
            result.push('\u{FFFD}');
            i += 1;
            continue;
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn roundtrip_empty() {
        let s = "";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn roundtrip_ascii() {
        let s = "Hello, world!";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn roundtrip_bmp() {
        let s = "世界你好！";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn roundtrip_smp() {
        let s = "𝕙𝕖𝕝𝕝𝕠 𝕨𝕠𝕣𝕝𝕕";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn roundtrip_sip() {
        let s = "\u{20000}";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn roundtrip_ssp() {
        let s = "\u{E0001}";
        let encoded = encode(s);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn encode_null() {
        let s = "\0";
        let encoded = encode(s);
        assert_eq!(encoded, &[0b1100_0000, 0b1000_0000]);
        let decoded = decode(&encoded).unwrap();
        assert_eq!(decoded, s);
    }
    
    #[test]
    fn decode_null() {
        let bytes = &[0b1100_0000, 0b1000_0000];
        let decoded = decode(bytes).unwrap();
        assert_eq!(decoded, "\0");
    }
    
    #[test]
    fn encode_smp() {
        let s = "𝕣"; // U+1D563
        let encoded = encode(s);
        assert_eq!(encoded, &[
            0b1110_1101,
            0b1010_0000,
            0b1011_0101,
            0b1110_1101,
            0b1011_0101,
            0b1010_0011,
        ]);
    }
    
    #[test]
    fn decode_smp() {
        let bytes = &[
            0b1110_1101,
            0b1010_0000,
            0b1011_0101,
            0b1110_1101,
            0b1011_0101,
            0b1010_0011,
        ];
        let decoded = decode(bytes).unwrap();
        assert_eq!(decoded, "𝕣");
    }
    
    #[test]
    fn decode_empty() {
        assert_eq!(decode(&[]), Ok(String::new()));
    }
    
    #[test]
    fn decode_null_byte() {
        let bytes = &[0x00];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::InvalidStartByte { pos: 0, byte: 0x00 })));
    }
    
    #[test]
    fn decode_unexpected_eof() {
        let bytes = &[0b1110_0000];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::UnexpectedEof { pos: 0, expected: 2 })));
    }
    
    #[test]
    fn decode_invalid_start_byte() {
        let bytes = &[0b1111_1111];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::InvalidStartByte { pos: 0, byte: 0b1111_1111 })));
    }
    
    #[test]
    fn decode_invalid_continuation() {
        let bytes = &[0b1100_0001, 0b1111_0010];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::InvalidContinuation { start_pos: 0, pos: 1, byte: 0b1111_0010 })));
    }
    
    #[test]
    fn decode_overlong_encoding() {
        let bytes = &[0b1100_0001, 0b1011_0010];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::OverlongEncoding { start_pos: 0, len: 2 })));
    }
    
    #[test]
    fn decode_lone_surrogate() {
        let bytes = &[0b1110_1101, 0b1010_0000, 0b1000_0000];
        let decoded = decode(bytes);
        assert!(matches!(decoded, Err(Error::LoneSurrogate { start_pos: 0, len: 3 })));
    }
    
    #[test]
    fn decode_lossy_empty() {
        assert_eq!(decode_lossy(&[]), "");
    }
    
    #[test]
    fn decode_lossy_null_byte() {
        let bytes = &[0x00];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}");
    }
    
    #[test]
    fn decode_lossy_unexpected_eof() {
        let bytes = &[0b1110_0000];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}");
    }
    
    #[test]
    fn decode_lossy_invalid_start_byte() {
        let bytes = &[0b1111_1111];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}");
    }
    
    #[test]
    fn decode_lossy_invalid_continuation() {
        let bytes = &[0b1100_0001, 0b1111_0010];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}\u{FFFD}");
    }
    
    #[test]
    fn decode_lossy_overlong_encoding() {
        let bytes = &[0b1100_0001, 0b1011_0010];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}");
    }
    
    #[test]
    fn decode_lossy_lone_surrogate() {
        let bytes = &[0b1110_1101, 0b1010_0000, 0b1000_0000];
        assert_eq!(decode_lossy(bytes), "\u{FFFD}");
    }
}