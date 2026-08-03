#!/usr/bin/env python3
"""fuzz_diff.py -- seeded differential fuzz driver (architecture.md section 7.0).

PINNED interface (binding for workers and validators):

    --algo {soundex|nysiis|dmetaphone}   (required)
    --count N                            (default 50000; 1000 under --selftest)
    --seed S                             (default 20260803)
    --out <dir>                          (default <repo>/tools/reports; validators
                                          pass a %TEMP% dir so validation never
                                          dirties the repo)
    --selftest                           negative control: compares against a
                                          deliberately wrong expectation; MUST
                                          exit non-zero with mismatch_count >= 1
    --mode {fixed|original}              soundex only; forwarded to oracle_py.py
                                          (`original` = pre-fix upstream semantics
                                          for the documented-divergence report)

Compares the Rust port (fuzzy-cli batch stdin protocol) against the ground-truth
oracle over a deterministic seeded corpus:

    dmetaphone -> tools/oracle-c/dmoracle.exe (compiled ORIGINAL double_metaphone.c)
    soundex / nysiis -> tools/oracle_py.py (pure-Python oracle, FIXED semantics;
                        driven over the same batch line protocol as fuzzy-cli,
                        invoked as `python oracle_py.py --mode <mode>`)

Corpora (PINNED, per algo):
    dmetaphone (ASCII-only, 7): empty, single_char, ascii_words, name_like,
                                mixed_case, digits_punct, very_long
    soundex/nysiis (8):         those plus `unicode` (UTF-8 multi-byte)

Both sides are driven ONCE per run via batch stdin lines -- never one process
per case. Report written to <out>/fuzz_<algo>_<seed>_<count>.json (or
divergence_soundex_<seed>_<count>.json for `--algo soundex --mode original`,
or selftest_fuzz_<algo>_<seed>_<count>.json under --selftest) with the pinned
schema:

    {algo, seed, cases, corpora{<category>: <count>}, mismatch_count,
     mismatches: [{input, expected, actual}], elapsed_s, timestamp}

Exit code 0 iff mismatch_count == 0 (1 = mismatches, 2 = harness failure).
Deterministic: same seed + count reproduces the identical corpus prefix, so a
smaller --count run with the same seed replays a prefix of a larger run.
"""

from __future__ import annotations

import argparse
import json
import os
import random
import string
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
CLI_EXE = REPO_ROOT / "rust" / "target" / "debug" / "fuzzy-cli.exe"
DM_ORACLE_EXE = REPO_ROOT / "tools" / "oracle-c" / "dmoracle.exe"
DM_ORACLE_BUILD = REPO_ROOT / "tools" / "oracle-c" / "build_oracle.ps1"
PY_ORACLE = REPO_ROOT / "tools" / "oracle_py.py"
CARGO_EXE = Path(r"C:\Users\Lenovo\.cargo\bin\cargo.exe")

DEFAULT_SEED = 20260803
DEFAULT_COUNT = 50000
SELFTEST_COUNT = 1000
# Mismatch entries recorded in the report are capped so a pathological run (or
# the --selftest negative control) cannot produce a giant JSON; mismatch_count
# always reflects the true total.
MAX_RECORDED_MISMATCHES = 100

# PINNED corpora (architecture.md section 7.0). Order is fixed: the first
# len(categories) cases of every run are one of each category in this order,
# guaranteeing every category is covered (count >= 1) for any run with
# count >= len(categories), without breaking prefix determinism.
CATEGORIES_DM = [
    "empty",
    "single_char",
    "ascii_words",
    "name_like",
    "mixed_case",
    "digits_punct",
    "very_long",
]
CATEGORIES_SN = [
    "empty",
    "single_char",
    "ascii_words",
    "name_like",
    "mixed_case",
    "digits_punct",
    "unicode",
    "very_long",
]
# Sampling weights for cases beyond the guaranteed-coverage prefix.
WEIGHTS_DM = [1, 3, 42, 25, 12, 8, 2]
WEIGHTS_SN = [1, 3, 42, 25, 12, 8, 5, 2]

LOWER = string.ascii_lowercase
UPPER = string.ascii_uppercase
ALNUM = LOWER + UPPER + string.digits
PUNCT = "-'.,!@#+_"  # no whitespace (token protocol), no '|' (output separator)

# name_like syllable tables: biased toward onsets/codas that exercise the
# interesting Double Metaphone arms (GN/KN/PN/PS/WR skip, initial X, CZ/TZ,
# SCH, WITZ, SlavoGermanic W/K, vowel clusters).
NAME_PREFIXES = ["mac", "mc", "van", "von", "de", "del", "di", "le", "o'", "san", "st", "der"]
NAME_ONSETS = [
    "b", "c", "d", "f", "g", "h", "j", "k", "l", "m", "n", "p", "q", "r", "s",
    "t", "v", "w", "x", "z", "ch", "sh", "th", "ph", "gh", "kh", "kn", "gn",
    "pn", "ps", "wr", "rh", "cz", "tz", "sch", "sk", "sl", "sm", "sn", "sp",
    "st", "str", "br", "cr", "dr", "fr", "gr", "pr", "tr", "chr",
]
NAME_VOWELS = [
    "a", "e", "i", "o", "u", "y", "ae", "ai", "au", "ea", "ee", "ei", "eu",
    "ie", "oa", "oe", "oi", "oo", "ou", "ue",
]
NAME_CODAS = [
    "b", "c", "ck", "d", "f", "g", "gh", "k", "l", "ll", "m", "n", "nd", "ng",
    "nk", "nn", "p", "r", "rd", "rk", "rn", "rs", "rt", "s", "sh", "ss", "st",
    "t", "th", "tsch", "tz", "x", "z", "sch", "witz", "stein", "berg", "mann",
    "son", "sen", "ski", "sky", "wicz", "owski", "iewicz", "ham", "ton",
    "ford", "wood", "bury", "field",
]

# unicode category (soundex/nysiis only): Latin-1/Latin-extended letters that
# survive (or vanish under) the unicode-uppercase + A-Z filter.
ACCENTED = (
    "éèêëáàâäíìîïóòôöúùûüýÿñçßøåæœłżźćńśřďťňůšž"
    "ÉÈÊËÁÀÂÄÍÌÎÏÓÒÔÖÚÙÛÜÝÑÇØÅÆŒŁŻŹĆŃŚŘĎŤŇŮŠŽ"
)
UNICODE_WORDS = [
    "Jéroboam", "Straße", "Müller", "Søren", "François", "Björk", "Renée",
    "Zoë", "Åsa", "Dvořák", "Núñez", "García", "Schröder", "José", "Chloë",
    "Bjørn", "Guðmundur", "Łódź", "Michał", "Żółć",
]


# ---------------------------------------------------------------------------
# Corpus generation (deterministic; case i depends only on seed and i)
# ---------------------------------------------------------------------------

def gen_word(category: str, rng: random.Random) -> str:
    if category == "empty":
        return ""
    if category == "single_char":
        return rng.choice(ALNUM)
    if category == "ascii_words":
        return "".join(rng.choices(LOWER, k=rng.randint(2, 12)))
    if category == "name_like":
        parts = []
        if rng.random() < 0.25:
            parts.append(rng.choice(NAME_PREFIXES))
        for _ in range(rng.randint(1, 3)):
            syllable = rng.choice(NAME_ONSETS) + rng.choice(NAME_VOWELS)
            if rng.random() < 0.5:
                syllable += rng.choice(NAME_CODAS)
            parts.append(syllable)
        name = "".join(parts)
        return name[:1].upper() + name[1:]
    if category == "mixed_case":
        return "".join(rng.choices(LOWER + UPPER, k=rng.randint(2, 12)))
    if category == "digits_punct":
        n = rng.randint(1, 14)
        chars = rng.choices(LOWER + LOWER + string.digits + PUNCT, k=n)
        if all(c in LOWER for c in chars):
            chars[rng.randrange(n)] = rng.choice(string.digits + PUNCT)
        return "".join(chars)
    if category == "very_long":
        return "".join(rng.choices(ALNUM, k=rng.randint(200, 2000)))
    if category == "unicode":
        if rng.random() < 0.4:
            return rng.choice(UNICODE_WORDS)
        chars = list("".join(rng.choices(LOWER, k=rng.randint(2, 10))))
        for _ in range(rng.randint(1, 3)):
            chars.insert(rng.randrange(len(chars) + 1), rng.choice(ACCENTED))
        return "".join(chars)
    raise ValueError(f"unknown category: {category}")


def gen_size(algo: str, rng: random.Random) -> int:
    """Per-case <size> for the sized algorithms (nysiis takes none)."""
    if algo == "dmetaphone":
        # Mostly unlimited (0); sprinkle 1..5 to exercise wrapper truncation.
        return 0 if rng.random() < 0.8 else rng.randint(1, 5)
    if algo == "soundex":
        # Span the padding-semantics boundary: <=4 pads, >4 is a max only.
        return rng.choice([0, 1, 2, 3, 4, 4, 4, 5, 8, 100])
    return 0  # nysiis: unused


def generate_corpus(algo: str, count: int, seed: int):
    """Return (cases, corpora) where cases is a list of (category, size, word).

    Prefix-stable: the first N cases of a run are identical for any count >= N
    with the same seed. The first len(categories) cases are one of each
    category (pinned order) so every category is covered.
    """
    if algo == "dmetaphone":
        categories, weights = CATEGORIES_DM, WEIGHTS_DM
    else:
        categories, weights = CATEGORIES_SN, WEIGHTS_SN
    rng = random.Random(seed)
    cases = []
    corpora = {c: 0 for c in categories}
    for i in range(count):
        category = categories[i] if i < len(categories) else rng.choices(categories, weights)[0]
        word = gen_word(category, rng)
        size = gen_size(algo, rng)
        cases.append((category, size, word))
        corpora[category] += 1
    return cases, corpora


# ---------------------------------------------------------------------------
# Batch process driving (ONE process per side per run -- never per case)
# ---------------------------------------------------------------------------

def run_batch(argv, lines):
    """Feed `lines` (UTF-8, LF-terminated) to one process; return stdout lines.

    Exactly one output line is expected per input line. Raises SystemExit(2)
    on a non-zero exit (harness failure, not a port mismatch).
    """
    data = ("\n".join(lines) + "\n").encode("utf-8") if lines else b""
    proc = subprocess.run(argv, input=data, stdout=subprocess.PIPE, stderr=subprocess.PIPE)
    if proc.returncode != 0:
        sys.stderr.write(proc.stderr.decode("utf-8", "replace"))
        raise SystemExit(f"harness failure: {argv} exited {proc.returncode}")
    out = proc.stdout.decode("utf-8", "replace").split("\n")
    if out and out[-1] == "":
        out.pop()  # trailing newline after the last line
    return [line.rstrip("\r") for line in out]


def ensure_cli():
    if CLI_EXE.exists():
        return
    env = dict(os.environ)
    env["PATH"] = r"C:\Users\Lenovo\.cargo\bin;" + env.get("PATH", "")
    proc = subprocess.run(
        [str(CARGO_EXE), "build", "-p", "fuzzy-cli"],
        cwd=REPO_ROOT / "rust",
        env=env,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0 or not CLI_EXE.exists():
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit("harness failure: could not build fuzzy-cli (cargo build -p fuzzy-cli)")


def ensure_dm_oracle():
    if DM_ORACLE_EXE.exists():
        return
    proc = subprocess.run(
        [
            "powershell", "-NoProfile", "-ExecutionPolicy", "Bypass",
            "-File", str(DM_ORACLE_BUILD),
        ],
        cwd=REPO_ROOT,
        capture_output=True,
        text=True,
    )
    if proc.returncode != 0 or not DM_ORACLE_EXE.exists():
        sys.stderr.write(proc.stdout + proc.stderr)
        raise SystemExit(
            "harness failure: could not build the C oracle "
            "(tools/oracle-c/build_oracle.ps1)"
        )


def cli_line(algo: str, size: int, word: str) -> str:
    """One fuzzy-cli protocol line; a MISSING word token means empty string."""
    if algo == "nysiis":
        return f"nysiis {word}" if word else "nysiis"
    return f"{algo} {size} {word}" if word else f"{algo} {size}"


def dm_wrapper(primary: str, secondary: str, size: int):
    """Apply the .pyx wrapper semantics to RAW C-oracle codes.

    Mirrors fuzzy-core dmetaphone_with_size exactly (architecture.md 5.3):
    collapse equal codes BEFORE truncating; empty code -> None; size == 0 is
    unlimited (the C core already caps at 4).
    """
    def finish(code: str):
        if code == "":
            return None
        return code[:size] if size > 0 else code

    sec = None if primary == secondary else finish(secondary)
    return finish(primary), sec


def render_dm(primary, secondary) -> str:
    return "{0}|{1}".format(primary if primary is not None else "-",
                            secondary if secondary is not None else "-")


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(
        description="Seeded differential fuzz driver: fuzzy-cli vs ground-truth oracle."
    )
    parser.add_argument("--algo", required=True, choices=["soundex", "nysiis", "dmetaphone"])
    parser.add_argument("--count", type=int, default=None,
                        help=f"cases to generate (default {DEFAULT_COUNT}; "
                             f"{SELFTEST_COUNT} under --selftest)")
    parser.add_argument("--seed", type=int, default=DEFAULT_SEED)
    parser.add_argument("--out", default=None,
                        help="report directory (default <repo>/tools/reports)")
    parser.add_argument("--selftest", action="store_true",
                        help="negative control: compare against a deliberately "
                             "wrong expectation; exits non-zero")
    parser.add_argument("--mode", choices=["fixed", "original"], default="fixed",
                        help="soundex only: oracle semantics (original = pre-fix "
                             "upstream, for the divergence report)")
    args = parser.parse_args()

    if args.mode == "original" and args.algo != "soundex":
        parser.error("--mode original is soundex-only")
    count = args.count
    if count is None:
        count = SELFTEST_COUNT if args.selftest else DEFAULT_COUNT
    if count < 1:
        parser.error("--count must be >= 1")

    out_dir = Path(args.out) if args.out else REPO_ROOT / "tools" / "reports"
    if not out_dir.is_absolute():
        out_dir = REPO_ROOT / out_dir
    out_dir.mkdir(parents=True, exist_ok=True)

    # Locate / build the two sides.
    ensure_cli()
    if args.algo == "dmetaphone":
        ensure_dm_oracle()
        oracle_argv = [str(DM_ORACLE_EXE)]
    else:
        if not PY_ORACLE.exists():
            raise SystemExit(
                f"harness failure: {PY_ORACLE} not found (python oracle is built "
                "by the soundex/nysiis tooling feature)"
            )
        oracle_argv = [sys.executable, str(PY_ORACLE), "--mode", args.mode]

    cases, corpora = generate_corpus(args.algo, count, args.seed)

    cli_lines = [cli_line(args.algo, size, word) for _, size, word in cases]
    if args.algo == "dmetaphone":
        oracle_lines = [word for _, _, word in cases]  # raw words, one per line
    else:
        oracle_lines = list(cli_lines)  # oracle_py speaks the same protocol

    start = time.monotonic()
    cli_out = run_batch([str(CLI_EXE)], cli_lines)
    oracle_out = run_batch(oracle_argv, oracle_lines)
    elapsed = time.monotonic() - start

    if len(cli_out) != count or len(oracle_out) != count:
        raise SystemExit(
            f"harness failure: line-count drift (cases={count}, "
            f"cli={len(cli_out)}, oracle={len(oracle_out)})"
        )

    # Expected values: oracle output normalized to the CLI's wrapper format.
    expected = []
    if args.algo == "dmetaphone":
        for (_, size, _), raw in zip(cases, oracle_out):
            parts = raw.split("|")
            if len(parts) != 2:
                raise SystemExit(f"harness failure: bad oracle line {raw!r}")
            primary, secondary = dm_wrapper(parts[0], parts[1], size)
            expected.append(render_dm(primary, secondary))
    else:
        expected = list(oracle_out)

    if args.selftest:
        # Negative control: deliberately wrong expectations. No real code line
        # ever starts with this marker, so every case mismatches.
        expected = ["SELFTEST-WRONG:" + e for e in expected]

    mismatches = []
    mismatch_count = 0
    for (category, size, word), want, got in zip(cases, expected, cli_out):
        if want != got:
            mismatch_count += 1
            if len(mismatches) < MAX_RECORDED_MISMATCHES:
                mismatches.append({
                    "input": cli_line(args.algo, size, word),
                    "expected": want,
                    "actual": got,
                })

    if args.selftest:
        stem = f"selftest_fuzz_{args.algo}_{args.seed}_{count}"
    elif args.algo == "soundex" and args.mode == "original":
        stem = f"divergence_{args.algo}_{args.seed}_{count}"
    else:
        stem = f"fuzz_{args.algo}_{args.seed}_{count}"
    report_path = out_dir / f"{stem}.json"

    report = {
        "algo": args.algo,
        "seed": args.seed,
        "cases": count,
        "corpora": corpora,
        "mismatch_count": mismatch_count,
        "mismatches": mismatches,
        "elapsed_s": round(elapsed, 3),
        "timestamp": datetime.now(timezone.utc).isoformat(timespec="seconds"),
        "mode": args.mode,
        "selftest": bool(args.selftest),
    }
    report_path.write_text(json.dumps(report, indent=2) + "\n", encoding="utf-8")

    print(
        f"fuzz_diff algo={args.algo} seed={args.seed} cases={count} "
        f"mismatch_count={mismatch_count} elapsed_s={report['elapsed_s']} "
        f"report={report_path}"
    )
    return 0 if mismatch_count == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
