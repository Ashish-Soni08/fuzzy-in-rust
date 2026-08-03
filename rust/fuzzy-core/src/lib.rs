#![forbid(unsafe_code)]

//! fuzzy-core: safe-Rust port of the `fuzzy` phonetic-algorithms library
//! (Soundex, NYSIIS, Double Metaphone).
//!
//! Public API is pinned by architecture.md section 4; normative behavior by
//! architecture.md section 5 (including the two sanctioned upstream bug fixes
//! #14/#15 for Soundex). The original C/Cython sources under `src/` are the
//! ground truth for everything else.
//!
//! Scaffold state: every algorithm entry point is an `unimplemented!()` stub;
//! the ports land in the soundex-port / nysiis-port / dmetaphone-port features.

mod dmetaphone;
mod nysiis;
mod soundex;

pub use dmetaphone::{
    dmetaphone, dmetaphone_bytes, dmetaphone_with_size, Codes, DmetaphoneResult, NonAsciiError,
};
pub use nysiis::nysiis;
pub use soundex::soundex;
