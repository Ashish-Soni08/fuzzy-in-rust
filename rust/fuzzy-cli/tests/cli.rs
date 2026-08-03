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
