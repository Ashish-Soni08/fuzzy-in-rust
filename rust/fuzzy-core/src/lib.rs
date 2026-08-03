#![forbid(unsafe_code)]

//! fuzzy-core: safe-Rust port of the `fuzzy` phonetic-algorithms library
//! (Soundex, NYSIIS, Double Metaphone).
//!
//! Public API is pinned by architecture.md section 4; normative behavior by
//! architecture.md section 5 (including the two sanctioned upstream bug fixes
//! #14/#15 for Soundex). The original C/Cython sources under `src/` are the
//! ground truth for everything else.
//!
//! # API
//!
//! - `soundex(size, s) -> String` — Soundex with the fixed #14/#15 semantics
//!   (unicode-uppercase then A-Z filter; `size > 4` is a maximum, not a pad
//!   target).
//! - `nysiis(s) -> String` — exact NYSIIS port, quirks included.
//! - `dmetaphone(s)` / `dmetaphone_with_size(size, s)` — Double Metaphone,
//!   returning `Result<(Option<Vec<u8>>, Option<Vec<u8>>), NonAsciiError>`;
//!   non-ASCII input is rejected (original behavior preserved).
//! - `dmetaphone_bytes(input: &[u8]) -> (Vec<u8>, Vec<u8>)` — raw bytes-level
//!   entry point (no ASCII validation, no None-collapse, no size truncation):
//!   a faithful exposure of the C algorithm, including the Latin-1 arms.

mod dmetaphone;
mod nysiis;
mod soundex;

pub use dmetaphone::{
    dmetaphone, dmetaphone_bytes, dmetaphone_with_size, Codes, DmetaphoneResult, NonAsciiError,
};
pub use nysiis::nysiis;
pub use soundex::soundex;
