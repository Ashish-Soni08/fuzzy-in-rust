//! Native integration tests for the Soundex port (FIXED semantics,
//! architecture.md section 5.1 / RULEBOOK.md section 4).
//!
//! Every section 5.1 binding data point is a `#[test]` with `binding` in the
//! name (contract-filterable, VAL-SDX-024). The remaining tests pin the size
//! matrix, case normalization, the three dedup behaviors, unicode uppercase
//! expansion, non-letter stripping, the digit map, and long-input handling.

use fuzzy_core::soundex;

// ---------------------------------------------------------------------------
// Binding data points (architecture.md section 5.1 table — all seven).
// ---------------------------------------------------------------------------

#[test]
fn soundex_binding_fuzzy_readme_f200() {
    // README doctest data point.
    assert_eq!(soundex(4, "fuzzy"), "F200");
}

#[test]
fn soundex_binding_fancyfree_f521() {
    // test_soundex_result (xfail upstream, issue #14).
    assert_eq!(soundex(4, "FancyFree"), "F521");
}

#[test]
fn soundex_binding_test_size8_t23_no_padding() {
    // test_soundex_Test (xfail upstream, issue #14): size 8 is a maximum, NOT
    // a pad target. The original produced 'T2300000'.
    let code = soundex(8, "Test");
    assert_eq!(code, "T23");
    assert_eq!(code.len(), 3, "size > 4 must not pad");
}

#[test]
fn soundex_binding_jeroboam_non_ascii_j615() {
    // test_soundex_non_ascii (xfail upstream, issue #15): unicode-uppercase
    // (JÉROBOAM), then the A-Z filter drops É; no error.
    assert_eq!(soundex(8, "Jéroboam"), "J615");
}

#[test]
fn soundex_binding_empty_input_pads_0000() {
    // Original behavior preserved: empty input filters to empty, then pads.
    assert_eq!(soundex(4, ""), "0000");
}

#[test]
fn soundex_binding_digits_only_pads_0000() {
    // Original behavior preserved: digits are stripped by the A-Z filter.
    assert_eq!(soundex(4, "123"), "0000");
}

#[test]
fn soundex_binding_size_zero_returns_empty() {
    // Original behavior preserved: Soundex(0) -> '' (short-circuit).
    assert_eq!(soundex(0, "anything"), "");
}

// ---------------------------------------------------------------------------
// Size semantics.
// ---------------------------------------------------------------------------

#[test]
fn soundex_size_matrix_on_fuzzy() {
    // VAL-SDX-009: sizes <= 4 pad to exactly size; sizes > 4 never pad.
    // The raw code of 'fuzzy' is 'F2'.
    let expected = ["F", "F2", "F20", "F200", "F2", "F2", "F2"];
    for (size, want) in [1usize, 2, 3, 4, 5, 8, 100].into_iter().zip(expected) {
        assert_eq!(soundex(size, "fuzzy"), want, "size {size}");
    }
}

#[test]
fn soundex_size_zero_short_circuits_even_on_empty_input() {
    assert_eq!(soundex(0, ""), "");
}

#[test]
fn soundex_small_sizes_pad_an_empty_code() {
    // Padding target is exactly `size` whenever size <= 4, even from empty.
    assert_eq!(soundex(1, ""), "0");
    assert_eq!(soundex(2, ""), "00");
    assert_eq!(soundex(3, ""), "000");
}

#[test]
fn soundex_single_letter_pads_to_size() {
    assert_eq!(soundex(4, "A"), "A000");
    assert_eq!(soundex(2, "B"), "B0");
}

#[test]
fn soundex_stops_at_size() {
    // Emission stops as soon as `written == size`.
    assert_eq!(soundex(2, "FancyFree"), "F5");
    assert_eq!(soundex(3, "FancyFree"), "F52");
    assert_eq!(soundex(1, "fuzzy"), "F");
}

// ---------------------------------------------------------------------------
// Normalization (section 5.1 rule 1): unicode-uppercase, then keep A-Z only.
// ---------------------------------------------------------------------------

#[test]
fn soundex_case_normalization_lowercase_and_mixed() {
    // VAL-SDX-010: both forms uppercase to ROBERT -> R163.
    assert_eq!(soundex(4, "robert"), "R163");
    assert_eq!(soundex(4, "rObErT"), "R163");
}

#[test]
fn soundex_sharp_s_uppercases_to_ss_and_survives() {
    // VAL-SDX-014: 'ß'.to_uppercase() == "SS" (ASCII), surviving the filter.
    assert_eq!(soundex(4, "ß"), "S200");
    assert_eq!(soundex(4, "Straße"), "S362");
    assert_eq!(soundex(4, "Strasse"), "S362");
}

#[test]
fn soundex_non_letters_stripped_wherever_they_appear() {
    // VAL-SDX-015: digits/punctuation interleaved; whitespace covered here
    // (the CLI word token cannot carry spaces).
    assert_eq!(soundex(4, "f1u2z3z4y"), "F200");
    assert_eq!(soundex(4, "F-U-Z-Z-Y"), "F200");
    assert_eq!(soundex(4, "f u z z y"), "F200");
    assert_eq!(soundex(4, "  ...  "), "0000");
}

// ---------------------------------------------------------------------------
// Dedup rule (section 5.1 rule 4): append d iff d != '0' AND
// (written == 1 OR last_written != d). The simplified rule — do not
// "improve" it with classic H/W adjacency handling.
// ---------------------------------------------------------------------------

#[test]
fn soundex_dedup_first_digit_always_written() {
    // VAL-SDX-011: the written == 1 clause bypasses dedup for the first
    // digit even when the first two letters share a code.
    assert_eq!(soundex(4, "BB"), "B100");
    assert_eq!(soundex(4, "CK"), "C200");
}

#[test]
fn soundex_dedup_vowels_do_not_reset_last_written() {
    // VAL-SDX-012: skipped vowels (and H/W) do not reset dedup. Classic
    // Soundex would give B110 / T522; the original's simplified rule gives:
    assert_eq!(soundex(4, "BABAB"), "B100");
    assert_eq!(soundex(4, "Tymczak"), "T520");
}

#[test]
fn soundex_dedup_same_code_runs_collapse() {
    // VAL-SDX-013: consecutive same-code letters emit one digit.
    assert_eq!(soundex(4, "Jackson"), "J250"); // C,K,S all code 2
    assert_eq!(soundex(4, "Gutierrez"), "G362"); // RR code 6
}

// ---------------------------------------------------------------------------
// Digit map "01230120022455012623010202" (indexed by c - b'A').
// ---------------------------------------------------------------------------

#[test]
fn soundex_digit_map_contract_cases() {
    // VAL-SDX-022: pins D, H, L, P, Q, V, W, X as non-first letters.
    let cases = [
        ("AD", "A300"),
        ("AH", "A000"),
        ("AL", "A400"),
        ("AP", "A100"),
        ("AQ", "A200"),
        ("AV", "A100"),
        ("AW", "A000"),
        ("AX", "A200"),
    ];
    for (word, want) in cases {
        assert_eq!(soundex(4, word), want, "word {word}");
    }
}

#[test]
fn soundex_digit_map_full_alphabet_coverage() {
    // Every map entry pinned as a non-first letter: soundex(2, "A{L}") is
    // "A" + d when d != '0' (written == 1 clause), else "A" padded to "A0".
    const SPEC_MAP: &[u8; 26] = b"01230120022455012623010202";
    for (i, &d) in SPEC_MAP.iter().enumerate() {
        let letter = (b'A' + i as u8) as char;
        let word = format!("A{letter}");
        let want = if d == b'0' {
            "A0".to_string()
        } else {
            format!("A{}", d as char)
        };
        assert_eq!(soundex(2, &word), want, "map entry for {letter}");
    }
}

// ---------------------------------------------------------------------------
// Robustness.
// ---------------------------------------------------------------------------

#[test]
fn soundex_very_long_input() {
    // VAL-SDX-016: 10 000 chars; the second B emits '1' via the written == 1
    // clause, the remaining 9 998 are suppressed by dedup.
    let word = "B".repeat(10_000);
    assert_eq!(soundex(4, &word), "B100");
}

#[test]
fn soundex_long_input_with_large_size_stops_at_size() {
    // size > 4 is a maximum: emission stops at size even for long input.
    let word = "BCDFGH".repeat(100);
    let code = soundex(8, &word);
    assert_eq!(code.len(), 8);
    // B verbatim, then C=2 D=3 F=1 G=2 (H=0 skipped) B=1 C=2 D=3 -> stop at 8.
    assert_eq!(code, "B2312123");
}
