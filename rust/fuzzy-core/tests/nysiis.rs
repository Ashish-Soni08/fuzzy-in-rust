//! Native integration tests for the NYSIIS port (EXACT port of
//! `src/fuzzy.pyx` lines 81-185, architecture.md section 5.2 / RULEBOOK.md
//! section 5).
//!
//! Every section 5.2 binding data point is a `#[test]` with `binding` in the
//! name (contract-filterable, VAL-NYS-027). The remaining tests pin every
//! pipeline stage and quirk in order: normalization (unicode-uppercase then
//! `[^A-Z]` strip), trailing S/Z strip, MAC/PF prefixes, the suffix-map loop,
//! the 3/2/1-char transform scan, the not-first map, the middle-Y rule,
//! trailing-vowel trim, first-vowel restore (including the empty-string and
//! all-vowels quirks), consecutive-duplicate collapse, and long input.
//!
//! Coverage note: every entry of all four lookup tables is exercised by at
//! least one case below (suffix map 10, transforms 18, not-first 19,
//! middle 1 — counts cross-checked against the .pyx).

use fuzzy_core::nysiis;

// ---------------------------------------------------------------------------
// Binding data points (architecture.md section 5.2 — all three).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_binding_fuzzy_readme_fasy() {
    // README doctest data point. Exercises U->A (transforms), Z->S
    // (not-first), consecutive-dup collapse of the two S's, and a final Y
    // kept verbatim (last position: middle-Y rule inapplicable).
    assert_eq!(nysiis("fuzzy"), "FASY");
}

#[test]
fn nysiis_binding_empty_string_quirk() {
    // QUIRK (architecture.md section 5.2, RULEBOOK.md G2): Python
    // `'' in 'AEIOU'` is True, so empty input takes the first-vowel branch
    // and still returns ''.
    assert_eq!(nysiis(""), "");
}

#[test]
fn nysiis_binding_digits_only_empty() {
    // Digits are stripped by the [^A-Z] filter, leaving empty -> ''.
    assert_eq!(nysiis("123"), "");
}

// ---------------------------------------------------------------------------
// Normalization (step 1): unicode-uppercase, then keep A-Z only.
// ---------------------------------------------------------------------------

#[test]
fn nysiis_case_normalization_lowercase_and_mixed() {
    // VAL-NYS-005: both forms uppercase to FUZZY -> FASY.
    assert_eq!(nysiis("Fuzzy"), "FASY");
    assert_eq!(nysiis("fUzZy"), "FASY");
}

#[test]
fn nysiis_non_alpha_stripped_wherever_they_appear() {
    // VAL-NYS-006: digits/punctuation interleaved; whitespace covered here
    // (the CLI word token cannot carry spaces).
    assert_eq!(nysiis("f1u2z3z4y"), "FASY");
    assert_eq!(nysiis("f-u-z-z-y"), "FASY");
    assert_eq!(nysiis("f u z z y"), "FASY");
    assert_eq!(nysiis("  ...  "), "");
}

#[test]
fn nysiis_sharp_s_uppercases_to_ss_and_survives() {
    // VAL-NYS-007 / QUIRK Q1: 'ß'.to_uppercase() == "SS" (ASCII), surviving
    // the A-Z filter. A lone ß becomes SS, which the trailing-S/Z strip then
    // consumes entirely -> ''. 'Straße' uppercases to STRASSE.
    assert_eq!(nysiis("ß"), "");
    assert_eq!(nysiis("Straße"), "STRAS");
    assert_eq!(nysiis("Strasse"), "STRAS");
}

#[test]
fn nysiis_accented_letters_dropped_after_uppercase() {
    // 'é' uppercases to 'É' (non-ASCII) and is dropped by the A-Z filter:
    // 'Jéroboam' filters to JROBOAM. Hand-trace: J,R,O->A,B,O->A,A,M->N
    // (not-first) -> J R A B A A N; no trailing vowel; first='J'; dup
    // collapse of AA -> 'JRABAN'.
    assert_eq!(nysiis("Jéroboam"), "JRABAN");
}

// ---------------------------------------------------------------------------
// Trailing S/Z strip (step 3) — runs BEFORE MAC/PF and the suffix loop (Q3).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_trailing_sz_strip() {
    // VAL-NYS-008: FUZZ/FUZ/FUS reduce to FU -> scan F,A -> trailing-vowel
    // trim leaves F. FUZZY's trailing Y blocks the strip.
    assert_eq!(nysiis("FUZZ"), "F");
    assert_eq!(nysiis("FUZ"), "F");
    assert_eq!(nysiis("FUS"), "F");
    assert_eq!(nysiis("FUZZY"), "FASY");
}

// ---------------------------------------------------------------------------
// Prefix handling (step 4): initial MAC -> MC (stop adjusted), PF -> start=1.
// ---------------------------------------------------------------------------

#[test]
fn nysiis_initial_mac_becomes_mc() {
    // VAL-NYS-009.
    assert_eq!(nysiis("MAC"), "MC");
    assert_eq!(nysiis("MACBETH"), "MCBATH");
}

#[test]
fn nysiis_mac_stop_adjustment_then_suffix_loop() {
    // VAL-NYS-024: MACIX -> MC + IX (stop decremented), suffix loop maps
    // IX->IC -> scanned MCIC -> MCAC.
    assert_eq!(nysiis("MACIX"), "MCAC");
}

#[test]
fn nysiis_initial_pf_drops_p() {
    // VAL-NYS-010: PF sets start=1; the P is dropped from the scanned slice
    // and the scan restarts at index 0 (Q4). PFIX also exercises the suffix
    // loop with start=1 (IX->IC).
    assert_eq!(nysiis("PFISTER"), "FASTAR");
    assert_eq!(nysiis("PFIX"), "FAC");
}

// ---------------------------------------------------------------------------
// Suffix loop (step 5): while (stop - start) > 2, map the trailing pair,
// accumulate, stop at the first unmapped pair (Q5).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_suffix_map_ix_ex() {
    // VAL-NYS-011: DIX -> D+IC -> DIC -> DAC (I->A); BEX -> BEC -> BAC.
    assert_eq!(nysiis("DIX"), "DAC");
    assert_eq!(nysiis("BEX"), "BAC");
}

#[test]
fn nysiis_suffix_map_ye_ee_ie() {
    // VAL-NYS-012: trailing pair -> Y; scanned BY (final Y kept verbatim —
    // last position, middle-Y rule inapplicable).
    assert_eq!(nysiis("BYE"), "BY");
    assert_eq!(nysiis("BEE"), "BY");
    assert_eq!(nysiis("BIE"), "BY");
}

#[test]
fn nysiis_suffix_map_dt_rt_rd_nt_nd() {
    // VAL-NYS-013: trailing pair -> D; scanned BD.
    assert_eq!(nysiis("BDT"), "BD");
    assert_eq!(nysiis("BRT"), "BD");
    assert_eq!(nysiis("BRD"), "BD");
    assert_eq!(nysiis("BNT"), "BD");
    assert_eq!(nysiis("BND"), "BD");
}

#[test]
fn nysiis_suffix_loop_chains_and_stops_at_first_unmapped_pair() {
    // VAL-NYS-014: BIXIX applies IX->IC twice (suffix ICIC, scanned BICIC);
    // BABIX applies it once, then pair AB is unmapped -> break (scanned
    // BABIC).
    assert_eq!(nysiis("BIXIX"), "BACAC");
    assert_eq!(nysiis("BABIX"), "BABAC");
}

// ---------------------------------------------------------------------------
// Transform scan (step 6/7): 3-char keys before 2-char before 1-char (Q6).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_transforms_3char_keys() {
    // VAL-NYS-015: SCH->S, GHT->GT. SCHMIDT also exercises suffix DT->D,
    // M->N (not-first), I->A. WRIGHT also exercises WR->R, I->A.
    assert_eq!(nysiis("SCHMIDT"), "SNAD");
    assert_eq!(nysiis("WRIGHT"), "RAGT");
}

#[test]
fn nysiis_transforms_2char_keys() {
    // VAL-NYS-016: KN->N, PH->F, AY->Y (HAYES: trailing-S strip, suffix
    // YE->Y, then AY->Y in the scan).
    assert_eq!(nysiis("KNIGHT"), "NAGT");
    assert_eq!(nysiis("PHILIP"), "FALAP");
    assert_eq!(nysiis("HAYES"), "HY");
}

#[test]
fn nysiis_transforms_remaining_entries() {
    // VAL-NYS-025: pins K->C, EY/IY/OY/UY/YW->Y, SH->S, and 2-char-before-
    // 1-char scan precedence (BIYB: IY matches before I->A).
    assert_eq!(nysiis("KEY"), "CY");
    assert_eq!(nysiis("BOY"), "BY");
    assert_eq!(nysiis("GUY"), "GY");
    assert_eq!(nysiis("SHB"), "SB");
    assert_eq!(nysiis("YWB"), "YB");
    assert_eq!(nysiis("BIYB"), "BYB");
}

#[test]
fn nysiis_transforms_1char_vowel_keys_at_any_position() {
    // E/I/O/U -> A are transforms (not not-first), so they apply at i == 0
    // too; K -> C likewise. At a non-first position the trailing-vowel trim
    // then removes the produced A.
    assert_eq!(nysiis("BE"), "B");
    assert_eq!(nysiis("BI"), "B");
    assert_eq!(nysiis("BO"), "B");
    assert_eq!(nysiis("BU"), "B");
    assert_eq!(nysiis("BK"), "BC");
}

// ---------------------------------------------------------------------------
// Not-first map (applies only when i > start) and middle-Y rule
// (i > start AND i < stop - 1).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_not_first_transforms() {
    // VAL-NYS-017: EV->AF (multi-char replacement, Q7), M->N, Q->G (AQA also
    // exercises trailing-vowel trim + first-vowel restore).
    assert_eq!(nysiis("DEVLIN"), "DAFLAN");
    assert_eq!(nysiis("SMITH"), "SNATH");
    assert_eq!(nysiis("AQA"), "AG");
}

#[test]
fn nysiis_not_first_z_m_q_entries() {
    // Z->S is also pinned by the fuzzy->FASY binding case; these isolate the
    // three 1-char not-first entries.
    assert_eq!(nysiis("BZB"), "BSB");
    assert_eq!(nysiis("BMB"), "BNB");
    assert_eq!(nysiis("BQB"), "BGB");
}

#[test]
fn nysiis_not_first_hw_vowel_entries() {
    // VAL-NYS-026: pins the 15 H/W-vowel not-first entries
    // (AH/EH/IH/OH/UH/AW/EW/IW/OW/UW/HA/HE/HI/HO/HU -> A).
    let words = [
        "BAHB", "BEHB", "BIHB", "BOHB", "BUHB", "BAWB", "BEWB", "BIWB", "BOWB", "BUWB", "BHAB",
        "BHEB", "BHIB", "BHOB", "BHUB",
    ];
    for word in words {
        assert_eq!(nysiis(word), "BAB", "word {word}");
    }
}

#[test]
fn nysiis_not_first_never_applies_at_position_zero() {
    // At i == start only the transforms map is consulted: initial M, Q, Z
    // are copied verbatim (no not-first M->N/Q->G/Z->S).
    assert_eq!(nysiis("MB"), "MB");
    assert_eq!(nysiis("QB"), "QB");
    assert_eq!(nysiis("ZB"), "ZB");
}

#[test]
fn nysiis_middle_y_rule_only_strictly_inside() {
    // VAL-NYS-018: Y->A only when i > start AND i < stop - 1.
    assert_eq!(nysiis("BYB"), "BAB"); // middle Y converted
    assert_eq!(nysiis("BY"), "BY"); // final Y untouched
    assert_eq!(nysiis("YB"), "YB"); // initial Y untouched
}

// ---------------------------------------------------------------------------
// Trailing-vowel trim (step 8) + first-vowel restore (step 9, quirks Q2/Q8).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_first_vowel_restore() {
    // VAL-NYS-019: the scan maps vowels to A; the restore forces output
    // position 0 back to the original first char when it is a vowel.
    assert_eq!(nysiis("EB"), "EB"); // scan AB, restored to E
    assert_eq!(nysiis("EDGE"), "EG"); // E->A, DG->G, trailing A trimmed, restore E
    assert_eq!(nysiis("IB"), "IB");
    assert_eq!(nysiis("OB"), "OB");
    assert_eq!(nysiis("UB"), "UB");
}

#[test]
fn nysiis_first_vowel_restore_all_vowels_quirk() {
    // VAL-NYS-019 quirk (Q8): AEIOU scans to AAAAA, the trailing-vowel trim
    // zeroes the length, the restore writes r[0] but the TRIM LENGTH still
    // governs the dedup slice -> ''.
    assert_eq!(nysiis("AEIOU"), "");
}

#[test]
fn nysiis_single_vowel_input_trims_to_empty() {
    // A lone vowel scans to a single vowel, trimmed to length 0; the restore
    // writes r[0] but the slice stays empty.
    assert_eq!(nysiis("A"), "");
    assert_eq!(nysiis("E"), "");
}

// ---------------------------------------------------------------------------
// Consecutive-duplicate collapse (step 10): adjacent only (Q9).
// ---------------------------------------------------------------------------

#[test]
fn nysiis_consecutive_dup_collapse_adjacent_only() {
    // VAL-NYS-020.
    assert_eq!(nysiis("BALL"), "BAL"); // LL collapsed
    assert_eq!(nysiis("BB"), "B");
    assert_eq!(nysiis("BAB"), "BAB"); // non-adjacent duplicates preserved
}

// ---------------------------------------------------------------------------
// Robustness.
// ---------------------------------------------------------------------------

#[test]
fn nysiis_very_long_input() {
    // VAL-NYS-021: 10 000 chars scan to 10 000 B's, then consecutive-dup
    // collapse yields B.
    let word = "B".repeat(10_000);
    assert_eq!(nysiis(&word), "B");
}

#[test]
fn nysiis_very_long_input_no_collapse_across_transform_boundary() {
    // Alternating B/A pairs never produce adjacent duplicates: 5 000 pairs.
    let word = "BA".repeat(5_000);
    let code = nysiis(&word);
    // Scan: B verbatim, A verbatim (A is in no map) -> BA repeated; the
    // final A is trimmed by the trailing-vowel trim.
    assert_eq!(code, "BA".repeat(4_999) + "B");
}
