//! NYSIIS, exact port of `src/fuzzy.pyx` lines 81-185 per architecture.md
//! section 5.2, including every documented quirk (unicode-uppercase then
//! `[^A-Z]` strip, trailing S/Z strip, MAC/PF prefixes, suffix loop, the
//! empty-string `'' in 'AEIOU'` quirk, consecutive-duplicate collapse).
//!
//! The four lookup tables below are transcribed verbatim from the .pyx, in
//! source order (entry counts cross-checked against the original:
//! suffix 10, transforms 18, not-first 19, middle 1).

/// `_nysiis_suffix_map` transcribed verbatim from `src/fuzzy.pyx`.
const SUFFIX_MAP: &[(&[u8], &[u8])] = &[
    (b"IX", b"IC"),
    (b"EX", b"EC"),
    (b"YE", b"Y"),
    (b"EE", b"Y"),
    (b"IE", b"Y"),
    (b"DT", b"D"),
    (b"RT", b"D"),
    (b"RD", b"D"),
    (b"NT", b"D"),
    (b"ND", b"D"),
];

/// `_nysiis_transforms` transcribed verbatim from `src/fuzzy.pyx`.
const TRANSFORMS: &[(&[u8], &[u8])] = &[
    (b"AY", b"Y"),
    (b"DG", b"G"),
    (b"E", b"A"),
    (b"EY", b"Y"),
    (b"GHT", b"GT"),
    (b"K", b"C"),
    (b"KN", b"N"),
    (b"I", b"A"),
    (b"IY", b"Y"),
    (b"O", b"A"),
    (b"OY", b"Y"),
    (b"PH", b"F"),
    (b"SH", b"S"),
    (b"SCH", b"S"),
    (b"U", b"A"),
    (b"UY", b"Y"),
    (b"WR", b"R"),
    (b"YW", b"Y"),
];

/// `_nysiis_trans_not_first` transcribed verbatim from `src/fuzzy.pyx`.
const TRANS_NOT_FIRST: &[(&[u8], &[u8])] = &[
    (b"AH", b"A"),
    (b"AW", b"A"),
    (b"EH", b"A"),
    (b"EV", b"AF"),
    (b"EW", b"A"),
    (b"HA", b"A"),
    (b"HE", b"A"),
    (b"HI", b"A"),
    (b"HO", b"A"),
    (b"HU", b"A"),
    (b"IH", b"A"),
    (b"IW", b"A"),
    (b"M", b"N"),
    (b"OH", b"A"),
    (b"OW", b"A"),
    (b"Q", b"G"),
    (b"UH", b"A"),
    (b"UW", b"A"),
    (b"Z", b"S"),
];

/// `_nysiis_trans_middle` transcribed verbatim from `src/fuzzy.pyx`.
const TRANS_MIDDLE: &[(&[u8], &[u8])] = &[(b"Y", b"A")];

/// Dict lookup over a verbatim-transcribed table.
fn lookup(table: &[(&'static [u8], &'static [u8])], key: &[u8]) -> Option<&'static [u8]> {
    table
        .iter()
        .find(|&&(k, _)| k == key)
        .map(|&(_, value)| value)
}

/// NYSIIS code of `s`.
///
/// Pinned signature (architecture.md section 4). Binding data points:
/// `nysiis("fuzzy") == "FASY"`, `nysiis("") == ""`, `nysiis("123") == ""`.
///
/// The pipeline below follows the .pyx statement for statement; step numbers
/// match architecture.md section 5.2 / RULEBOOK.md section 5.
pub fn nysiis(s: &str) -> String {
    // Step 1 (QUIRK Q1): unicode-aware uppercase (Python str.upper()
    // equivalent), THEN strip everything outside A-Z. 'ß' uppercases to the
    // ASCII pair "SS" and survives; 'é' uppercases to 'É' (non-ASCII bytes)
    // and is dropped.
    let mut filtered: Vec<u8> = Vec::with_capacity(s.len());
    for byte in s.to_uppercase().bytes() {
        if byte.is_ascii_uppercase() {
            filtered.push(byte);
        }
    }
    let mut s = filtered;
    let mut start = 0usize;
    let mut stop = s.len();

    // Step 2: first char of the filtered string (None is the Python '' case).
    let first: Option<u8> = if stop > 0 { Some(s[0]) } else { None };

    // Step 3 (QUIRK Q3): strip trailing S/Z BEFORE any prefix handling. Only
    // the stop index moves; the string itself is unchanged here.
    while stop > 0 && matches!(s[stop - 1], b'S' | b'Z') {
        stop -= 1;
    }

    // Step 4 (QUIRK Q4): initial MAC -> MC with stop adjusted; initial PF ->
    // start = 1 (the P is dropped from the scanned slice later).
    if s.starts_with(b"MAC") {
        // Python: s = 'MC' + s[3:]; stop = stop - 1. Note s[3:] keeps any
        // trailing S/Z that step 3 excluded via stop; the decremented stop
        // cuts them back off in the step-6 slice.
        let mut rewritten = Vec::with_capacity(s.len() - 1);
        rewritten.extend_from_slice(b"MC");
        rewritten.extend_from_slice(&s[3..]);
        s = rewritten;
        stop -= 1;
    } else if s.starts_with(b"PF") {
        start = 1;
    }

    // Step 5 (QUIRK Q5): suffix loop — while (stop - start) > 2, map the
    // trailing pair, PREPEND the mapping to the accumulated suffix, and stop
    // at the first unmapped pair. (stop >= start always: PF implies
    // stop >= 2 because s[1] == 'F' ends the trailing-S/Z strip.)
    let mut suffix: Vec<u8> = Vec::new();
    while stop - start > 2 {
        match lookup(SUFFIX_MAP, &s[stop - 2..stop]) {
            Some(mapped) => {
                let mut next = Vec::with_capacity(mapped.len() + suffix.len());
                next.extend_from_slice(mapped);
                next.extend_from_slice(&suffix);
                suffix = next;
                stop -= 2;
            }
            None => break,
        }
    }

    // Step 6: s = s[start:stop] + suffix; the main scan restarts at index 0
    // of this new string (Python rebinds i = start = 0).
    let mut scanned = Vec::with_capacity(stop - start + suffix.len());
    scanned.extend_from_slice(&s[start..stop]);
    scanned.extend_from_slice(&suffix);
    let s = scanned;

    // Step 7 (QUIRK Q6/Q7): main scan left to right. For each length 3, 2, 1
    // (longest first, first match wins): try TRANSFORMS; else, only when
    // i > start, try TRANS_NOT_FIRST; else, only when also i < stop - 1, try
    // TRANS_MIDDLE. Replacements may be multi-char (EV -> AF). Advance by the
    // matched key length; unmatched chars are copied verbatim.
    let mut r: Vec<u8> = Vec::with_capacity(s.len());
    let mut i = 0usize;
    let start = 0usize;
    let stop = s.len();
    while i < stop {
        let remain = stop - i;
        let mut matched: Option<(&'static [u8], usize)> = None;
        for len in [3usize, 2, 1] {
            if remain < len {
                continue;
            }
            let x = &s[i..i + len];
            if let Some(app) = lookup(TRANSFORMS, x) {
                matched = Some((app, len));
                break;
            } else if i > start {
                if let Some(app) = lookup(TRANS_NOT_FIRST, x) {
                    matched = Some((app, len));
                    break;
                } else if i < stop - 1 {
                    if let Some(app) = lookup(TRANS_MIDDLE, x) {
                        matched = Some((app, len));
                        break;
                    }
                }
            }
        }
        match matched {
            Some((app, len)) => {
                r.extend_from_slice(app);
                i += len;
            }
            None => {
                r.push(s[i]);
                i += 1;
            }
        }
    }

    // Step 8: trim trailing vowels; the trim length is recorded and still
    // governs the dedup slice after the step-9 restore (QUIRK Q8).
    let mut stop = r.len();
    while stop > 0 && matches!(r[stop - 1], b'A' | b'E' | b'I' | b'O' | b'U') {
        stop -= 1;
    }

    // Step 9 (QUIRK Q2, RULEBOOK.md G2): if the original first char is a
    // vowel, force output position 0 back to it. In Python `'' in 'AEIOU'`
    // is True, so EMPTY input takes this branch too; there r is empty and
    // stop is 0, so the Python `r = [first]` (r = ['']) is unobservable
    // through the r[:stop] slice and nysiis('') == ''.
    let first_in_aeiou = match first {
        None => true, // the Python ''-in-'AEIOU' quirk
        Some(f) => matches!(f, b'A' | b'E' | b'I' | b'O' | b'U'),
    };
    if first_in_aeiou {
        if let Some(f) = first {
            if r.is_empty() {
                // Python: r = [first]. Structurally unreachable (an empty r
                // implies an empty filtered string, hence first == None);
                // kept for fidelity with the .pyx.
                r.push(f);
            } else {
                r[0] = f;
            }
        }
    }

    // Step 10 (QUIRK Q9): collapse CONSECUTIVE duplicates only, over
    // r[:stop] (stop <= r.len() holds: the restore never grows r past an
    // already-zero stop); join.
    let mut out = String::with_capacity(stop);
    let mut last: Option<u8> = None;
    for &x in &r[..stop] {
        if last == Some(x) {
            continue;
        }
        out.push(x as char);
        last = Some(x);
    }
    out
}
