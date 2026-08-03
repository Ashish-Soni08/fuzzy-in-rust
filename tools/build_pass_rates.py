#!/usr/bin/env python3
"""build_pass_rates.py -- aggregate REAL pass-rate numbers into pass_rates.json.

This file is the single source DECISIONS.md quotes. Every number comes from an
actual command run or a committed report -- nothing is hand-entered:

  1. original_suite      <- parsed from the COMMITTED
                            tools/reports/original_suite_output.txt AND
                            cross-checked against a LIVE re-run of
                            `python -m pytest tests/original/ -v`.
  2. native_tests        <- this script RUNS `cargo test --workspace`
                            (workspace manifest rust/Cargo.toml) and parses the
                            per-target summaries, attributed to crates via the
                            workspace layout (Cargo.toml + tests/ dirs).
  3. fuzz_campaigns      <- parsed from each COMMITTED
                            tools/reports/fuzz_*.json (enumerated via
                            `git ls-files`, never the filesystem, so
                            uncommitted files cannot sneak in).
  4. divergence_summary  <- mismatch_count from the COMMITTED
                            tools/reports/divergence_soundex_*.json;
                            per-sanctioned-class counts from a deterministic
                            re-run of the seeded corpus (same seed/count as the
                            committed report) through fuzzy-cli and
                            oracle_py --mode original, cross-checked against
                            the committed total. Classification:
                            expected starts with "ERROR non-ASCII input"
                              -> bug #15 (Soundex non-ASCII raises originally)
                            expected == actual right-padded with '0'
                              -> bug #14 (Soundex pads to size > 4 originally)
                            anything else -> "unclassified" (MUST be zero; a
                            non-zero count fails this script loudly).
  5. known_limitations   <- curated prose (numbers interpolated from the
                            computed values above).

Usage (from the repo root, venv python):

    python tools/build_pass_rates.py [--out <dir>]

--out defaults to <repo>/tools/reports. Validators re-running for comparison
should pass a %TEMP% dir so validation never dirties the repo; `generated_at`
and `elapsed_s`-style fields are informational -- counts are deterministic.

Exit code 0 on success; 1 on any cross-check failure (the report is NOT
written on failure -- an unverifiable number must never be committed).
"""

from __future__ import annotations

import argparse
import json
import re
import subprocess
import sys
import tomllib
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
RUST_DIR = REPO_ROOT / "rust"
TOOLS_DIR = REPO_ROOT / "tools"
REPORTS_DIR = TOOLS_DIR / "reports"
ORIGINAL_SUITE_OUTPUT = REPORTS_DIR / "original_suite_output.txt"
CARGO_EXE = Path(r"C:\Users\Lenovo\.cargo\bin\cargo.exe")

CARGO_TIMEOUT_S = 1800
PYTEST_TIMEOUT_S = 300
BATCH_TIMEOUT_S = 300

# The two sanctioned divergence classes (architecture.md section 2, bugs
# yougov/fuzzy#14 and yougov/fuzzy#15). These keys are part of the report
# schema quoted by DECISIONS.md.
CLASS_PAD_GT4 = "soundex_size_gt4_padding_bug_14"
CLASS_NON_ASCII = "soundex_non_ascii_input_bug_15"


def fail(msg: str) -> "SystemExit":
    sys.stderr.write(f"build_pass_rates: FAIL: {msg}\n")
    return SystemExit(1)


def git(repo_root: Path, *args: str) -> str:
    proc = subprocess.run(
        ["git", *args], cwd=repo_root, capture_output=True, text=True, timeout=60
    )
    if proc.returncode != 0:
        raise fail(f"git {' '.join(args)} exited {proc.returncode}: {proc.stderr.strip()}")
    return proc.stdout


# ---------------------------------------------------------------------------
# 1. Original pytest suite (committed output + live re-run cross-check)
# ---------------------------------------------------------------------------

SUMMARY_TOKEN_RE = re.compile(r"(\d+) (passed|xpassed|xfailed|failed|errors?)\b")
PER_TEST_RE = re.compile(
    r"^tests/original/test_fuzzy\.py::(\w+)\s+(PASSED|XPASS|XFAIL|FAILED|ERROR)\b",
    re.MULTILINE,
)
ANSI_RE = re.compile(r"\x1b\[[0-9;]*m")


def parse_pytest_output(text: str, source: str) -> dict:
    # pytest colorizes when its config forces --color=yes; strip ANSI escapes so
    # the banner/per-test parsing works for both live runs and captured files.
    text = ANSI_RE.sub("", text)
    collected_m = re.search(r"collected (\d+) items?", text)
    if not collected_m:
        raise fail(f"{source}: could not find 'collected N items' line")
    counts = {"passed": 0, "xpassed": 0, "xfailed": 0, "failed": 0, "errors": 0}
    summary_line = None
    for line in text.splitlines():
        if line.startswith("=") and SUMMARY_TOKEN_RE.search(line):
            summary_line = line  # last matching banner line wins
    if summary_line is None:
        raise fail(f"{source}: could not find the pytest summary banner line")
    for n, kind in SUMMARY_TOKEN_RE.findall(summary_line):
        key = "errors" if kind == "error" else kind
        counts[key] = int(n)
    per_test = {name: verdict for name, verdict in PER_TEST_RE.findall(text)}
    return {
        "collected": int(collected_m.group(1)),
        **counts,
        "per_test": per_test,
        "summary_line": summary_line.strip("= "),
    }


def original_suite_section() -> dict:
    if not ORIGINAL_SUITE_OUTPUT.exists():
        raise fail(f"committed {ORIGINAL_SUITE_OUTPUT} is missing")
    committed = parse_pytest_output(
        ORIGINAL_SUITE_OUTPUT.read_text(encoding="utf-8"), "committed original_suite_output.txt"
    )

    # Live re-run (the venv python runs this script, so sys.executable has pytest).
    proc = subprocess.run(
        [sys.executable, "-m", "pytest", "tests/original/", "-v"],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
        timeout=PYTEST_TIMEOUT_S,
    )
    live = parse_pytest_output(proc.stdout + proc.stderr, "live pytest re-run")
    for key in ("collected", "passed", "xpassed", "xfailed", "failed", "errors"):
        if live[key] != committed[key]:
            raise fail(
                f"live pytest re-run disagrees with committed original_suite_output.txt "
                f"on {key}: live={live[key]} committed={committed[key]}"
            )
    if proc.returncode != 0:
        raise fail(f"live pytest re-run exited {proc.returncode} (expected 0)")

    return {
        "source": "tools/reports/original_suite_output.txt",
        "command": "python -m pytest tests/original/ -v (venv python; services.yaml: test-original)",
        "collected": committed["collected"],
        "passed": committed["passed"],
        "xpassed": committed["xpassed"],
        "xfailed": committed["xfailed"],
        "failed": committed["failed"],
        "errors": committed["errors"],
        "exit_code": proc.returncode,
        "per_test": committed["per_test"],
        "summary_line": committed["summary_line"],
        "note": (
            "The 3 xpassed tests are the intentional bug fixes #14/#15: the "
            "original suite marks them non-strict xfail, and the port implements "
            "the behavior the tests intend, so they XPASS. Verified against a "
            "live re-run at generation time (identical counts, exit 0)."
        ),
    }


# ---------------------------------------------------------------------------
# 2. Native Rust tests (live `cargo test --workspace` run, parsed)
# ---------------------------------------------------------------------------

RUNNING_RE = re.compile(
    r"^\s*Running (?:unittests )?(?P<target>.+?) "
    r"\(.*[\\/](?P<stem>[A-Za-z0-9_]+)-[0-9a-f]+\.exe\)\s*$"
)
DOCTEST_RE = re.compile(r"^\s*Doc-tests (?P<lib>\S+)\s*$")
RESULT_RE = re.compile(
    r"^test result: (?P<verdict>\w+)\. (?P<passed>\d+) passed; (?P<failed>\d+) failed; "
    r"(?P<ignored>\d+) ignored; (?P<measured>\d+) measured; (?P<filtered>\d+) filtered out"
)


def workspace_stem_map() -> dict:
    """Map test-binary stem -> (crate package name, target label)."""
    ws = tomllib.loads((RUST_DIR / "Cargo.toml").read_text(encoding="utf-8"))
    stem_map: dict[str, tuple[str, str]] = {}
    lib_to_pkg: dict[str, str] = {}

    def claim(stem: str, pkg: str, label: str) -> None:
        if stem in stem_map:
            raise fail(f"ambiguous test-binary stem {stem!r} ({stem_map[stem]} vs {pkg}/{label})")
        stem_map[stem] = (pkg, label)

    for member in ws["workspace"]["members"]:
        crate_dir = RUST_DIR / member
        pkg_toml = tomllib.loads((crate_dir / "Cargo.toml").read_text(encoding="utf-8"))
        pkg = pkg_toml["package"]["name"]
        default_name = pkg.replace("-", "_")
        if (crate_dir / "src" / "lib.rs").exists():
            lib_name = pkg_toml.get("lib", {}).get("name", default_name)
            lib_to_pkg[lib_name] = pkg
            claim(lib_name, pkg, "unit (src/lib.rs)")
        if (crate_dir / "src" / "main.rs").exists():
            claim(default_name, pkg, "unit (src/main.rs)")
        tests_dir = crate_dir / "tests"
        if tests_dir.is_dir():
            for test_file in sorted(tests_dir.glob("*.rs")):
                claim(test_file.stem, pkg, f"integration (tests/{test_file.name})")
    return stem_map, lib_to_pkg


def native_tests_section() -> dict:
    cargo = str(CARGO_EXE) if CARGO_EXE.exists() else "cargo"
    proc = subprocess.run(
        [cargo, "test", "--workspace"],
        cwd=RUST_DIR,
        stdout=subprocess.PIPE,
        stderr=subprocess.STDOUT,  # merged, preserving header/result order
        text=True,
        timeout=CARGO_TIMEOUT_S,
    )
    output = proc.stdout
    if proc.returncode != 0:
        sys.stderr.write(output)
        raise fail(f"cargo test --workspace exited {proc.returncode}")

    stem_map, lib_to_pkg = workspace_stem_map()
    crates: dict[str, dict] = {}
    current = None  # (pkg, label) awaiting its result line
    targets_seen = 0
    for line in output.splitlines():
        m = RUNNING_RE.match(line)
        if m:
            stem = m.group("stem")
            if stem not in stem_map:
                raise fail(f"unknown test-binary stem {stem!r} in line: {line.strip()!r}")
            current = stem_map[stem]
            continue
        m = DOCTEST_RE.match(line)
        if m:
            lib = m.group("lib")
            if lib not in lib_to_pkg:
                raise fail(f"unknown Doc-tests crate {lib!r}")
            current = (lib_to_pkg[lib], "doc-tests")
            continue
        m = RESULT_RE.match(line)
        if m:
            if current is None:
                raise fail(f"test result line with no preceding target header: {line!r}")
            pkg, label = current
            passed, failed = int(m.group("passed")), int(m.group("failed"))
            if m.group("verdict") != "ok" or failed != 0:
                raise fail(f"target {pkg}/{label} not green: {line.strip()!r}")
            entry = crates.setdefault(pkg, {"targets": {}, "passed": 0, "failed": 0})
            entry["targets"][label] = {"passed": passed, "failed": failed}
            entry["passed"] += passed
            entry["failed"] += failed
            targets_seen += 1
            current = None
    if targets_seen == 0:
        raise fail("parsed zero test targets from cargo output -- parser drift?")

    return {
        "command": "cargo test --workspace (from rust/; services.yaml: test)",
        "crates": crates,
        "total_passed": sum(c["passed"] for c in crates.values()),
        "total_failed": sum(c["failed"] for c in crates.values()),
        "exit_code": proc.returncode,
        "note": (
            "Counts parsed from a live `cargo test --workspace` run at generation "
            "time; per-crate attribution derives test-binary stems from the "
            "workspace layout (Cargo.toml [lib] names + tests/ dirs)."
        ),
    }


# ---------------------------------------------------------------------------
# 3. Fuzz campaigns (committed reports only, via git ls-files)
# ---------------------------------------------------------------------------

def load_committed_reports(pattern: str) -> dict[str, dict]:
    paths = git(REPO_ROOT, "ls-files", pattern).split()
    reports = {}
    for rel in paths:
        report = json.loads((REPO_ROOT / rel).read_text(encoding="utf-8"))
        reports[rel] = report
    return reports


def fuzz_campaigns_section() -> dict:
    reports = load_committed_reports("tools/reports/fuzz_*.json")
    by_algo: dict[str, dict] = {}
    for rel, report in sorted(reports.items()):
        algo = report["algo"]
        if algo in by_algo:
            raise fail(f"multiple committed fuzz reports for algo {algo!r}: {rel}")
        if report["mismatch_count"] != len(report["mismatches"]):
            raise fail(f"{rel}: mismatch_count != len(mismatches)")
        if sum(report["corpora"].values()) != report["cases"]:
            raise fail(f"{rel}: corpora counts do not sum to cases")
        by_algo[algo] = {
            "report": rel,
            "seed": report["seed"],
            "cases": report["cases"],
            "mismatch_count": report["mismatch_count"],
            "corpora": report["corpora"],
            "mode": report.get("mode"),
            "elapsed_s": report["elapsed_s"],
            "timestamp": report["timestamp"],
        }
    for algo in ("soundex", "nysiis", "dmetaphone"):
        if algo not in by_algo:
            raise fail(f"no committed fuzz report for algo {algo!r}")
    return {
        "source": "committed tools/reports/fuzz_*.json (enumerated via git ls-files)",
        "command": (
            "python tools/fuzz_diff.py --algo <algo> --count 50000 "
            "(default seed 20260803; services.yaml: fuzz-soundex / fuzz-nysiis / fuzz-dmetaphone)"
        ),
        "algorithms": by_algo,
        "total_cases": sum(a["cases"] for a in by_algo.values()),
        "total_mismatches": sum(a["mismatch_count"] for a in by_algo.values()),
        "note": (
            "dmetaphone: Rust port vs the compiled ORIGINAL C (tools/oracle-c). "
            "soundex/nysiis: Rust port vs tools/oracle_py.py (fixed semantics). "
            "elapsed_s/timestamp are informational fields from the committed reports."
        ),
    }


# ---------------------------------------------------------------------------
# 4. Divergence summary (committed report + deterministic classification re-run)
# ---------------------------------------------------------------------------

def divergence_section() -> dict:
    reports = load_committed_reports("tools/reports/divergence_soundex_*.json")
    if len(reports) != 1:
        raise fail(f"expected exactly 1 committed divergence report, found {len(reports)}")
    rel, committed = next(iter(reports.items()))
    seed, count = committed["seed"], committed["cases"]

    sys.path.insert(0, str(TOOLS_DIR))
    import fuzz_diff  # noqa: E402 -- repo tooling module, stdlib-only

    cases, _corpora = fuzz_diff.generate_corpus("soundex", count, seed)
    lines = [fuzz_diff.cli_line("soundex", size, word) for _, size, word in cases]
    fuzz_diff.ensure_cli()
    cli_out = fuzz_diff.run_batch([str(fuzz_diff.CLI_EXE)], lines)
    oracle_out = fuzz_diff.run_batch(
        [sys.executable, str(fuzz_diff.PY_ORACLE), "--mode", "original"], lines
    )
    if len(cli_out) != count or len(oracle_out) != count:
        raise fail(
            f"divergence re-run line-count drift: cases={count} "
            f"cli={len(cli_out)} oracle={len(oracle_out)}"
        )

    by_class = {CLASS_PAD_GT4: 0, CLASS_NON_ASCII: 0}
    unclassified = []
    total = 0
    for line, want, got in zip(lines, oracle_out, cli_out):
        if want == got:
            continue
        total += 1
        if want.startswith("ERROR non-ASCII input"):
            by_class[CLASS_NON_ASCII] += 1
        elif len(want) > len(got) and want.startswith(got) and set(want[len(got):]) == {"0"}:
            by_class[CLASS_PAD_GT4] += 1
        else:
            unclassified.append({"input": line, "expected": want, "actual": got})

    if total != committed["mismatch_count"]:
        raise fail(
            f"divergence re-run found {total} mismatches but the committed report "
            f"{rel} records {committed['mismatch_count']} -- corpus/harness drift?"
        )
    if unclassified:
        raise fail(
            f"{len(unclassified)} divergence cases fall OUTSIDE the two sanctioned "
            f"classes (#14 padding / #15 non-ASCII); first: {unclassified[0]}"
        )

    return {
        "report": rel,
        "command": (
            "python tools/fuzz_diff.py --algo soundex --mode original --count "
            f"{count} (seed {seed}; services.yaml: fuzz-soundex-divergence; "
            "expected exit code NON-zero -- the mismatches ARE the bug-fix evidence)"
        ),
        "cases": count,
        "seed": seed,
        "mismatch_count": committed["mismatch_count"],
        "by_sanctioned_class": by_class,
        "unclassified": len(unclassified),
        "recorded_mismatch_entries": len(committed["mismatches"]),
        "classification_method": (
            "Deterministic re-run of the seeded corpus (same seed/count as the "
            "committed report) through fuzzy-cli and oracle_py --mode original, "
            "reusing tools/fuzz_diff.py's own generate_corpus/run_batch. "
            "expected=='ERROR non-ASCII input...' => bug #15; expected==actual "
            "right-padded with '0' => bug #14; anything else => unclassified. "
            "Re-run total cross-checked against the committed mismatch_count."
        ),
        "note": (
            "ALL divergences from original Soundex semantics fall into the two "
            "sanctioned bug-fix classes (architecture.md section 2). nysiis and "
            "dmetaphone have ZERO divergence from original semantics (their fuzz "
            "reports above show mismatch_count 0)."
        ),
    }


# ---------------------------------------------------------------------------
# 5. Known limitations (curated prose; numbers interpolated from real values)
# ---------------------------------------------------------------------------

def known_limitations(divergence: dict) -> list[str]:
    sys.path.insert(0, str(TOOLS_DIR))
    import fuzz_diff  # noqa: E402 -- for MAX_RECORDED_MISMATCHES

    return [
        (
            "DMetaphone Latin-1 byte arms (0xC7 'C-cedilla' -> S, 0xD1 'N-tilde' -> N) "
            "are unreachable via the Python API: Python str input is UTF-8 and "
            "DMetaphone rejects non-ASCII by design, so those raw bytes can never "
            "arrive. The arms are ported for fidelity and covered natively through "
            "dmetaphone_bytes (latin1 tests)."
        ),
        (
            "Words containing spaces are not expressible in the fuzzy-cli batch "
            "protocol (the word field is one whitespace-delimited token), so the CLI "
            "fuzz corpora cannot include them; whitespace-in-word behavior is covered "
            "by native Rust tests instead."
        ),
        (
            "DMetaphone non-ASCII input raises UnicodeEncodeError through the Python "
            "API by design (original behavior preserved per architecture.md section 2; "
            "only Soundex bugs #14/#15 are sanctioned fixes)."
        ),
        (
            "Soundex intentionally diverges from the original in exactly two classes: "
            "size > 4 is a maximum length, not a zero-pad target (bug #14), and "
            "non-ASCII input is unicode-uppercased then A-Z-filtered instead of "
            "raising UnicodeEncodeError (bug #15). These are the 3 XPASS results in "
            f"the original suite and the {divergence['mismatch_count']} cases in the "
            "committed divergence report."
        ),
        (
            "The committed divergence report records at most "
            f"{fuzz_diff.MAX_RECORDED_MISMATCHES} mismatch entries "
            "(MAX_RECORDED_MISMATCHES cap in tools/fuzz_diff.py); mismatch_count is "
            "the authoritative total, and the per-class counts in this file come from "
            "a deterministic re-run of the seeded corpus (see divergence_summary)."
        ),
    ]


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(
        description="Aggregate real pass-rate numbers into pass_rates.json."
    )
    parser.add_argument(
        "--out",
        default=None,
        help="report directory (default <repo>/tools/reports; validators pass a %TEMP% dir)",
    )
    args = parser.parse_args()
    out_dir = Path(args.out) if args.out else REPORTS_DIR
    if not out_dir.is_absolute():
        out_dir = REPO_ROOT / out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    original = original_suite_section()
    native = native_tests_section()
    fuzz = fuzz_campaigns_section()
    divergence = divergence_section()

    report = {
        "_meta": {
            "purpose": (
                "Honest pass-rate numbers for the fuzzy Python->Rust port; the "
                "single source DECISIONS.md quotes."
            ),
            "generated_by": "tools/build_pass_rates.py",
            "regenerate": "python tools/build_pass_rates.py (venv python, from the repo root)",
            "generated_at": datetime.now(timezone.utc).isoformat(timespec="seconds"),
            "repo_head_at_generation": git(REPO_ROOT, "rev-parse", "HEAD").strip(),
            "honesty_rule": (
                "Every number comes from an actual command run or a committed "
                "report; the generator fails loudly instead of writing an "
                "unverifiable number. generated_at / repo_head_at_generation / "
                "elapsed_s / timestamp fields are informational; all counts are "
                "deterministic."
            ),
        },
        "original_suite": original,
        "native_tests": native,
        "fuzz_campaigns": fuzz,
        "divergence_summary": divergence,
        "known_limitations": known_limitations(divergence),
        "headline": {
            "original_suite": f"{original['passed']} passed, {original['xpassed']} xpassed",
            "native_tests": f"{native['total_passed']} passed, {native['total_failed']} failed",
            "fuzz": (
                f"{fuzz['total_cases']} cases, {fuzz['total_mismatches']} mismatches "
                "across soundex/nysiis/dmetaphone"
            ),
            "divergence": (
                f"{divergence['mismatch_count']} intentional divergences, all in the "
                "2 sanctioned bug-fix classes (#14 padding, #15 non-ASCII)"
            ),
        },
    }

    report_path = out_dir / "pass_rates.json"
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")
    print(f"pass_rates: wrote {report_path}")
    print(f"  original_suite : {report['headline']['original_suite']} (exit {original['exit_code']})")
    print(f"  native_tests   : {report['headline']['native_tests']}")
    print(f"  fuzz           : {report['headline']['fuzz']}")
    print(f"  divergence     : {report['headline']['divergence']}")
    print(f"  by_class       : {divergence['by_sanctioned_class']}")
    return 0


if __name__ == "__main__":
    sys.exit(main())
