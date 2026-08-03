//! Double Metaphone, exact port of `src/double_metaphone.c` per
//! architecture.md section 5.3 and RULEBOOK.md section 6.
//!
//! Structure-preserving translation: a byte-oriented metastring model over
//! `Vec<u8>` (growable append, bounds-checked `get_at` returning 0 out of
//! range, exact-length `string_at` matcher), 5-space input padding with
//! `length`/`last` computed BEFORE padding, the initial GN/KN/PN/WR/PS skip
//! and X -> S rule, the main loop `while (primary.len < 4 || secondary.len
//! < 4)` with the `current >= length` break, the big switch ported arm by
//! arm (INCLUDING the two Latin-1 byte arms 0xC7/0xD1), and the 4-char cap.
//!
//! Wrapper semantics (collapse-equal, empty -> None, size truncation) live
//! only in the `&str` entry points; [`dmetaphone_bytes`] exposes the raw C
//! behavior. Every behavioral doubt was resolved against the compiled C
//! oracle (`tools/oracle-c/dmoracle.exe`), the ground truth.

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
pub fn dmetaphone(s: &str) -> DmetaphoneResult {
    dmetaphone_with_size(0, s)
}

/// Double Metaphone codes of `s`, truncated to `size` (`size == 0` means
/// unlimited). Same wrapper semantics as [`dmetaphone`].
pub fn dmetaphone_with_size(size: usize, s: &str) -> DmetaphoneResult {
    // RULEBOOK 6.3 W1: ASCII validation first (the original raises
    // UnicodeEncodeError on non-ASCII; preserved deliberately).
    if let Some((byte_position, ch)) = s.char_indices().find(|&(_, c)| !c.is_ascii()) {
        return Err(NonAsciiError::new(ch, byte_position));
    }
    let (primary, secondary) = dmetaphone_bytes(s.as_bytes());
    // RULEBOOK 6.3 W2+W3, order binding: collapse equal codes BEFORE
    // truncating (`dmetaphone 1 bier` -> P|P, not P|-); an empty code maps
    // to None; size == 0 is unlimited (the core already caps at 4, so only
    // sizes 1..=3 ever truncate).
    let finish = |mut code: Vec<u8>| {
        if code.is_empty() {
            None
        } else {
            if size > 0 {
                code.truncate(size);
            }
            Some(code)
        }
    };
    let secondary = if primary == secondary {
        None
    } else {
        finish(secondary)
    };
    Ok((finish(primary), secondary))
}

/// Raw bytes-level entry point: no ASCII validation, no `None`-collapse, no
/// size truncation. A faithful exposure of the C algorithm, including the
/// Latin-1 arms (bytes 0xC7 and 0xD1).
pub fn dmetaphone_bytes(input: &[u8]) -> (Vec<u8>, Vec<u8>) {
    // RULEBOOK D1: the real length/last are computed BEFORE padding; then the
    // input is padded with 5 spaces. All lookahead relies on the padding.
    let length = input.len() as isize;
    let last = length - 1;
    let mut original = Vec::with_capacity(input.len() + 7);
    original.extend_from_slice(input);
    original.extend_from_slice(b"     ");
    // RULEBOOK D2/G4/G8: C `MakeUpper` is byte-wise `toupper` and runs AFTER
    // padding; MSVC passes high bytes through unchanged, which
    // `make_ascii_uppercase` matches exactly.
    original.make_ascii_uppercase();

    let mut primary: Vec<u8> = Vec::new();
    let mut secondary: Vec<u8> = Vec::new();
    let mut current: isize = 0;

    // RULEBOOK D3: skip these when at start of word.
    if string_at(&original, 0, 2, &[b"GN", b"KN", b"PN", b"WR", b"PS"]) {
        current += 1;
    }

    // Initial 'X' is pronounced 'Z' e.g. 'Xavier'.
    if get_at(&original, 0) == b'X' {
        primary.push(b'S'); // 'Z' maps to 'S'
        secondary.push(b'S');
        current += 1;
    }

    // RULEBOOK D4: the OR condition is load-bearing — the loop stays alive
    // for the secondary after the primary reaches 4 (tagliarb -> TKLR|TLRP).
    while primary.len() < 4 || secondary.len() < 4 {
        if current >= length {
            break;
        }

        match get_at(&original, current) {
            b'A' | b'E' | b'I' | b'O' | b'U' | b'Y' => {
                if current == 0 {
                    // all init vowels now map to 'A'
                    primary.push(b'A');
                    secondary.push(b'A');
                }
                current += 1;
            }

            b'B' => {
                // "-mb", e.g. "dumb", already skipped over...
                primary.push(b'P');
                secondary.push(b'P');
                current += if get_at(&original, current + 1) == b'B' {
                    2
                } else {
                    1
                };
            }

            // QUIRK (gap G1): Latin-1 case label 'Ç' from the C source.
            0xC7 => {
                primary.push(b'S');
                secondary.push(b'S');
                current += 1;
            }

            b'C' => {
                // various germanic
                if current > 1
                    && !is_vowel(&original, current - 2)
                    && string_at(&original, current - 1, 3, &[b"ACH"])
                    && get_at(&original, current + 2) != b'I'
                    && (get_at(&original, current + 2) != b'E'
                        || string_at(&original, current - 2, 6, &[b"BACHER", b"MACHER"]))
                {
                    primary.push(b'K');
                    secondary.push(b'K');
                    current += 2;
                // special case 'caesar'
                } else if current == 0 && string_at(&original, current, 6, &[b"CAESAR"]) {
                    primary.push(b'S');
                    secondary.push(b'S');
                    current += 2;
                // italian 'chianti'
                } else if string_at(&original, current, 4, &[b"CHIA"]) {
                    primary.push(b'K');
                    secondary.push(b'K');
                    current += 2;
                } else if string_at(&original, current, 2, &[b"CH"]) {
                    // find 'michael'
                    if current > 0 && string_at(&original, current, 4, &[b"CHAE"]) {
                        primary.push(b'K');
                        secondary.push(b'X');
                    // greek roots e.g. 'chemistry', 'chorus'
                    } else if current == 0
                        && (string_at(&original, current + 1, 5, &[b"HARAC", b"HARIS"])
                            || string_at(
                                &original,
                                current + 1,
                                3,
                                &[b"HOR", b"HYM", b"HIA", b"HEM"],
                            ))
                        && !string_at(&original, 0, 5, &[b"CHORE"])
                    {
                        primary.push(b'K');
                        secondary.push(b'K');
                    // germanic, greek, or otherwise 'ch' for 'kh' sound
                    } else if string_at(&original, 0, 4, &[b"VAN ", b"VON "])
                        || string_at(&original, 0, 3, &[b"SCH"])
                        // 'architect' but not 'arch', 'orchestra', 'orchid'
                        || string_at(
                            &original,
                            current - 2,
                            6,
                            &[b"ORCHES", b"ARCHIT", b"ORCHID"],
                        )
                        || string_at(&original, current + 2, 1, &[b"T", b"S"])
                        || ((string_at(&original, current - 1, 1, &[b"A", b"O", b"U", b"E"])
                            || current == 0)
                            // e.g., 'wachtler', 'wechsler', but not 'tichner'
                            && string_at(
                                &original,
                                current + 2,
                                1,
                                &[b"L", b"R", b"N", b"M", b"B", b"H", b"F", b"V", b"W", b" "],
                            ))
                    {
                        primary.push(b'K');
                        secondary.push(b'K');
                    } else if current > 0 {
                        if string_at(&original, 0, 2, &[b"MC"]) {
                            // e.g., "McHugh"
                            primary.push(b'K');
                            secondary.push(b'K');
                        } else {
                            primary.push(b'X');
                            secondary.push(b'K');
                        }
                    } else {
                        primary.push(b'X');
                        secondary.push(b'X');
                    }
                    current += 2;
                // e.g, 'czerny'
                } else if string_at(&original, current, 2, &[b"CZ"])
                    && !string_at(&original, current - 2, 4, &[b"WICZ"])
                {
                    primary.push(b'S');
                    secondary.push(b'X');
                    current += 2;
                // e.g., 'focaccia'
                } else if string_at(&original, current + 1, 3, &[b"CIA"]) {
                    primary.push(b'X');
                    secondary.push(b'X');
                    current += 3;
                } else {
                    // double 'C', but not if e.g. 'McClellan'
                    let double_c = string_at(&original, current, 2, &[b"CC"])
                        && !(current == 1 && get_at(&original, 0) == b'M');
                    if double_c
                        // 'bellocchio' but not 'bacchus'
                        && string_at(&original, current + 2, 1, &[b"I", b"E", b"H"])
                        && !string_at(&original, current + 2, 2, &[b"HU"])
                    {
                        // 'accident', 'accede', 'succeed' get KS;
                        // 'bacci', 'bertucci', other italian get X.
                        if (current == 1 && get_at(&original, current - 1) == b'A')
                            || string_at(&original, current - 1, 5, &[b"UCCEE", b"UCCES"])
                        {
                            primary.extend_from_slice(b"KS");
                            secondary.extend_from_slice(b"KS");
                        } else {
                            primary.push(b'X');
                            secondary.push(b'X');
                        }
                        current += 3;
                    } else if !double_c {
                        // QUIRK: Pierce's rule fires for ANY lone C not
                        // followed by C/H/Z/I/E/Y and skips TWO characters,
                        // swallowing the next one (oracle: bcdfgh -> PKFK).
                        primary.push(b'K');
                        secondary.push(b'K');
                        current += 2;
                    // CC matched but the inner rule failed: fall through to
                    // the generic C handling, exactly like the C.
                    } else if string_at(&original, current, 2, &[b"CK", b"CG", b"CQ"]) {
                        primary.push(b'K');
                        secondary.push(b'K');
                        current += 2;
                    } else if string_at(&original, current, 2, &[b"CI", b"CE", b"CY"]) {
                        // italian vs. english
                        if string_at(&original, current, 3, &[b"CIO", b"CIE", b"CIA"]) {
                            primary.push(b'S');
                            secondary.push(b'X');
                        } else {
                            primary.push(b'S');
                            secondary.push(b'S');
                        }
                        current += 2;
                    } else {
                        primary.push(b'K');
                        secondary.push(b'K');
                        // name sent in 'mac caffrey', 'mac gregor'
                        if string_at(&original, current + 1, 2, &[b" C", b" Q", b" G"]) {
                            current += 3;
                        } else if string_at(&original, current + 1, 1, &[b"C", b"K", b"Q"])
                            && !string_at(&original, current + 1, 2, &[b"CE", b"CI"])
                        {
                            current += 2;
                        } else {
                            current += 1;
                        }
                    }
                }
            }

            b'D' => {
                if string_at(&original, current, 2, &[b"DG"]) {
                    if string_at(&original, current + 2, 1, &[b"I", b"E", b"Y"]) {
                        // e.g. 'edge'
                        primary.push(b'J');
                        secondary.push(b'J');
                        current += 3;
                    } else {
                        // e.g. 'edgar'
                        primary.extend_from_slice(b"TK");
                        secondary.extend_from_slice(b"TK");
                        current += 2;
                    }
                } else if string_at(&original, current, 2, &[b"DT", b"DD"]) {
                    primary.push(b'T');
                    secondary.push(b'T');
                    current += 2;
                } else {
                    primary.push(b'T');
                    secondary.push(b'T');
                    current += 1;
                }
            }

            b'F' => {
                current += if get_at(&original, current + 1) == b'F' {
                    2
                } else {
                    1
                };
                primary.push(b'F');
                secondary.push(b'F');
            }

            b'G' => {
                if get_at(&original, current + 1) == b'H' {
                    if current > 0 && !is_vowel(&original, current - 1) {
                        primary.push(b'K');
                        secondary.push(b'K');
                        current += 2;
                    } else if current == 0 {
                        // 'ghislane', 'ghiradelli' (C nests this under
                        // `current < 3`, which only current == 0 can reach
                        // here)
                        if get_at(&original, current + 2) == b'I' {
                            primary.push(b'J');
                            secondary.push(b'J');
                        } else {
                            primary.push(b'K');
                            secondary.push(b'K');
                        }
                        current += 2;
                    // Parker's rule (with some further refinements) - e.g. 'hugh'
                    } else if (current > 1
                        && string_at(&original, current - 2, 1, &[b"B", b"H", b"D"]))
                        // e.g., 'bough'
                        || (current > 2
                            && string_at(&original, current - 3, 1, &[b"B", b"H", b"D"]))
                        // e.g., 'broughton'
                        || (current > 3 && string_at(&original, current - 4, 1, &[b"B", b"H"]))
                    {
                        current += 2;
                    } else {
                        // e.g., 'laugh', 'McLaughlin', 'cough', 'gough',
                        // 'rough', 'tough'
                        if current > 2
                            && get_at(&original, current - 1) == b'U'
                            && string_at(&original, current - 3, 1, &[b"C", b"G", b"L", b"R", b"T"])
                        {
                            primary.push(b'F');
                            secondary.push(b'F');
                        } else if current > 0 && get_at(&original, current - 1) != b'I' {
                            primary.push(b'K');
                            secondary.push(b'K');
                        }
                        current += 2;
                    }
                } else if get_at(&original, current + 1) == b'N' {
                    if current == 1 && is_vowel(&original, 0) && !slavo_germanic(&original) {
                        primary.extend_from_slice(b"KN");
                        secondary.push(b'N');
                    // not e.g. 'cagney'
                    } else if !string_at(&original, current + 2, 2, &[b"EY"])
                        && get_at(&original, current + 1) != b'Y'
                        && !slavo_germanic(&original)
                    {
                        primary.push(b'N');
                        secondary.extend_from_slice(b"KN");
                    } else {
                        primary.extend_from_slice(b"KN");
                        secondary.extend_from_slice(b"KN");
                    }
                    current += 2;
                // 'tagliaro'
                } else if string_at(&original, current + 1, 2, &[b"LI"])
                    && !slavo_germanic(&original)
                {
                    primary.extend_from_slice(b"KL");
                    secondary.push(b'L');
                    current += 2;
                // -ges-,-gep-,-gel-, -gie- at beginning
                } else if current == 0
                    && (get_at(&original, current + 1) == b'Y'
                        || string_at(
                            &original,
                            current + 1,
                            2,
                            &[
                                b"ES", b"EP", b"EB", b"EL", b"EY", b"IB", b"IL", b"IN", b"IE",
                                b"EI", b"ER",
                            ],
                        ))
                {
                    primary.push(b'K');
                    secondary.push(b'J');
                    current += 2;
                // -ger-, -gy-
                } else if (string_at(&original, current + 1, 2, &[b"ER"])
                    || get_at(&original, current + 1) == b'Y')
                    && !string_at(&original, 0, 6, &[b"DANGER", b"RANGER", b"MANGER"])
                    && !string_at(&original, current - 1, 1, &[b"E", b"I"])
                    && !string_at(&original, current - 1, 3, &[b"RGY", b"OGY"])
                {
                    primary.push(b'K');
                    secondary.push(b'J');
                    current += 2;
                // italian e.g, 'biaggi'
                } else if string_at(&original, current + 1, 1, &[b"E", b"I", b"Y"])
                    || string_at(&original, current - 1, 4, &[b"AGGI", b"OGGI"])
                {
                    // obvious germanic
                    if string_at(&original, 0, 4, &[b"VAN ", b"VON "])
                        || string_at(&original, 0, 3, &[b"SCH"])
                        || string_at(&original, current + 1, 2, &[b"ET"])
                    {
                        primary.push(b'K');
                        secondary.push(b'K');
                    // always soft if french ending
                    } else if string_at(&original, current + 1, 4, &[b"IER "]) {
                        primary.push(b'J');
                        secondary.push(b'J');
                    } else {
                        primary.push(b'J');
                        secondary.push(b'K');
                    }
                    current += 2;
                } else {
                    current += if get_at(&original, current + 1) == b'G' {
                        2
                    } else {
                        1
                    };
                    primary.push(b'K');
                    secondary.push(b'K');
                }
            }

            b'H' => {
                // only keep if first & before vowel or btw. 2 vowels
                if (current == 0 || is_vowel(&original, current - 1))
                    && is_vowel(&original, current + 1)
                {
                    primary.push(b'H');
                    secondary.push(b'H');
                    current += 2;
                } else {
                    // also takes care of 'HH'
                    current += 1;
                }
            }

            b'J' => {
                // obvious spanish, 'jose', 'san jacinto'
                if string_at(&original, current, 4, &[b"JOSE"])
                    || string_at(&original, 0, 4, &[b"SAN "])
                {
                    if (current == 0 && get_at(&original, current + 4) == b' ')
                        || string_at(&original, 0, 4, &[b"SAN "])
                    {
                        primary.push(b'H');
                        secondary.push(b'H');
                    } else {
                        primary.push(b'J');
                        secondary.push(b'H');
                    }
                    current += 1;
                } else {
                    // (JOSE at current is already excluded by the branch
                    // above, so `current == 0` suffices for the C's
                    // `current == 0 && !StringAt(JOSE)`.)
                    if current == 0 {
                        // Yankelovich/Jankelowicz
                        primary.push(b'J');
                        secondary.push(b'A');
                    // spanish pron. of e.g. 'bajador'
                    } else if is_vowel(&original, current - 1)
                        && !slavo_germanic(&original)
                        && (get_at(&original, current + 1) == b'A'
                            || get_at(&original, current + 1) == b'O')
                    {
                        primary.push(b'J');
                        secondary.push(b'H');
                    } else if current == last {
                        primary.push(b'J');
                        // C: MetaphAdd(secondary, "") — a no-op.
                    } else if !string_at(
                        &original,
                        current + 1,
                        1,
                        &[b"L", b"T", b"K", b"S", b"N", b"M", b"B", b"Z"],
                    ) && !string_at(&original, current - 1, 1, &[b"S", b"K", b"L"])
                    {
                        primary.push(b'J');
                        secondary.push(b'J');
                    }
                    // it could happen!
                    current += if get_at(&original, current + 1) == b'J' {
                        2
                    } else {
                        1
                    };
                }
            }

            b'K' => {
                current += if get_at(&original, current + 1) == b'K' {
                    2
                } else {
                    1
                };
                primary.push(b'K');
                secondary.push(b'K');
            }

            b'L' => {
                if get_at(&original, current + 1) == b'L' {
                    // spanish e.g. 'cabrillo', 'gallegos'
                    if (current == length - 3
                        && string_at(&original, current - 1, 4, &[b"ILLO", b"ILLA", b"ALLE"]))
                        || ((string_at(&original, last - 1, 2, &[b"AS", b"OS"])
                            || string_at(&original, last, 1, &[b"A", b"O"]))
                            && string_at(&original, current - 1, 4, &[b"ALLE"]))
                    {
                        primary.push(b'L');
                        // C: MetaphAdd(secondary, "") — a no-op.
                        current += 2;
                    } else {
                        current += 2;
                        primary.push(b'L');
                        secondary.push(b'L');
                    }
                } else {
                    current += 1;
                    primary.push(b'L');
                    secondary.push(b'L');
                }
            }

            b'M' => {
                // 'dumb', 'thumb'
                if (string_at(&original, current - 1, 3, &[b"UMB"])
                    && (current + 1 == last || string_at(&original, current + 2, 2, &[b"ER"])))
                    || get_at(&original, current + 1) == b'M'
                {
                    current += 2;
                } else {
                    current += 1;
                }
                primary.push(b'M');
                secondary.push(b'M');
            }

            b'N' => {
                current += if get_at(&original, current + 1) == b'N' {
                    2
                } else {
                    1
                };
                primary.push(b'N');
                secondary.push(b'N');
            }

            // QUIRK (gap G1): Latin-1 case label 'Ñ' from the C source.
            0xD1 => {
                current += 1;
                primary.push(b'N');
                secondary.push(b'N');
            }

            b'P' => {
                if get_at(&original, current + 1) == b'H' {
                    primary.push(b'F');
                    secondary.push(b'F');
                    current += 2;
                } else {
                    // also account for "campbell", "raspberry"
                    current += if string_at(&original, current + 1, 1, &[b"P", b"B"]) {
                        2
                    } else {
                        1
                    };
                    primary.push(b'P');
                    secondary.push(b'P');
                }
            }

            b'Q' => {
                current += if get_at(&original, current + 1) == b'Q' {
                    2
                } else {
                    1
                };
                primary.push(b'K');
                secondary.push(b'K');
            }

            b'R' => {
                // french e.g. 'rogier', but exclude 'hochmeier'
                if current == last
                    && !slavo_germanic(&original)
                    && string_at(&original, current - 2, 2, &[b"IE"])
                    && !string_at(&original, current - 4, 2, &[b"ME", b"MA"])
                {
                    // C: MetaphAdd(primary, "") — a no-op.
                    secondary.push(b'R');
                } else {
                    primary.push(b'R');
                    secondary.push(b'R');
                }
                current += if get_at(&original, current + 1) == b'R' {
                    2
                } else {
                    1
                };
            }

            b'S' => {
                // special cases 'island', 'isle', 'carlisle', 'carlysle'
                if string_at(&original, current - 1, 3, &[b"ISL", b"YSL"]) {
                    current += 1;
                // special case 'sugar-'
                } else if current == 0 && string_at(&original, current, 5, &[b"SUGAR"]) {
                    primary.push(b'X');
                    secondary.push(b'S');
                    current += 1;
                } else if string_at(&original, current, 2, &[b"SH"]) {
                    // germanic
                    if string_at(
                        &original,
                        current + 1,
                        4,
                        &[b"HEIM", b"HOEK", b"HOLM", b"HOLZ"],
                    ) {
                        primary.push(b'S');
                        secondary.push(b'S');
                    } else {
                        primary.push(b'X');
                        secondary.push(b'X');
                    }
                    current += 2;
                // italian & armenian
                } else if string_at(&original, current, 3, &[b"SIO", b"SIA"])
                    || string_at(&original, current, 4, &[b"SIAN"])
                {
                    if !slavo_germanic(&original) {
                        primary.push(b'S');
                        secondary.push(b'X');
                    } else {
                        primary.push(b'S');
                        secondary.push(b'S');
                    }
                    current += 3;
                // german & anglicisations, e.g. 'smith' match 'schmidt',
                // 'snider' match 'schneider'; also -sz- in slavic languages
                // (altho in hungarian it is pronounced 's')
                } else if (current == 0
                    && string_at(&original, current + 1, 1, &[b"M", b"N", b"L", b"W"]))
                    || string_at(&original, current + 1, 1, &[b"Z"])
                {
                    primary.push(b'S');
                    secondary.push(b'X');
                    current += if string_at(&original, current + 1, 1, &[b"Z"]) {
                        2
                    } else {
                        1
                    };
                // Schlesinger's rule. QUIRK: in the C source the I/E/Y and
                // SK/SK handlers sit INSIDE the `GetAt(current+2) == 'H'`
                // block after an if/else whose branches both break — dead
                // code. SC followed by a non-H letter therefore falls THROUGH
                // to the generic S handling below (oracle: science -> SKNK,
                // not SNS). The dead code is omitted here (rustc would flag
                // it as unreachable).
                } else if string_at(&original, current, 2, &[b"SC"])
                    && get_at(&original, current + 2) == b'H'
                {
                    // dutch origin, e.g. 'school', 'schooner'
                    if string_at(
                        &original,
                        current + 3,
                        2,
                        &[b"OO", b"ER", b"EN", b"UY", b"ED", b"EM"],
                    ) {
                        // 'schermerhorn', 'schenker'
                        if string_at(&original, current + 3, 2, &[b"ER", b"EN"]) {
                            primary.push(b'X');
                            secondary.extend_from_slice(b"SK");
                        } else {
                            primary.extend_from_slice(b"SK");
                            secondary.extend_from_slice(b"SK");
                        }
                        current += 3;
                    } else {
                        if current == 0 && !is_vowel(&original, 3) && get_at(&original, 3) != b'W' {
                            primary.push(b'X');
                            secondary.push(b'S');
                        } else {
                            primary.push(b'X');
                            secondary.push(b'X');
                        }
                        current += 3;
                    }
                } else {
                    // french e.g. 'resnais', 'artois'
                    if current == last && string_at(&original, current - 2, 2, &[b"AI", b"OI"]) {
                        // C: MetaphAdd(primary, "") — a no-op.
                        secondary.push(b'S');
                    } else {
                        primary.push(b'S');
                        secondary.push(b'S');
                    }
                    current += if string_at(&original, current + 1, 1, &[b"S", b"Z"]) {
                        2
                    } else {
                        1
                    };
                }
            }

            b'T' => {
                // The C has separate TION and TIA/TCH ifs with identical
                // bodies; merged here (clippy if_same_then_else).
                if string_at(&original, current, 4, &[b"TION"])
                    || string_at(&original, current, 3, &[b"TIA", b"TCH"])
                {
                    primary.push(b'X');
                    secondary.push(b'X');
                    current += 3;
                } else if string_at(&original, current, 2, &[b"TH"])
                    || string_at(&original, current, 3, &[b"TTH"])
                {
                    // special case 'thomas', 'thames' or germanic
                    if string_at(&original, current + 2, 2, &[b"OM", b"AM"])
                        || string_at(&original, 0, 4, &[b"VAN ", b"VON "])
                        || string_at(&original, 0, 3, &[b"SCH"])
                    {
                        primary.push(b'T');
                        secondary.push(b'T');
                    } else {
                        // QUIRK: the C emits the DIGIT CHARACTER '0' for the
                        // primary here ("yes, zero" per the C comment).
                        primary.push(b'0');
                        secondary.push(b'T');
                    }
                    current += 2;
                } else {
                    current += if string_at(&original, current + 1, 1, &[b"T", b"D"]) {
                        2
                    } else {
                        1
                    };
                    primary.push(b'T');
                    secondary.push(b'T');
                }
            }

            b'V' => {
                current += if get_at(&original, current + 1) == b'V' {
                    2
                } else {
                    1
                };
                primary.push(b'F');
                secondary.push(b'F');
            }

            b'W' => {
                // can also be in middle of word
                if string_at(&original, current, 2, &[b"WR"]) {
                    primary.push(b'R');
                    secondary.push(b'R');
                    current += 2;
                } else {
                    if current == 0
                        && (is_vowel(&original, current + 1)
                            || string_at(&original, current, 2, &[b"WH"]))
                    {
                        // Wasserman should match Vasserman
                        if is_vowel(&original, current + 1) {
                            primary.push(b'A');
                            secondary.push(b'F');
                        } else {
                            // need Uomo to match Womo
                            primary.push(b'A');
                            secondary.push(b'A');
                        }
                        // QUIRK: no break/increment in the C — the Arnow and
                        // WICZ/WITZ rules below can fire on top (oracle:
                        // witz -> ATS|FFX).
                    }

                    // Arnow should match Arnoff
                    if (current == last && is_vowel(&original, current - 1))
                        || string_at(
                            &original,
                            current - 1,
                            5,
                            &[b"EWSKI", b"EWSKY", b"OWSKI", b"OWSKY"],
                        )
                        || string_at(&original, 0, 3, &[b"SCH"])
                    {
                        // C: MetaphAdd(primary, "") — a no-op.
                        secondary.push(b'F');
                        current += 1;
                    // polish e.g. 'filipowicz'
                    } else if string_at(&original, current, 4, &[b"WICZ", b"WITZ"]) {
                        primary.extend_from_slice(b"TS");
                        secondary.extend_from_slice(b"FX");
                        current += 4;
                    } else {
                        // else skip it
                        current += 1;
                    }
                }
            }

            b'X' => {
                // french e.g. breaux
                if !(current == last
                    && (string_at(&original, current - 3, 3, &[b"IAU", b"EAU"])
                        || string_at(&original, current - 2, 2, &[b"AU", b"OU"])))
                {
                    primary.extend_from_slice(b"KS");
                    secondary.extend_from_slice(b"KS");
                }
                current += if string_at(&original, current + 1, 1, &[b"C", b"X"]) {
                    2
                } else {
                    1
                };
            }

            b'Z' => {
                // chinese pinyin e.g. 'zhao'
                if get_at(&original, current + 1) == b'H' {
                    primary.push(b'J');
                    secondary.push(b'J');
                    current += 2;
                } else {
                    if string_at(&original, current + 1, 2, &[b"ZO", b"ZI", b"ZA"])
                        || (slavo_germanic(&original)
                            && current > 0
                            && get_at(&original, current - 1) != b'T')
                    {
                        primary.push(b'S');
                        secondary.extend_from_slice(b"TS");
                    } else {
                        primary.push(b'S');
                        secondary.push(b'S');
                    }
                    current += if get_at(&original, current + 1) == b'Z' {
                        2
                    } else {
                        1
                    };
                }
            }

            // default: digits, punctuation, padding spaces and any other
            // byte are skipped silently (VAL-DM-017).
            _ => current += 1,
        }
    }

    // RULEBOOK D6/G7: the C writes '\0' at index 4 via SetAt without updating
    // the length; `Vec::truncate(4)` is the observably equivalent form.
    primary.truncate(4);
    secondary.truncate(4);

    (primary, secondary)
}

/// C `GetAt`: bounds-checked byte fetch, returning 0 out of range (the C
/// returns `'\0'`). This bounds rule is load-bearing: ALL lookahead and
/// lookbehind relies on it (RULEBOOK 6.1, gap G5).
fn get_at(s: &[u8], pos: isize) -> u8 {
    if pos < 0 || pos >= s.len() as isize {
        0
    } else {
        s[pos as usize]
    }
}

/// C `IsVowel`: A/E/I/O/U/Y — Y included, per the C source.
fn is_vowel(s: &[u8], pos: isize) -> bool {
    matches!(get_at(s, pos), b'A' | b'E' | b'I' | b'O' | b'U' | b'Y')
}

/// C `SlavoGermanic`: crude substring check for W, K, CZ, WITZ — replicated,
/// not "fixed" (RULEBOOK 6.1).
fn slavo_germanic(s: &[u8]) -> bool {
    [b"W".as_slice(), b"K", b"CZ", b"WITZ"]
        .iter()
        .any(|needle| contains_subslice(s, needle))
}

/// Substring search standing in for the C `strstr` over the padded buffer.
fn contains_subslice(haystack: &[u8], needle: &[u8]) -> bool {
    haystack.windows(needle.len()).any(|w| w == needle)
}

/// C `StringAt(start, length, ...)`: exact-`length` byte compare at `start`
/// against each pattern (the C varargs list is `""`-terminated; `strncmp`
/// compares exactly `length` bytes, so a pattern shorter than `length` can
/// never match and a longer one matches on its prefix — `starts_with`
/// reproduces both). `start < 0` or at/past the (padded) length yields false,
/// like the C guard (RULEBOOK 6.1, gaps G3/G9).
fn string_at(s: &[u8], start: isize, length: usize, pats: &[&[u8]]) -> bool {
    if start < 0 || start as usize >= s.len() {
        return false;
    }
    let start = start as usize;
    match s.get(start..start + length) {
        Some(hay) => pats.iter().any(|p| p.starts_with(hay)),
        None => false,
    }
}
