//! Double Metaphone, exact port of `src/double_metaphone.c` per
//! architecture.md section 5.3 (byte-oriented state machine, 5-space input
//! padding, 4-char code cap, primary/secondary wrapper semantics).

use std::fmt;

/// The `(primary, secondary)` code pair produced by Double Metaphone. Wrapper
/// semantics (architecture.md section 5.3): primary == secondary collapses the
/// secondary to `None`; an empty code becomes `None`.
pub type Codes = (Option<Vec<u8>>, Option<Vec<u8>>);

/// Result of the validating `&str` entry points (pinned signature from
/// architecture.md section 4, factored into a type alias for clarity).
pub type DmetaphoneResult = Result<Codes, NonAsciiError>;

/// Error returned by [`dmetaphone`] / [`dmetaphone_with_size`] when the input
/// contains a non-ASCII character (original Double Metaphone behavior is
/// preserved: the Python API raises on non-ASCII; the PyO3 layer maps this to
/// `UnicodeEncodeError`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NonAsciiError {
    ch: char,
    byte_position: usize,
}

impl NonAsciiError {
    /// Create the error for offending character `ch` found at byte offset
    /// `byte_position` of the input.
    pub fn new(ch: char, byte_position: usize) -> Self {
        Self { ch, byte_position }
    }

    /// The offending non-ASCII character.
    pub fn character(&self) -> char {
        self.ch
    }

    /// Byte offset of the offending character within the input string.
    pub fn byte_position(&self) -> usize {
        self.byte_position
    }
}

impl fmt::Display for NonAsciiError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "non-ASCII character {:?} at byte position {}",
            self.ch, self.byte_position
        )
    }
}

impl std::error::Error for NonAsciiError {}

/// Double Metaphone codes of `s` with unlimited size (still capped at 4 chars
/// by the core algorithm). Wrapper semantics: primary == secondary collapses
/// the secondary to `None`; an empty code becomes `None`.
///
/// Scaffold stub: panics until the dmetaphone-port feature lands.
pub fn dmetaphone(_s: &str) -> DmetaphoneResult {
    unimplemented!(
        "dmetaphone: ported by the dmetaphone-port feature (architecture.md section 5.3)"
    )
}

/// Double Metaphone codes of `s`, truncated to `size` (`size == 0` means
/// unlimited). Same wrapper semantics as [`dmetaphone`].
///
/// Scaffold stub: panics until the dmetaphone-port feature lands.
pub fn dmetaphone_with_size(_size: usize, _s: &str) -> DmetaphoneResult {
    unimplemented!(
        "dmetaphone_with_size: ported by the dmetaphone-port feature (architecture.md section 5.3)"
    )
}

/// Raw bytes-level entry point: no ASCII validation, no `None`-collapse, no
/// size truncation. A faithful exposure of the C algorithm, including the
/// Latin-1 arms (bytes 0xC7 and 0xD1).
///
/// Scaffold stub: panics until the dmetaphone-port feature lands.
pub fn dmetaphone_bytes(_input: &[u8]) -> (Vec<u8>, Vec<u8>) {
    unimplemented!(
        "dmetaphone_bytes: ported by the dmetaphone-port feature (architecture.md section 5.3)"
    )
}
