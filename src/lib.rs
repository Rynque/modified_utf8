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