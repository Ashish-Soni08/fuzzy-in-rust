//! Soundex, FIXED semantics per architecture.md section 5.1 (upstream bugs
//! #14 padding and #15 non-ASCII are intentionally fixed here).

/// Soundex code of `s`, capped at `size` characters.
///
/// Pinned signature (architecture.md section 4). Behavior (section 5.1):
/// unicode-uppercase then filter to `A-Z`; first letter verbatim; simplified
/// dedup with the `written == 1` clause; pad with `'0'` to `size` only when
/// `size <= 4` (`size > 4` is a maximum, never a pad target); `size == 0`
/// short-circuits to the empty string.
///
/// Scaffold stub: panics until the soundex-port feature lands.
pub fn soundex(_size: usize, _s: &str) -> String {
    unimplemented!("soundex: ported by the soundex-port feature (architecture.md section 5.1)")
}
