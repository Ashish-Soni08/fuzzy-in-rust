//! Integration tests for the pinned fuzzy-cli batch line protocol
//! (architecture.md section 7.0), scaffold stage.
//!
//! Only malformed-line paths are asserted here: well-formed algorithm lines
//! dispatch to fuzzy-core, whose stubs panic with `unimplemented!()` until the
//! algorithm-port features land (sanctioned scaffold behavior). The ports add
//! their own dispatch assertions.

use std::io::Write;
use std::process::{Command, Stdio};

/// Run the built fuzzy-cli exe with `input` on stdin; return (exit status, raw stdout).
fn run_cli(input: &[u8]) -> (std::process::ExitStatus, Vec<u8>) {
    let mut child = Command::new(env!("CARGO_BIN_EXE_fuzzy-cli"))
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .spawn()
        .expect("spawn fuzzy-cli");
    child
        .stdin
        .as_mut()
        .expect("stdin piped")
        .write_all(input)
        .expect("write stdin");
    let output = child.wait_with_output().expect("wait for fuzzy-cli");
    (output.status, output.stdout)
}

fn stdout_lines(stdout: &[u8]) -> Vec<String> {
    String::from_utf8(stdout.to_vec())
        .expect("stdout is valid UTF-8")
        .lines()
        .map(|line| line.to_string())
        .collect()
}

#[test]
fn cli_unknown_algorithm_produces_error_line_and_exit_zero() {
    let (status, stdout) = run_cli(b"bogus 1 a\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
}

#[test]
fn cli_bad_size_produces_error_line() {
    let (status, stdout) = run_cli(b"soundex x fuzzy\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
}

#[test]
fn cli_missing_size_produces_error_line() {
    let (status, stdout) = run_cli(b"soundex\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
}

#[test]
fn cli_empty_line_produces_error_line() {
    let (status, stdout) = run_cli(b"\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
}

#[test]
fn cli_error_lines_never_abort_batch() {
    // One output line per input line, in order, and the process exits 0 even
    // though every line is malformed.
    let (status, stdout) = run_cli(b"bogus 1 a\nsoundex x fuzzy\n\ndmetaphone zzz mayer\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 4, "one output line per input line");
    for (i, line) in lines.iter().enumerate() {
        assert!(
            line.starts_with("ERROR "),
            "line {i} not an ERROR: {line:?}"
        );
    }
}

#[test]
fn cli_output_is_bomless_utf8() {
    let (_status, stdout) = run_cli(b"bogus\n");
    assert!(
        !stdout.starts_with(&[0xEF, 0xBB, 0xBF]),
        "stdout must not start with a UTF-8 BOM"
    );
}

#[test]
fn cli_stray_bom_on_first_line_is_stripped() {
    // Protocol input is BOM-less, but a stray BOM must not corrupt token 1:
    // the ERROR message names the algorithm token without the BOM.
    let (status, stdout) = run_cli(b"\xEF\xBB\xBFbogus 1 a\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(
        lines[0].contains("\"bogus\""),
        "BOM leaked into the token: {:?}",
        lines[0]
    );
}

#[test]
fn cli_crlf_input_tolerated() {
    // BufRead::lines strips the trailing CR; the ERROR text must not carry it.
    let (status, stdout) = run_cli(b"bogus 1 a\r\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 1);
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
    assert!(
        !lines[0].ends_with('\r'),
        "CR leaked into the line: {:?}",
        lines[0]
    );
}

// ---------------------------------------------------------------------------
// soundex dispatch (soundex-port feature): real outputs through the exe.
// ---------------------------------------------------------------------------

#[test]
fn cli_soundex_binding_datapoints() {
    // architecture.md section 5.1 binding data points via the pinned line
    // protocol. Input is written as raw UTF-8 bytes (Jéroboam), so console
    // encoding cannot corrupt the non-ASCII case. A missing word token is
    // the empty string (-> 0000).
    let (status, stdout) =
        run_cli("soundex 4 fuzzy\nsoundex 4 FancyFree\nsoundex 8 Test\nsoundex 8 Jéroboam\nsoundex 4\nsoundex 4 123\n".as_bytes());
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["F200", "F521", "T23", "J615", "0000", "0000"]);
}

#[test]
fn cli_soundex_size_zero_prints_empty_line() {
    let (status, stdout) = run_cli(b"soundex 0 anything\nsoundex 4 fuzzy\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["", "F200"]);
}

#[test]
fn cli_soundex_error_lines_do_not_abort_batch() {
    // VAL-SDX-023: bad size and unknown algorithm yield ERROR lines; the
    // batch continues and the process still exits 0.
    let (status, stdout) = run_cli(b"soundex x fuzzy\nbogus 1 a\nsoundex 4 fuzzy\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 3, "one output line per input line");
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
    assert!(lines[1].starts_with("ERROR "), "got: {:?}", lines[1]);
    assert_eq!(lines[2], "F200");
}

// ---------------------------------------------------------------------------
// nysiis dispatch (nysiis-port feature): real outputs through the exe.
// ---------------------------------------------------------------------------

#[test]
fn cli_nysiis_binding_datapoints() {
    // architecture.md section 5.2 binding data points via the pinned line
    // protocol. A missing word token is the empty string; digits strip to
    // empty. Both empty results print as empty lines.
    let (status, stdout) = run_cli(b"nysiis fuzzy\nnysiis\nnysiis 123\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["FASY", "", ""]);
}

#[test]
fn cli_nysiis_quirk_cases() {
    // ß -> SS survives the A-Z filter (written as raw UTF-8 bytes so console
    // encoding cannot corrupt it); MAC/PF prefixes; the all-vowels
    // trailing-trim quirk prints an empty line.
    let (status, stdout) =
        run_cli("nysiis Straße\nnysiis MACBETH\nnysiis PFISTER\nnysiis AEIOU\n".as_bytes());
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["STRAS", "MCBATH", "FASTAR", ""]);
}

// ---------------------------------------------------------------------------
// dmetaphone dispatch (dmetaphone-port feature): real outputs through the
// exe. Protocol: `dmetaphone <size> <word>` -> `<primary>|<secondary>` with
// `-` for a None code (architecture.md section 7.0). All expectations are
// C-oracle-verified raw codes with wrapper semantics applied.
// ---------------------------------------------------------------------------

#[test]
fn cli_dmetaphone_binding_datapoints() {
    // VAL-DM-003: mayer -> MR|-, fuzzy -> FS|-, missing word (empty) -> -|-.
    let (status, stdout) = run_cli(b"dmetaphone 0 mayer\ndmetaphone 0 fuzzy\ndmetaphone 0\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["MR|-", "FS|-", "-|-"]);
}

#[test]
fn cli_dmetaphone_initial_letter_rules() {
    // VAL-DM-004: GN/KN/PN/WR/PS skip, initial X -> S.
    let (status, stdout) = run_cli(
        b"dmetaphone 0 gnome\ndmetaphone 0 knee\ndmetaphone 0 pneumonia\ndmetaphone 0 write\ndmetaphone 0 psalm\ndmetaphone 0 xenon\n",
    );
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(
        lines,
        vec!["NM|-", "N|-", "NMN|-", "RT|-", "SLM|-", "SNN|-"]
    );
}

#[test]
fn cli_dmetaphone_cap_and_size_truncation() {
    // VAL-DM-005: oracle-verified bcdfgh -> raw PKFK|PKFK (the Pierce-rule
    // else swallows the D); size 0 unlimited, 1-3 truncate.
    let (status, stdout) = run_cli(
        b"dmetaphone 0 bcdfgh\ndmetaphone 3 bcdfgh\ndmetaphone 2 bcdfgh\ndmetaphone 1 mayer\ndmetaphone 2 mayer\n",
    );
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["PKFK|-", "PKF|-", "PK|-", "M|-", "MR|-"]);
}

#[test]
fn cli_dmetaphone_collapse_vs_distinct_secondary() {
    // VAL-DM-006: equal codes collapse to `-`; czerny keeps its distinct
    // secondary (CZ arm: primary S, secondary X).
    let (status, stdout) = run_cli(b"dmetaphone 0 mayer\ndmetaphone 0 czerny\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["MR|-", "SRN|XRN"]);
}

#[test]
fn cli_dmetaphone_slavogermanic_arms() {
    // VAL-DM-007: WITZ arm, final-W-after-vowel, GLI non-SlavoGermanic.
    let (status, stdout) =
        run_cli(b"dmetaphone 0 horowitz\ndmetaphone 0 arnow\ndmetaphone 0 tagliaro\n");
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["HRTS|HRFX", "ARN|ARNF", "TKLR|TLR"]);
}

#[test]
fn cli_dmetaphone_structural_semantics() {
    // VAL-DM-013 (OR-loop keeps secondary growing), VAL-DM-014 (padding
    // lookahead), VAL-DM-015 (collapse before truncate), VAL-DM-016
    // (distinct secondary truncation), VAL-DM-017 (default arm skips digits).
    let (status, stdout) = run_cli(
        b"dmetaphone 0 tagliarb\ndmetaphone 0 ach\ndmetaphone 1 bier\ndmetaphone 2 czerny\ndmetaphone 0 123\n",
    );
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines, vec!["TKLR|TLRP", "AK|-", "P|P", "SR|XR", "-|-"]);
}

#[test]
fn cli_dmetaphone_non_ascii_line_errors_and_batch_continues() {
    // VAL-DM-018: a non-ASCII dmetaphone line yields an ERROR line; the
    // batch continues and the process still exits 0. Input written as raw
    // UTF-8 bytes so console encoding cannot corrupt it.
    let (status, stdout) = run_cli("dmetaphone 0 Jéroboam\ndmetaphone 0 mayer\n".as_bytes());
    assert!(status.success());
    let lines = stdout_lines(&stdout);
    assert_eq!(lines.len(), 2, "one output line per input line");
    assert!(lines[0].starts_with("ERROR "), "got: {:?}", lines[0]);
    assert_eq!(lines[1], "MR|-");
}
