//! Soundex, FIXED semantics per architecture.md section 5.1 (upstream bugs
//! #14 padding and #15 non-ASCII are intentionally fixed here).

/// Letter -> digit map (classic), indexed by `c - b'A'` (A=0 .. Z=25).
const DIGIT_MAP: &[u8; 26] = b"01230120022455012623010202";

/// Soundex code of `s`, capped at `size` characters.
///
/// Pinned signature (architecture.md section 4). Behavior (section 5.1,
/// rules S0-S5): unicode-uppercase then filter to `A-Z`; first letter
/// verbatim; simplified dedup with the `written == 1` clause; stop at `size`;
/// pad with `'0'` to `size` only when `size <= 4` (`size > 4` is a maximum,
/// never a pad target); `size == 0` short-circuits to the empty string.
pub fn soundex(size: usize, s: &str) -> String {
    // S0: size 0 short-circuits (matches the original's Soundex(0) -> '').
    if size == 0 {
        return String::new();
    }

    // Capacity hint only: the output never exceeds `size`, and never exceeds
    // the filtered input length. Capped so a huge `size` cannot pre-allocate.
    let mut out = String::with_capacity(size.min(256));

    // S1 (#15 fix): unicode-uppercase (Python str.upper() equivalent), then
    // keep only ASCII A-Z — accented letters, digits, spaces and punctuation
    // are dropped. Multi-byte uppercase output (e.g. É) is >= 0x80 per byte
    // and filtered out; ß uppercases to the ASCII pair "SS" and survives.
    for byte in s.to_uppercase().bytes() {
        if !byte.is_ascii_uppercase() {
            continue;
        }
        if out.is_empty() {
            // S3: first letter verbatim.
            out.push(byte as char);
        } else {
            let d = DIGIT_MAP[usize::from(byte - b'A')];
            // S4: skip code-0 letters; otherwise append iff written == 1 OR
            // last_written != d. QUIRK (architecture.md section 5.1): the
            // written == 1 clause bypasses dedup for the first digit, and the
            // comparison is against the last WRITTEN character only — skipped
            // vowels/H/W do not reset dedup (simplified rule, no classic
            // H/W adjacency special-casing).
            if d != b'0' && (out.len() == 1 || out.as_bytes()[out.len() - 1] != d) {
                out.push(d as char);
            }
        }
        // S4: stop as soon as written == size.
        if out.len() == size {
            break;
        }
    }

    // S5 (#14 fix): pad with '0' to exactly `size` only when size <= 4;
    // for size > 4, `size` is a maximum length and no padding happens.
    if size <= 4 {
        while out.len() < size {
            out.push('0');
        }
    }

    // ASCII letters/digits by construction.
    out
}
