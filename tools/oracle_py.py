#!/usr/bin/env python3
"""oracle_py.py -- pure-Python ground-truth oracle for Soundex and NYSIIS.

Ground truth for the differential fuzzing of the Rust port (architecture.md
section 7, leg 3):

  * NYSIIS   -- transcribed EXACTLY from src/fuzzy.pyx (lines 81-185), all
                four lookup tables verbatim (_nysiis_suffix_map 10 entries,
                _nysiis_transforms 18, _nysiis_trans_not_first 19,
                _nysiis_trans_middle 1), including every quirk:
                unicode-uppercase then [^A-Z] strip (ss survives via
                '\\xdf'.upper() == 'SS'), trailing S/Z strip, MAC->MC /
                PF->drop-P, the suffix loop, and the empty-string quirk
                ('' in 'AEIOU' is True, so nysiis('') == '').
  * Soundex  -- two selectable semantics (--mode):
      - `fixed`    (default): architecture.md section 5.1 exactly (S0-S5) --
                    unicode-uppercase + A-Z filter (#15 fix), first letter
                    verbatim, simplified dedup with the `written == 1`
                    clause, stop at `size`, pad with '0' to `size` ONLY when
                    size <= 4 (#14 fix), size == 0 short-circuits to ''.
      - `original`: replicates the pre-fix upstream Cython semantics --
                    ASCII-strict (non-ASCII input raises UnicodeEncodeError,
                    bug #15) and pad-to-size ALWAYS (bug #14). Used for the
                    documented-divergence report
                    (tools/reports/divergence_soundex_<seed>_<count>.json).

Batch interface: identical to fuzzy-cli's pinned line protocol
(architecture.md section 7.0) so fuzz_diff.py can drive both sides the same
way. Reads UTF-8 BOM-less lines on stdin (a stray BOM on the first line is
stripped -- library/environment.md quirk #10), one output line per input
line, empty code -> empty line, errors -> `ERROR <message>` without aborting
the batch:

    soundex <size> <word>    -> the code
    nysiis <word>            -> the code
    dmetaphone ...           -> ERROR (DM ground truth is tools/oracle-c)

A MISSING word token means the empty string. Output is UTF-8 BOM-less.

--selftest asserts every binding data point (fixed Soundex F200/F521/T23/
J615/0000/0000/''; NYSIIS FASY/''/''; plus the contract's MACBETH/PFISTER/
AEIOU points and original-mode bug-replication checks) and exits non-zero on
any failure.

Non-ASCII literals below are written as \\xNN/\\uNNNN escapes so no console
codepage or editor re-encoding can corrupt them (validation-contract
convention).
"""

from __future__ import annotations

import argparse
import re
import sys

# ---------------------------------------------------------------------------
# Soundex
# ---------------------------------------------------------------------------

# Letter -> digit map (classic), indexed by c - 'A' (A=0 .. Z=25).
_SOUNDEX_MAP = "01230120022455012623010202"


def soundex_fixed(size: int, s: str) -> str:
    """FIXED Soundex semantics, architecture.md section 5.1 (rules S0-S5)."""
    # S0: size 0 short-circuits (matches the original's Soundex(0) -> '').
    if size == 0:
        return ""
    out: list[str] = []
    # S1 (#15 fix): unicode-uppercase (Python str.upper()), then keep only
    # ASCII A-Z -- accented letters, digits, spaces, punctuation dropped.
    # '\xdf'.upper() == 'SS' survives the filter, exactly like the Rust
    # str::to_uppercase() path.
    for c in s.upper():
        if not ("A" <= c <= "Z"):
            continue
        if not out:
            # S3: first letter verbatim.
            out.append(c)
        else:
            d = _SOUNDEX_MAP[ord(c) - 65]
            # S4: skip code-0 letters; append iff written == 1 OR
            # last_written != d (the written == 1 clause bypasses dedup for
            # the first digit; comparison is against the last WRITTEN char
            # only -- the simplified dedup, no H/W special-casing).
            if d != "0" and (len(out) == 1 or out[-1] != d):
                out.append(d)
        # S4: stop as soon as written == size.
        if len(out) == size:
            break
    # S5 (#14 fix): pad with '0' to exactly `size` only when size <= 4;
    # for size > 4, `size` is a maximum length and no padding happens.
    if size <= 4:
        while len(out) < size:
            out.append("0")
    return "".join(out)


def soundex_original(size: int, s: str) -> str:
    """ORIGINAL (pre-fix) upstream Soundex, replicating src/fuzzy.pyx exactly.

    Byte-oriented over the ASCII encoding of `s`: raises UnicodeEncodeError
    on ANY non-ASCII input (bug #15); manual c-32 lowercase mapping; pads
    with '0' to `size` ALWAYS, including size > 4 (bug #14). The result is
    truncated to `size` (the C code NUL-terminates at out[size]), which also
    reproduces Soundex(0) -> ''.
    """
    data = s.encode("ascii")  # bug #15: UnicodeEncodeError on non-ASCII
    out = bytearray()
    for c in data:
        if 97 <= c <= 122:  # a-z -> A-Z (manual toupper)
            c = c - 32
        if 65 <= c <= 90:  # A-Z only; every other byte skipped
            if not out:
                out.append(c)
            else:
                d = _SOUNDEX_MAP[c - 65]
                if d != "0" and (len(out) == 1 or chr(out[-1]) != d):
                    out.append(ord(d))
        if len(out) == size:
            break
    # bug #14: pad to size unconditionally.
    while len(out) < size:
        out.append(ord("0"))
    return bytes(out[:size]).decode("ascii")


# ---------------------------------------------------------------------------
# NYSIIS -- transcribed verbatim from src/fuzzy.pyx (statement for statement)
# ---------------------------------------------------------------------------

_NYSIIS_SUFFIX_MAP = {
    'IX': 'IC',
    'EX': 'EC',
    'YE': 'Y',
    'EE': 'Y',
    'IE': 'Y',
    'DT': 'D',
    'RT': 'D',
    'RD': 'D',
    'NT': 'D',
    'ND': 'D'
}

_NYSIIS_TRANSFORMS = {
    'AY':  'Y',
    'DG':  'G',
    'E':   'A',
    'EY':  'Y',
    'GHT': 'GT',
    'K':   'C',
    'KN':  'N',
    'I':   'A',
    'IY':  'Y',
    'O':   'A',
    'OY':  'Y',
    'PH':  'F',
    'SH':  'S',
    'SCH': 'S',
    'U':   'A',
    'UY':  'Y',
    'WR':  'R',
    'YW':  'Y'
}

_NYSIIS_TRANS_NOT_FIRST = {
    'AH': 'A',
    'AW': 'A',
    'EH': 'A',
    'EV': 'AF',
    'EW': 'A',
    'HA': 'A',
    'HE': 'A',
    'HI': 'A',
    'HO': 'A',
    'HU': 'A',
    'IH': 'A',
    'IW': 'A',
    'M':  'N',
    'OH': 'A',
    'OW': 'A',
    'Q':  'G',
    'UH': 'A',
    'UW': 'A',
    'Z':  'S'
}

_NYSIIS_TRANS_MIDDLE = {
    'Y': 'A'
}

_NON_AZ = re.compile('[^A-Z]')


def nysiis(s: str) -> str:
    """Exact transcription of nysiis() from src/fuzzy.pyx lines 81-185."""
    # Strip out anything non-alpha (AFTER unicode-aware upper(): the
    # '\xdf' -> 'SS' quirk survives the filter).
    s = _NON_AZ.sub('', s.upper())
    start, stop = 0, len(s)

    first = ''
    if stop:
        first = s[0]

    # Find index without trailing SZs
    i = stop
    while i and s[i - 1] in 'SZ':
        i = i - 1
    stop = i

    # Initial MAC -> MC, PF -> F
    if s[:3] == 'MAC':
        s = 'MC' + s[3:]
        stop = stop - 1
    elif s[:2] == 'PF':
        start = 1

    # Translate 2-character suffix elements
    suffix = ''
    while (stop - start) > 2:
        x = s[stop - 2:stop]
        if x in _NYSIIS_SUFFIX_MAP:
            suffix = _NYSIIS_SUFFIX_MAP[x] + suffix
            stop = stop - 2
        else:
            break

    s = s[start:stop] + suffix

    # Build a list of adjacent components while performing transformations
    r = []
    i = start = 0
    stop = len(s)
    while i < stop:
        remain = stop - i  # number of letters including this one

        app = ''

        for length in 3, 2, 1:
            if remain >= length:
                x = s[i:i + length]
                if x in _NYSIIS_TRANSFORMS:
                    app = _NYSIIS_TRANSFORMS[x]
                    break
                elif i > start:
                    if x in _NYSIIS_TRANS_NOT_FIRST:
                        app = _NYSIIS_TRANS_NOT_FIRST[x]
                        break
                    elif i < (stop - 1) and x in _NYSIIS_TRANS_MIDDLE:
                        app = _NYSIIS_TRANS_MIDDLE[x]
                        break

        if app:
            r.extend(app)
            i = i + length
        else:
            r.append(s[i])
            i = i + 1

    # Remove trailing vowels
    stop = len(r)
    while stop and r[stop - 1] in 'AEIOU':
        stop = stop - 1

    # If first char of original string is a vowel, use it. QUIRK: in Python
    # '' in 'AEIOU' is True, so empty input takes this branch too (and
    # nysiis('') == '' because the r[:stop] slice below stays empty).
    if first in 'AEIOU':
        if r:
            r[0] = first
        else:
            r = [first]

    # Filter out repeated characters
    q, last = [], ''
    for x in r[:stop]:
        if x == last:
            continue
        q.append(x)
        last = x

    return ''.join(q)


# ---------------------------------------------------------------------------
# Batch line protocol (identical to fuzzy-cli; architecture.md section 7.0)
# ---------------------------------------------------------------------------

_SIZE_TOKEN = re.compile(r"^\+?[0-9]+$")


def _parse_size(tokens: list[str]):
    """Mirror fuzzy-cli parse_size: missing/unparseable size is a line error."""
    if not tokens:
        return None, "ERROR missing size (expected: <algo> <size> <word>)"
    token = tokens[0]
    if not _SIZE_TOKEN.match(token):
        return None, f'ERROR bad size: "{token}" (expected a non-negative integer)'
    return int(token), None


def handle_line(line: str, mode: str) -> str:
    """Handle one input line and produce its single output line."""
    tokens = line.split()
    if not tokens:
        return "ERROR empty line (expected: soundex|nysiis|dmetaphone ...)"
    algo, rest = tokens[0], tokens[1:]
    if algo == "soundex":
        size, err = _parse_size(rest)
        if err is not None:
            return err
        word = rest[1] if len(rest) > 1 else ""
        try:
            if mode == "original":
                return soundex_original(size, word)
            return soundex_fixed(size, word)
        except UnicodeEncodeError:
            # Original semantics raise on non-ASCII (bug #15); the batch
            # protocol reports it as an ERROR line and keeps going.
            return ("ERROR non-ASCII input: original Soundex raises "
                    "UnicodeEncodeError (upstream bug #15)")
    if algo == "nysiis":
        return nysiis(rest[0] if rest else "")
    if algo == "dmetaphone":
        return ("ERROR dmetaphone is not implemented by oracle_py "
                "(ground truth is tools/oracle-c/dmoracle.exe)")
    return (f'ERROR unknown algorithm: "{algo}" '
            "(expected soundex|nysiis|dmetaphone)")


# ---------------------------------------------------------------------------
# Self-test: every binding data point (architecture.md section 5 + contract)
# ---------------------------------------------------------------------------

def selftest() -> int:
    jéroboam = "J\u00e9roboam"  # é = é
    strasse = "Stra\u00dfe"     # ß = ß (.upper() -> 'SS' quirk)
    checks = []

    def check(label, actual, expected):
        checks.append((label, actual, expected))

    # Fixed Soundex -- the 7 binding data points of architecture.md 5.1.
    check("fixed Soundex(4)('fuzzy')", soundex_fixed(4, "fuzzy"), "F200")
    check("fixed Soundex(4)('FancyFree')", soundex_fixed(4, "FancyFree"), "F521")
    check("fixed Soundex(8)('Test')", soundex_fixed(8, "Test"), "T23")
    check("fixed Soundex(8)('J\\u00e9roboam')", soundex_fixed(8, jéroboam), "J615")
    check("fixed Soundex(4)('')", soundex_fixed(4, ""), "0000")
    check("fixed Soundex(4)('123')", soundex_fixed(4, "123"), "0000")
    check("fixed Soundex(0)('anything')", soundex_fixed(0, "anything"), "")
    # Dedup-quirk anchors from RULEBOOK.md section 4.
    check("fixed Soundex(4)('BABAB')", soundex_fixed(4, "BABAB"), "B100")
    check("fixed Soundex(4)('Tymczak')", soundex_fixed(4, "Tymczak"), "T520")
    check("fixed Soundex(4)('Stra\\u00dfe')", soundex_fixed(4, strasse), "S362")

    # NYSIIS -- binding data points (5.2) + the contract's extra anchors.
    check("nysiis('fuzzy')", nysiis("fuzzy"), "FASY")
    check("nysiis('')", nysiis(""), "")
    check("nysiis('123')", nysiis("123"), "")
    check("nysiis('MACBETH')", nysiis("MACBETH"), "MCBATH")
    check("nysiis('PFISTER')", nysiis("PFISTER"), "FASTAR")
    check("nysiis('AEIOU')", nysiis("AEIOU"), "")

    # Original Soundex -- bug replication (#14 pad-always, #15 ASCII-strict).
    check("original Soundex(8)('Test')", soundex_original(8, "Test"), "T2300000")
    check("original Soundex(4)('fuzzy')", soundex_original(4, "fuzzy"), "F200")
    check("original Soundex(4)('')", soundex_original(4, ""), "0000")
    check("original Soundex(0)('anything')", soundex_original(0, "anything"), "")
    try:
        soundex_original(8, jéroboam)
    except UnicodeEncodeError:
        check("original Soundex(8)('J\\u00e9roboam') raises", "UnicodeEncodeError",
              "UnicodeEncodeError")
    else:
        check("original Soundex(8)('J\\u00e9roboam') raises", "no error",
              "UnicodeEncodeError")

    failures = 0
    for label, actual, expected in checks:
        ok = actual == expected
        failures += 0 if ok else 1
        print(f"{'PASS' if ok else 'FAIL'}  {label}  ->  {actual!r}"
              + ("" if ok else f"  (expected {expected!r})"))
    print(f"oracle_py --selftest: {len(checks) - failures}/{len(checks)} checks passed")
    return 0 if failures == 0 else 1


# ---------------------------------------------------------------------------
# Main
# ---------------------------------------------------------------------------

def main() -> int:
    parser = argparse.ArgumentParser(
        description="Pure-Python Soundex/NYSIIS oracle (fuzzy-cli batch protocol)."
    )
    parser.add_argument("--mode", choices=["fixed", "original"], default="fixed",
                        help="soundex semantics: fixed (default, architecture "
                             "5.1) or original (pre-fix upstream: pad-to-size "
                             "always, ASCII-strict)")
    parser.add_argument("--selftest", action="store_true",
                        help="assert every binding data point and exit")
    args = parser.parse_args()

    if args.selftest:
        return selftest()

    data = sys.stdin.buffer.read()
    if not data:
        return 0
    lines = data.decode("utf-8", "replace").split("\n")
    if lines and lines[-1] == "":
        lines.pop()  # trailing newline after the last line
    out = []
    first_line = True
    for line in lines:
        if line.endswith("\r"):
            line = line[:-1]
        # Strip a stray BOM on the first line (environment quirk #10).
        if first_line and line.startswith("\ufeff"):
            line = line[1:]
        first_line = False
        out.append(handle_line(line, args.mode))
    sys.stdout.buffer.write(("\n".join(out) + "\n").encode("utf-8"))
    return 0


if __name__ == "__main__":
    sys.exit(main())
