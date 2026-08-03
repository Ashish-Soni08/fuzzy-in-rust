//! Native integration tests for the Double Metaphone port (exact port of
//! `src/double_metaphone.c`, architecture.md section 5.3 / RULEBOOK.md
//! section 6).
//!
//! EVERY expected value in this file was verified against the compiled
//! original C oracle (`tools/oracle-c/dmoracle.exe`, ground truth per
//! architecture.md section 5.3) on 2026-08-03 — including the non-obvious
//! ones (`bcdfgh` -> `PKFK` via the Pierce-rule `current += 2` swallowing the
//! D; `science` -> `SKNK` via the SC-arm fall-through; `thumb` -> `0M|TM`
//! with the digit-zero character; `witz` -> `ATS|FFX` via the W arm firing
//! twice). The oracle prints raw `primary|secondary`; tests through
//! `dmetaphone`/`dmetaphone_with_size` apply the wrapper semantics
//! (collapse-equal, empty->None, size truncation) on top.
//!
//! Naming conventions (contract-filterable): the section 5.3 binding data
//! points contain `binding`; non-ASCII rejection tests contain `non_ascii`;
//! Latin-1 byte-arm tests contain `latin1`.

use fuzzy_core::{dmetaphone, dmetaphone_bytes, dmetaphone_with_size};

/// Owned `Some(code)` expectation from a byte string.
fn some(code: &[u8]) -> Option<Vec<u8>> {
    Some(code.to_vec())
}

/// Owned raw `(primary, secondary)` expectation from byte strings.
fn raw(primary: &[u8], secondary: &[u8]) -> (Vec<u8>, Vec<u8>) {
    (primary.to_vec(), secondary.to_vec())
}

// ---------------------------------------------------------------------------
// Binding data points (architecture.md section 5.3 — all three).
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_binding_mayer() {
    // Original test suite data point: DMetaphone()('mayer') == [b'MR', None].
    assert_eq!(dmetaphone("mayer"), Ok((some(b"MR"), None)));
}

#[test]
fn dmetaphone_binding_fuzzy() {
    // README data point: DMetaphone()('fuzzy') == [b'FS', None].
    assert_eq!(dmetaphone("fuzzy"), Ok((some(b"FS"), None)));
}

#[test]
fn dmetaphone_binding_empty_string() {
    // DMetaphone()('') == [None, None]: both codes empty -> None.
    assert_eq!(dmetaphone(""), Ok((None, None)));
    assert_eq!(dmetaphone_bytes(b""), raw(b"", b""));
}

// ---------------------------------------------------------------------------
// Initial-letter rules (RULEBOOK D3): GN/KN/PN/WR/PS skip, initial X -> S.
// Oracle: gnome NM|NM, knee N|N, pneumonia NMN|NMN, write RT|RT,
// psalm SLM|SLM, xenon SNN|SNN.
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_initial_gn_skip_gnome() {
    assert_eq!(dmetaphone("gnome"), Ok((some(b"NM"), None)));
}

#[test]
fn dmetaphone_initial_kn_skip_knee() {
    assert_eq!(dmetaphone("knee"), Ok((some(b"N"), None)));
}

#[test]
fn dmetaphone_initial_pn_skip_pneumonia() {
    assert_eq!(dmetaphone("pneumonia"), Ok((some(b"NMN"), None)));
}

#[test]
fn dmetaphone_initial_wr_skip_write() {
    assert_eq!(dmetaphone("write"), Ok((some(b"RT"), None)));
}

#[test]
fn dmetaphone_initial_ps_skip_psalm() {
    assert_eq!(dmetaphone("psalm"), Ok((some(b"SLM"), None)));
}

#[test]
fn dmetaphone_initial_x_maps_to_s_xenon() {
    assert_eq!(dmetaphone("xenon"), Ok((some(b"SNN"), None)));
}

// ---------------------------------------------------------------------------
// 4-char cap and size truncation (VAL-DM-005). Oracle: bcdfgh -> PKFK|PKFK
// raw — the C arm's Pierce-rule else does `current += 2` for the lone C,
// swallowing the D (an earlier hand-trace saying PKTF was WRONG; the oracle
// is ground truth). wasserman -> ASRMN|FSRMN before the cap.
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_cap_bcdfgh_raw_pkfk() {
    assert_eq!(dmetaphone_bytes(b"bcdfgh"), raw(b"PKFK", b"PKFK"));
}

#[test]
fn dmetaphone_cap_truncates_to_four_wasserman() {
    // Raw loop output would be ASRMN|FSRMN; the C caps both at 4 chars.
    assert_eq!(dmetaphone_bytes(b"wasserman"), raw(b"ASRM", b"FSRM"));
}

#[test]
fn dmetaphone_size_zero_means_unlimited_bcdfgh() {
    assert_eq!(dmetaphone_with_size(0, "bcdfgh"), Ok((some(b"PKFK"), None)));
}

#[test]
fn dmetaphone_size_three_truncates_bcdfgh() {
    assert_eq!(dmetaphone_with_size(3, "bcdfgh"), Ok((some(b"PKF"), None)));
}

#[test]
fn dmetaphone_size_two_truncates_bcdfgh() {
    assert_eq!(dmetaphone_with_size(2, "bcdfgh"), Ok((some(b"PK"), None)));
}

#[test]
fn dmetaphone_size_one_truncates_mayer() {
    assert_eq!(dmetaphone_with_size(1, "mayer"), Ok((some(b"M"), None)));
}

#[test]
fn dmetaphone_size_two_leaves_mayer_untruncated() {
    assert_eq!(dmetaphone_with_size(2, "mayer"), Ok((some(b"MR"), None)));
}

#[test]
fn dmetaphone_size_larger_than_code_leaves_it_unchanged() {
    assert_eq!(dmetaphone_with_size(100, "mayer"), Ok((some(b"MR"), None)));
}

// ---------------------------------------------------------------------------
// Wrapper semantics (RULEBOOK 6.3): primary == secondary collapses the
// secondary to None; distinct secondaries pass through.
// Oracle: mayer MR|MR, czerny SRN|XRN.
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_collapse_equal_secondary_to_none() {
    // Raw codes are both MR; the wrapper collapses the secondary.
    assert_eq!(dmetaphone_bytes(b"mayer"), raw(b"MR", b"MR"));
    assert_eq!(dmetaphone("mayer"), Ok((some(b"MR"), None)));
}

#[test]
fn dmetaphone_distinct_secondary_passthrough_czerny() {
    // CZ arm: primary S, secondary X.
    assert_eq!(dmetaphone("czerny"), Ok((some(b"SRN"), some(b"XRN"))));
}

#[test]
fn dmetaphone_distinct_secondary_truncation_czerny_size_two() {
    // VAL-DM-016: truncation applies to both distinct codes.
    assert_eq!(
        dmetaphone_with_size(2, "czerny"),
        Ok((some(b"SR"), some(b"XR")))
    );
}

#[test]
fn dmetaphone_collapse_before_truncate_bier() {
    // VAL-DM-015 / RULEBOOK W3: raw P|PR differ, so NO collapse; truncating
    // both to size 1 yields P|P. A truncate-then-collapse port would wrongly
    // yield P|-.
    assert_eq!(dmetaphone_bytes(b"bier"), raw(b"P", b"PR"));
    assert_eq!(
        dmetaphone_with_size(1, "bier"),
        Ok((some(b"P"), some(b"P")))
    );
}

// ---------------------------------------------------------------------------
// SlavoGermanic-sensitive arms (VAL-DM-007). Oracle: horowitz HRTS|HRFX
// (WITZ arm), arnow ARN|ARNF (final W after vowel), tagliaro TKLR|TLR
// (GLI arm with SlavoGermanic false).
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_slavogermanic_horowitz_witz() {
    assert_eq!(dmetaphone("horowitz"), Ok((some(b"HRTS"), some(b"HRFX"))));
}

#[test]
fn dmetaphone_slavogermanic_arnow_final_w() {
    assert_eq!(dmetaphone("arnow"), Ok((some(b"ARN"), some(b"ARNF"))));
}

#[test]
fn dmetaphone_slavogermanic_tagliaro_gli() {
    assert_eq!(dmetaphone("tagliaro"), Ok((some(b"TKLR"), some(b"TLR"))));
}

// ---------------------------------------------------------------------------
// Load-bearing structural semantics (RULEBOOK D1/D4).
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_or_loop_tagliarb() {
    // VAL-DM-013: the `while (primary < 4 || secondary < 4)` OR-condition
    // keeps the loop alive for the secondary after the primary reaches 4.
    // An AND-rewrite would yield TKLR|TLR.
    assert_eq!(dmetaphone("tagliarb"), Ok((some(b"TKLR"), some(b"TLRP"))));
}

#[test]
fn dmetaphone_padding_lookahead_ach() {
    // VAL-DM-014: the CH arm's lookahead list contains " ", which matches a
    // padding space at end of word; a port without padding yields AX|AK.
    assert_eq!(dmetaphone("ach"), Ok((some(b"AK"), None)));
}

#[test]
fn dmetaphone_default_arm_skips_digits_and_punctuation() {
    // VAL-DM-017: unmatched bytes (digits, punctuation) are skipped silently.
    assert_eq!(dmetaphone("123"), Ok((None, None)));
    assert_eq!(dmetaphone_bytes(b"123"), raw(b"", b""));
}

// ---------------------------------------------------------------------------
// Non-ASCII rejection (original DM behavior preserved, RULEBOOK section 7):
// Err(NonAsciiError) carrying the offending character and its byte position.
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_non_ascii_rejected_with_char_and_position() {
    let err = dmetaphone("Jéroboam").expect_err("non-ASCII must be rejected");
    assert_eq!(err.character(), 'é');
    assert_eq!(err.byte_position(), 1);
}

#[test]
fn dmetaphone_non_ascii_with_size_rejected() {
    let err = dmetaphone_with_size(4, "Jéroboam").expect_err("non-ASCII must be rejected");
    assert_eq!(err.character(), 'é');
    assert_eq!(err.byte_position(), 1);
}

#[test]
fn dmetaphone_non_ascii_byte_position_after_multibyte_prefix() {
    // The position is a BYTE offset into the UTF-8 input.
    let err = dmetaphone("abc€x").expect_err("non-ASCII must be rejected");
    assert_eq!(err.character(), '€');
    assert_eq!(err.byte_position(), 3);
}

// ---------------------------------------------------------------------------
// Latin-1 byte arms (gap G1, VAL-DM-009): case 0xC7 ('Ç' -> S) and case
// 0xD1 ('Ñ' -> N), reachable only through the raw bytes API. Oracle-verified
// by feeding raw bytes to dmoracle.exe: [C7] -> S|S, [D1] -> N|N,
// [C7,'A','T'] -> ST|ST, ['B',D1] -> PN|PN, [E7] -> |, [F1] -> |.
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_latin1_c_cedilla_byte_maps_to_s() {
    assert_eq!(dmetaphone_bytes(&[0xC7]), raw(b"S", b"S"));
}

#[test]
fn dmetaphone_latin1_n_tilde_byte_maps_to_n() {
    assert_eq!(dmetaphone_bytes(&[0xD1]), raw(b"N", b"N"));
}

#[test]
fn dmetaphone_latin1_c_cedilla_in_context() {
    // Ç -> S; the interior vowel is not at position 0 so it is skipped; T -> T.
    assert_eq!(dmetaphone_bytes(&[0xC7, b'A', b'T']), raw(b"ST", b"ST"));
}

#[test]
fn dmetaphone_latin1_n_tilde_in_context() {
    assert_eq!(dmetaphone_bytes(&[b'B', 0xD1]), raw(b"PN", b"PN"));
}

#[test]
fn dmetaphone_latin1_lowercase_bytes_hit_no_arm() {
    // MakeUpper is byte-wise ASCII-only (MSVC toupper passes high bytes
    // through unchanged, RULEBOOK G8): lowercase ç/ñ match no arm and are
    // skipped by the default arm.
    assert_eq!(dmetaphone_bytes(&[0xE7]), raw(b"", b""));
    assert_eq!(dmetaphone_bytes(&[0xF1]), raw(b"", b""));
}

// ---------------------------------------------------------------------------
// Arm-by-arm coverage — every expectation below verified against the C
// oracle (dmoracle.exe raw output; wrapper semantics applied where the
// &str API is used).
// ---------------------------------------------------------------------------

#[test]
fn dmetaphone_caesar_special_case() {
    assert_eq!(dmetaphone("caesar"), Ok((some(b"SSR"), None)));
}

#[test]
fn dmetaphone_chianti_italian() {
    assert_eq!(dmetaphone("chianti"), Ok((some(b"KNT"), None)));
}

#[test]
fn dmetaphone_michael_chae() {
    // CHAE arm: primary K, secondary X.
    assert_eq!(dmetaphone("michael"), Ok((some(b"MKL"), some(b"MXL"))));
}

#[test]
fn dmetaphone_chemistry_greek_root() {
    assert_eq!(dmetaphone("chemistry"), Ok((some(b"KMST"), None)));
}

#[test]
fn dmetaphone_chore_excluded_from_greek_root() {
    // The CHORE exclusion keeps the greek-root rule from firing.
    assert_eq!(dmetaphone("chore"), Ok((some(b"XR"), None)));
}

#[test]
fn dmetaphone_school_dutch_sch() {
    assert_eq!(dmetaphone("school"), Ok((some(b"SKL"), None)));
}

#[test]
fn dmetaphone_schermerhorn_sch_er() {
    // SCH+ER: primary X, secondary SK.
    assert_eq!(
        dmetaphone("schermerhorn"),
        Ok((some(b"XRMR"), some(b"SKRM")))
    );
}

#[test]
fn dmetaphone_science_sc_non_h_fallthrough() {
    // QUIRK (dead code in the C): the SC arm's I/E/Y and SK handlers sit
    // inside the `next == 'H'` block after an if/else whose branches both
    // break, so SC + non-H falls THROUGH to the generic S path. Oracle:
    // science -> SKNK|SKNK (a port of the "intended" structure gives SNS).
    assert_eq!(dmetaphone("science"), Ok((some(b"SKNK"), None)));
    assert_eq!(dmetaphone("scene"), Ok((some(b"SKN"), None)));
}

#[test]
fn dmetaphone_thumb_th_zero_char() {
    // QUIRK: the TH arm emits the DIGIT CHARACTER '0' for the primary
    // ("yes, zero" per the C comment). Oracle: thumb -> 0M|TM.
    assert_eq!(dmetaphone("thumb"), Ok((some(b"0M"), some(b"TM"))));
}

#[test]
fn dmetaphone_thomas_th_exception() {
    assert_eq!(dmetaphone("thomas"), Ok((some(b"TMS"), None)));
}

#[test]
fn dmetaphone_edge_dg_soft() {
    assert_eq!(dmetaphone("edge"), Ok((some(b"AJ"), None)));
}

#[test]
fn dmetaphone_edgar_dg_hard() {
    assert_eq!(dmetaphone("edgar"), Ok((some(b"ATKR"), None)));
}

#[test]
fn dmetaphone_dumb_mb() {
    assert_eq!(dmetaphone("dumb"), Ok((some(b"TM"), None)));
}

#[test]
fn dmetaphone_sugar_special_case() {
    assert_eq!(dmetaphone("sugar"), Ok((some(b"XKR"), some(b"SKR"))));
}

#[test]
fn dmetaphone_smith_initial_s_m() {
    // S + M/N/L/W at start: primary S, secondary X; then TH -> 0/T.
    assert_eq!(dmetaphone("smith"), Ok((some(b"SM0"), some(b"XMT"))));
}

#[test]
fn dmetaphone_schmidt_matches_smith_family() {
    assert_eq!(dmetaphone("schmidt"), Ok((some(b"XMT"), some(b"SMT"))));
}

#[test]
fn dmetaphone_witz_initial_w_fires_twice() {
    // QUIRK: the W arm's initial-vowel rule (A/F) has no break, so the
    // WICZ/WITZ rule (TS/FX) fires on top. Oracle: witz -> ATS|FFX.
    assert_eq!(dmetaphone("witz"), Ok((some(b"ATS"), some(b"FFX"))));
}

#[test]
fn dmetaphone_agnes_gn_after_initial_vowel() {
    // G-arm GN rule with current == 1 and a vowel at 0: primary KN, secondary N.
    assert_eq!(dmetaphone("agnes"), Ok((some(b"AKNS"), some(b"ANS"))));
}

#[test]
fn dmetaphone_cagney_gn_ey_exception() {
    // The "not e.g. 'cagney'" guard: EY after GN forces KN/KN.
    assert_eq!(dmetaphone("cagney"), Ok((some(b"KKN"), None)));
}

#[test]
fn dmetaphone_xavier_french_final_r() {
    // Initial X -> S; final R after IE drops from the primary.
    assert_eq!(dmetaphone("xavier"), Ok((some(b"SF"), some(b"SFR"))));
}

#[test]
fn dmetaphone_rogier_french_final_r() {
    assert_eq!(dmetaphone("rogier"), Ok((some(b"RJ"), some(b"RJR"))));
}

#[test]
fn dmetaphone_hochmeier_french_r_excluded() {
    // The ME/MA lookbehind excludes 'hochmeier' from the french-R rule.
    assert_eq!(dmetaphone("hochmeier"), Ok((some(b"HKMR"), None)));
}

#[test]
fn dmetaphone_resnais_french_final_s() {
    assert_eq!(dmetaphone("resnais"), Ok((some(b"RSN"), some(b"RSNS"))));
}

#[test]
fn dmetaphone_cabrillo_spanish_ll() {
    // Spanish LL: primary L only.
    assert_eq!(dmetaphone("cabrillo"), Ok((some(b"KPRL"), some(b"KPR"))));
}

#[test]
fn dmetaphone_gallegos_spanish_ll() {
    assert_eq!(dmetaphone("gallegos"), Ok((some(b"KLKS"), some(b"KKS"))));
}

#[test]
fn dmetaphone_filipowicz_wicz() {
    // WICZ arm: TS/FX, then the 4-char cap.
    assert_eq!(dmetaphone("filipowicz"), Ok((some(b"FLPT"), some(b"FLPF"))));
}

#[test]
fn dmetaphone_wasserman_initial_w_vowel() {
    // Initial W before a vowel: primary A, secondary F.
    assert_eq!(dmetaphone("wasserman"), Ok((some(b"ASRM"), some(b"FSRM"))));
}

#[test]
fn dmetaphone_breaux_french_x() {
    // Final X after EAU is silent in both codes.
    assert_eq!(dmetaphone("breaux"), Ok((some(b"PR"), None)));
}

#[test]
fn dmetaphone_zhao_pinyin() {
    // ZH -> J.
    assert_eq!(dmetaphone("zhao"), Ok((some(b"J"), None)));
}

#[test]
fn dmetaphone_jose_spanish() {
    assert_eq!(dmetaphone("jose"), Ok((some(b"HS"), None)));
}

#[test]
fn dmetaphone_bajador_spanish_j() {
    assert_eq!(dmetaphone("bajador"), Ok((some(b"PJTR"), some(b"PHTR"))));
}

#[test]
fn dmetaphone_pizza_zi() {
    // Z + ZO/ZI/ZA: primary S, secondary TS.
    assert_eq!(dmetaphone("pizza"), Ok((some(b"PS"), some(b"PTS"))));
}

#[test]
fn dmetaphone_schwarz_slavogermanic_z() {
    assert_eq!(dmetaphone("schwarz"), Ok((some(b"XRS"), some(b"XFRT"))));
}

#[test]
fn dmetaphone_aachen_ch_after_vowel() {
    // CH with a preceding vowel but no L/R/N/... after: primary X, secondary K.
    assert_eq!(dmetaphone("aachen"), Ok((some(b"AXN"), some(b"AKN"))));
}

#[test]
fn dmetaphone_mchugh_mc() {
    assert_eq!(dmetaphone("mchugh"), Ok((some(b"MK"), None)));
}

#[test]
fn dmetaphone_mcclellan_double_c_excluded() {
    // CC at current == 1 after M is excluded from the double-C rule.
    assert_eq!(dmetaphone("mcclellan"), Ok((some(b"MKLL"), None)));
}

#[test]
fn dmetaphone_bacchus_cc_hu_fallthrough() {
    // CC + HU fails the inner rule and falls through to the generic C path.
    assert_eq!(dmetaphone("bacchus"), Ok((some(b"PKS"), None)));
}

#[test]
fn dmetaphone_bellocchio_cc_soft() {
    assert_eq!(dmetaphone("bellocchio"), Ok((some(b"PLX"), None)));
}

#[test]
fn dmetaphone_accident_accede_ks() {
    assert_eq!(dmetaphone("accident"), Ok((some(b"AKST"), None)));
    assert_eq!(dmetaphone("accede"), Ok((some(b"AKST"), None)));
}

#[test]
fn dmetaphone_bacci_italian() {
    assert_eq!(dmetaphone("bacci"), Ok((some(b"PX"), None)));
}

#[test]
fn dmetaphone_focaccia_cia() {
    assert_eq!(dmetaphone("focaccia"), Ok((some(b"FKX"), None)));
}

#[test]
fn dmetaphone_nation_tion() {
    assert_eq!(dmetaphone("nation"), Ok((some(b"NXN"), None)));
}

#[test]
fn dmetaphone_dutch_tch() {
    assert_eq!(dmetaphone("dutch"), Ok((some(b"TX"), None)));
}

#[test]
fn dmetaphone_gnocchi() {
    assert_eq!(dmetaphone("gnocchi"), Ok((some(b"NX"), None)));
}

#[test]
fn dmetaphone_ghiradelli_initial_gh_i() {
    // Initial GH + I -> J.
    assert_eq!(dmetaphone("ghiradelli"), Ok((some(b"JRTL"), None)));
}

#[test]
fn dmetaphone_hugh_parker_rule() {
    // GH after H at start (Parker's rule): GH silent.
    assert_eq!(dmetaphone("hugh"), Ok((some(b"H"), None)));
}

#[test]
fn dmetaphone_bough_parker_rule() {
    assert_eq!(dmetaphone("bough"), Ok((some(b"P"), None)));
}

#[test]
fn dmetaphone_laugh_gh_f() {
    assert_eq!(dmetaphone("laugh"), Ok((some(b"LF"), None)));
}

#[test]
fn dmetaphone_cough_gh_f() {
    assert_eq!(dmetaphone("cough"), Ok((some(b"KF"), None)));
}

#[test]
fn dmetaphone_danger_ger_exception() {
    // DANGER/RANGER/MANGER are excluded from the soft -ger- rule.
    assert_eq!(dmetaphone("danger"), Ok((some(b"TNJR"), some(b"TNKR"))));
}

#[test]
fn dmetaphone_biaggi_italian_g() {
    assert_eq!(dmetaphone("biaggi"), Ok((some(b"PJ"), some(b"PK"))));
}

#[test]
fn dmetaphone_giuseppe() {
    assert_eq!(dmetaphone("giuseppe"), Ok((some(b"JSP"), some(b"KSP"))));
}

#[test]
fn dmetaphone_jackson() {
    assert_eq!(dmetaphone("jackson"), Ok((some(b"JKSN"), some(b"AKSN"))));
}

#[test]
fn dmetaphone_island_isl_skip() {
    // ISL: the S is silent.
    assert_eq!(dmetaphone("island"), Ok((some(b"ALNT"), None)));
}

#[test]
fn dmetaphone_knight_kn_skip_gh_silent() {
    assert_eq!(dmetaphone("knight"), Ok((some(b"NT"), None)));
}

#[test]
fn dmetaphone_single_initial_x() {
    assert_eq!(dmetaphone("x"), Ok((some(b"S"), None)));
}

#[test]
fn dmetaphone_sch_initial_raw() {
    // SCH at start with a non-vowel, non-W at index 3 (padding space):
    // primary X, secondary S.
    assert_eq!(dmetaphone_bytes(b"sch"), raw(b"X", b"S"));
}

// ---------------------------------------------------------------------------
// Data-driven curated vectors (VAL-DM-010): every entry of
// tools/vectors/dmetaphone_vectors.json is checked against the port. The
// file holds ORACLE-RAW codes (empty string = empty code), each validated
// against the C oracle when the file was curated; this test applies the
// wrapper semantics (primary == secondary -> secondary None; empty -> None)
// on top, exactly as the &str entry point does. Parsed with a minimal
// std-only JSON extractor (no serde, per the zero-dependency rule).
// ---------------------------------------------------------------------------

/// One entry of the pinned vectors schema: `{"word","primary","secondary"}`.
struct DmVector {
    word: String,
    primary: String,
    secondary: String,
}

/// Parse a JSON string literal starting at `bytes[*i] == b'"'`, advancing
/// `*i` past the closing quote. Handles the standard escapes; sufficient for
/// the ASCII-only vectors file.
fn parse_json_string(bytes: &[u8], i: &mut usize) -> String {
    assert_eq!(bytes[*i], b'"', "expected string literal");
    *i += 1;
    let mut out = String::new();
    while bytes[*i] != b'"' {
        if bytes[*i] == b'\\' {
            *i += 1;
            match bytes[*i] {
                b'n' => out.push('\n'),
                b't' => out.push('\t'),
                b'r' => out.push('\r'),
                b'u' => {
                    let hex =
                        std::str::from_utf8(&bytes[*i + 1..*i + 5]).expect("valid \\u escape");
                    let code = u32::from_str_radix(hex, 16).expect("valid \\u hex");
                    out.push(char::from_u32(code).expect("valid unicode scalar"));
                    *i += 4;
                }
                other => out.push(other as char),
            }
        } else {
            out.push(bytes[*i] as char);
        }
        *i += 1;
    }
    *i += 1;
    out
}

/// Skip JSON insignificant whitespace.
fn skip_ws(bytes: &[u8], i: &mut usize) {
    while *i < bytes.len() && matches!(bytes[*i], b' ' | b'\t' | b'\r' | b'\n') {
        *i += 1;
    }
}

/// Extract every `{"word","primary","secondary"}` triple from the pinned
/// vectors file. Scans for `"key" : "string-value"` pairs; `word` starts a
/// new vector, `primary`/`secondary` attach to the most recent one. Keys
/// inside `_meta` are ignored because they never match these three names.
fn load_vectors() -> Vec<DmVector> {
    let text = include_str!("../../../tools/vectors/dmetaphone_vectors.json");
    let bytes = text.as_bytes();
    let mut vectors: Vec<DmVector> = Vec::new();
    let mut i = 0;
    while i < bytes.len() {
        if bytes[i] != b'"' {
            i += 1;
            continue;
        }
        let mut j = i;
        let key = parse_json_string(bytes, &mut j);
        skip_ws(bytes, &mut j);
        if j < bytes.len() && bytes[j] == b':' {
            j += 1;
            skip_ws(bytes, &mut j);
            if j < bytes.len() && bytes[j] == b'"' {
                let value = parse_json_string(bytes, &mut j);
                match key.as_str() {
                    "word" => vectors.push(DmVector {
                        word: value,
                        primary: String::new(),
                        secondary: String::new(),
                    }),
                    "primary" => {
                        if let Some(v) = vectors.last_mut() {
                            v.primary = value;
                        }
                    }
                    "secondary" => {
                        if let Some(v) = vectors.last_mut() {
                            v.secondary = value;
                        }
                    }
                    _ => {}
                }
            }
        }
        i = j;
    }
    vectors
}

#[test]
fn dmetaphone_vectors_file_matches_port() {
    let vectors = load_vectors();
    assert!(
        vectors.len() >= 50,
        "pinned schema requires >= 50 curated vectors, parsed {}",
        vectors.len()
    );
    for v in &vectors {
        // Wrapper semantics (architecture.md section 5.3 / RULEBOOK 6.3):
        // primary == secondary collapses the secondary to None; an empty
        // code becomes None. The file holds oracle-RAW codes.
        let expected_primary = if v.primary.is_empty() {
            None
        } else {
            Some(v.primary.as_bytes().to_vec())
        };
        let expected_secondary = if v.secondary == v.primary || v.secondary.is_empty() {
            None
        } else {
            Some(v.secondary.as_bytes().to_vec())
        };
        assert_eq!(
            dmetaphone(&v.word),
            Ok((expected_primary, expected_secondary)),
            "vector mismatch for word {:?} (oracle-raw {}|{})",
            v.word,
            v.primary,
            v.secondary
        );
    }
}
