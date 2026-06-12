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