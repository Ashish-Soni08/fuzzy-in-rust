#![forbid(unsafe_code)]

//! fuzzy-cli: batch line-protocol front-end over the fuzzy-core port.
//!
//! PINNED protocol (architecture.md section 7.0): reads UTF-8 BOM-less lines
//! on stdin, one output line per input line:
//!   `soundex <size> <word>`    -> the code (empty code = empty line)
//!   `nysiis <word>`            -> the code (empty code = empty line)
//!   `dmetaphone <size> <word>` -> `<primary>|<secondary>`, `-` for a None code
//! A MISSING word token means the empty string. Malformed lines (unknown
//! algorithm, unparseable/missing size, non-ASCII input to dmetaphone) print
//! `ERROR <message>` and never abort the batch. Output is UTF-8 BOM-less.
//!
//! fuzzy-core is source-included via the #[path] module below rather than a
//! Cargo dependency: the mission contract (VAL-REPO-013) pins EMPTY
//! [dependencies] sections for BOTH crates (std-only, no external crates),
//! and a path dependency would still be a [dependencies] entry. Including the
//! source dispatches to the exact same fuzzy-core functions.

// The CLI uses only part of the fuzzy-core API (dmetaphone_with_size, not the
// raw-bytes/unlimited entry points). Compiled as a lib crate those items are
// public API; inside this private module they would trip dead_code lints, so
// scope an allow to the include.
#[allow(dead_code, unused_imports)]
#[path = "../../fuzzy-core/src/lib.rs"]
mod fuzzy_core;

use std::io::{self, BufRead, Write};

fn main() {
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut out = io::BufWriter::new(stdout.lock());

    let mut first_line = true;
    for line in stdin.lock().lines() {
        let response = match line {
            Ok(mut line) => {
                // The protocol is BOM-less, but a stray BOM on the first line
                // (e.g. a console pipe with a BOM-emitting encoding) must not
                // corrupt the first token.
                if first_line && line.starts_with('\u{FEFF}') {
                    line.remove(0);
                }
                first_line = false;
                handle_line(&line)
            }
            // Invalid UTF-8 (or other read error): report, keep the batch going.
            Err(err) => format!("ERROR unreadable input line: {err}"),
        };
        // A closed stdout must not crash the batch either.
        let _ = writeln!(out, "{response}");
    }
    let _ = out.flush();
}

/// Handle one input line and produce its single output line.
fn handle_line(line: &str) -> String {
    let mut tokens = line.split_whitespace();
    let Some(algo) = tokens.next() else {
        return "ERROR empty line (expected: soundex|nysiis|dmetaphone ...)".to_string();
    };
    match algo {
        "soundex" => match parse_size(&mut tokens) {
            Ok(size) => fuzzy_core::soundex(size, tokens.next().unwrap_or("")),
            Err(err) => err,
        },
        "nysiis" => fuzzy_core::nysiis(tokens.next().unwrap_or("")),
        "dmetaphone" => match parse_size(&mut tokens) {
            Ok(size) => match fuzzy_core::dmetaphone_with_size(size, tokens.next().unwrap_or("")) {
                Ok((primary, secondary)) => {
                    format!("{}|{}", render_code(primary), render_code(secondary))
                }
                Err(err) => format!("ERROR {err}"),
            },
            Err(err) => err,
        },
        other => format!("ERROR unknown algorithm: {other:?} (expected soundex|nysiis|dmetaphone)"),
    }
}

/// Parse the `<size>` token; a missing or unparseable size is a line error.
fn parse_size<'a>(tokens: &mut impl Iterator<Item = &'a str>) -> Result<usize, String> {
    match tokens.next() {
        Some(token) => token
            .parse::<usize>()
            .map_err(|_| format!("ERROR bad size: {token:?} (expected a non-negative integer)")),
        None => Err("ERROR missing size (expected: <algo> <size> <word>)".to_string()),
    }
}

/// Render a code for the dmetaphone `<primary>|<secondary>` line: `-` for a
/// None code. Codes are ASCII by construction; lossy conversion is a
/// belt-and-braces guard for the raw-bytes path.
fn render_code(code: Option<Vec<u8>>) -> String {
    match code {
        Some(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        None => "-".to_string(),
    }
}
