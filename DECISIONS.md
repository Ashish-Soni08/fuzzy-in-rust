# DECISIONS.md — Equivalence Proof, Bug Fixes, and Trade-offs

**Port Mortem 2026 (Code Resurrection Hackathon), Track D: Python → Rust.**
Port of [`yougov/fuzzy`](https://github.com/yougov/fuzzy) (fork
`Ashish-Soni08/fuzzy-in-rust`, base commit `e15b195`) to safe Rust.
Written 2026-08-03. Every number in this document comes from a committed,
machine-checkable report under `tools/reports/` (aggregated in
[`tools/reports/pass_rates.json`](tools/reports/pass_rates.json)) or from a
command quoted below. Nothing is estimated.

---

## 1. Proof of Equivalence — three independent legs

The port is not trusted because we say so. It is trusted because three
independent checks, each grounded in the *original code* rather than in a
re-remembered spec, all agree.

### Leg 1 — the original test suite, unmodified

`tests/original/test_fuzzy.py` is a byte-identical copy of the upstream
`test/test_fuzzy.py`, pinned at kickoff by SHA-256
(`6DD19F9A38F848001D990CCB3745213A60EFBB36A11293642F1B3BDBD5510AE5`,
see `tests/original/SHA256SUMS.txt` and `tests/original/KICKOFF.md`;
`python tools\verify_original_hashes.py` re-verifies it). It runs against the
PyO3 module `fuzzy` built from the Rust port:

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" -m pytest tests/original/ -v
```

Result (committed capture: `tools/reports/original_suite_output.txt`):

```text
tests/original/test_fuzzy.py::test_soundex_does_not_mutate_strings PASSED [ 20%]
tests/original/test_fuzzy.py::test_soundex_result XPASS (issue #14)      [ 40%]
tests/original/test_fuzzy.py::test_soundex_Test XPASS (issue #14)        [ 60%]
tests/original/test_fuzzy.py::test_soundex_non_ascii XPASS (issue #15)   [ 80%]
tests/original/test_fuzzy.py::test_DMetaphone PASSED                     [100%]

======================== 2 passed, 3 xpassed in 0.13s =========================
```

**2 passed, 3 xpassed**, exit code 0. The three XPASS results are the two
fixed upstream bugs (sections 3 and 4): the suite marks those tests non-strict
`xfail`, and the port implements the behavior the tests intend, so they XPASS
and the suite stays green.

### Leg 2 — native Rust tests

`cargo test --workspace` (from `rust/`): **165 passed, 0 failed**, exit code 0.
Every binding data point of the behavioral spec (architecture.md §5) is a
`#[test]`, and the Double Metaphone suite is data-driven from
`tools/vectors/dmetaphone_vectors.json` — every vector pre-validated against
the compiled original C code.

| Crate | Test target | Passed | Failed |
|---|---|---|---|
| fuzzy-core | `tests/soundex.rs` | 22 | 0 |
| fuzzy-core | `tests/nysiis.rs` | 30 | 0 |
| fuzzy-core | `tests/dmetaphone.rs` | 93 | 0 |
| fuzzy-cli | `tests/cli.rs` | 20 | 0 |
| fuzzy-py | (bindings; verified via leg 1) | 0 | 0 |
| **Total** | | **165** | **0** |

### Leg 3 — differential fuzzing against ground truth

`tools/fuzz_diff.py` drives the Rust port (`fuzzy-cli` batch mode) against an
oracle over a seeded, deterministic corpus (seed `20260803`, 50,000 cases per
algorithm — the architecture §7 minimum for the final report):

- **dmetaphone**: vs `tools/oracle-c/dmoracle.exe` — the *original*
  `src/double_metaphone.c` compiled with MSVC. Ground truth is the original
  code itself, not a reimplementation.
- **soundex / nysiis**: vs `tools/oracle_py.py`, a statement-for-statement
  Python transcription of `src/fuzzy.pyx` (Soundex in its fixed semantics per
  architecture §5.1; NYSIIS exact, quirks included).

**150,000 total cases, 0 mismatches.** Full table in section 2. The harness
carries a negative control (`--selftest` injects a deliberately wrong
expectation and must exit non-zero), proving it can detect mismatches, and the
generator is deterministic: any validator can re-run with the same seed and
reproduce the identical corpus prefix.

A fourth, deliberately *failing* run documents the intentional divergences
(Rust-fixed vs original Soundex semantics) — see section 5.

---

## 2. Fuzz statistics

Committed reports (seed `20260803` for all three campaigns), aggregated in
`tools/reports/pass_rates.json`:

| Algorithm | Cases | Seed | Mismatches | Elapsed | Committed report |
|---|---:|---:|---:|---:|---|
| soundex | 50,000 | 20260803 | **0** | 0.966 s | `tools/reports/fuzz_soundex_20260803_50000.json` |
| nysiis | 50,000 | 20260803 | **0** | 8.204 s | `tools/reports/fuzz_nysiis_20260803_50000.json` |
| dmetaphone | 50,000 | 20260803 | **0** | 0.785 s | `tools/reports/fuzz_dmetaphone_20260803_50000.json` |
| **Total** | **150,000** | — | **0** | — | — |

Corpus composition per campaign (counts from the committed reports; the
generator mixes eight seeded categories — dmetaphone uses the seven ASCII-only
ones, since non-ASCII input is an `ERROR` by design through the Python-facing
API and its Latin-1 byte arms are covered by native `dmetaphone_bytes` tests):

| Corpus category | soundex | nysiis | dmetaphone |
|---|---:|---:|---:|
| empty | 502 | 531 | 522 |
| single_char | 1,513 | 1,548 | 1,639 |
| ascii_words | 21,272 | 21,348 | 22,612 |
| name_like | 12,690 | 12,809 | 13,624 |
| mixed_case | 6,243 | 6,165 | 6,231 |
| digits_punct | 4,140 | 4,106 | 4,310 |
| unicode | 2,582 | 2,434 | — |
| very_long | 1,058 | 1,059 | 1,062 |

Reproduce (each exits 0 with `mismatch_count: 0`; `--out` keeps the committed
reports untouched):

```powershell
$py = "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe"
& $py tools\fuzz_diff.py --algo soundex    --count 50000 --out $env:TEMP\fuzzval
& $py tools\fuzz_diff.py --algo nysiis     --count 50000 --out $env:TEMP\fuzzval
& $py tools\fuzz_diff.py --algo dmetaphone --count 50000 --out $env:TEMP\fuzzval
```

---

## 3. Bug #14 — Soundex pads to `size` unconditionally

Upstream issue: [yougov/fuzzy#14](https://github.com/yougov/fuzzy/issues/14)

**Root cause.** In `src/fuzzy.pyx`, `Soundex.__call__` ends with:

```cython
for i from written <= i < self.size:
    out[i] = 48
out[self.size] = 0
```

This loop right-pads the output buffer with `'0'` (ASCII 48) from the last
written position up to `size` — **unconditionally**. That makes `size` a pad
target instead of a maximum length, so `Soundex(8)('Test')` returns
`'T2300000'`: the classic Soundex code for "Test" is `T23`, and the extra five
zeros are pure padding noise. The reporter (and fuzzy 1.1) expected `size` to
behave as a maximum code length. The project's own test suite encodes that
intent: `test_soundex_Test` asserts `Soundex(8)('Test') == 'T23'` and is marked
`xfail(reason="issue #14")` — the intended result is literally `'T23'`.

**Repro — original vs fixed.** Original behavior replicated by
`tools/oracle_py.py --mode original` (a statement-exact transcription of the
Cython, ASCII-strict and pad-always); fixed behavior from the Rust port
(`fuzzy-cli` and the installed PyO3 module agree):

| Call | Original (bug) | Port (fixed) |
|---|---|---|
| `Soundex(8)('Test')` | `'T2300000'` | `'T23'` |
| `Soundex(8)('p')` | `'P0000000'` | `'P'` |
| `Soundex(8)('qcQyFZu')` | `'Q2120000'` | `'Q212'` |
| `Soundex(5)('uixyoxfq')` | `'U2120'` | `'U212'` |
| `Soundex(4)('fuzzy')` | `'F200'` | `'F200'` (unchanged) |
| `Soundex(4)('')` | `'0000'` | `'0000'` (unchanged) |

**The fix.** Pad with `'0'` to exactly `size` only when `size <= 4` —
preserving classic 4-character Soundex and the original's empty-input behavior
(`Soundex(4)('') == '0000'`) — and treat `size > 4` as a pure maximum length
with no padding. This is precisely what the xfail tests assert, so against the
port `test_soundex_result` and `test_soundex_Test` XPASS.

---

## 4. Bug #15 — Soundex raises `UnicodeEncodeError` on non-ASCII input

Upstream issue: [yougov/fuzzy#15](https://github.com/yougov/fuzzy/issues/15)

**Root cause.** `src/fuzzy.pyx` opens with the Cython directive:

```cython
# cython: c_string_type=unicode, c_string_encoding=ascii
```

With `c_string_encoding=ascii`, every assignment of a Python `str` to a
`char *` — `cs = s` in `Soundex.__call__` — ASCII-encodes the string at the
Python→C boundary. Any non-ASCII character (the reporter's example:
`Soundex(8)('Jéroboam')`) dies there with `UnicodeEncodeError`, before the
Soundex algorithm ever runs. The bug is not in the algorithm at all; it is in
the Cython string marshalling. Notably, `nysiis()` in the very same file never
had this problem: it normalizes in pure Python first — `s.upper()`
(Unicode-aware), then strips everything outside `A–Z` — so accented input
degrades gracefully instead of raising.

**Repro — original vs fixed.**

| Call | Original (bug) | Port (fixed) |
|---|---|---|
| `Soundex(8)('Jéroboam')` | raises `UnicodeEncodeError` | `'J615'` |
| `Soundex(3)('Ünmu')` | raises `UnicodeEncodeError` | `'N50'` |
| `Soundex(5)('Guðmundur')` | raises `UnicodeEncodeError` | `'G536'` |

(Original column via `tools/oracle_py.py --mode original`, which replicates the
ASCII-strict boundary; fixed column verified live through both `fuzzy-cli` and
the installed PyO3 module: `fuzzy.Soundex(8)('Jéroboam')` returns `'J615'`.)

**The fix.** Mirror exactly what NYSIIS already does: normalize with
Unicode-uppercase (Rust `str::to_uppercase()`, equivalent to Python
`str.upper()`), then filter to ASCII `A–Z`, dropping everything else. The
Soundex algorithm then runs on the filtered letters. This is precisely what
`test_soundex_non_ascii` asserts (`Soundex(8)('Jéroboam') == 'J615'`), so the
test XPASSes against the port. Scope discipline: only Soundex is fixed.
DMetaphone still raises `UnicodeEncodeError` on non-ASCII input through the
Python API, exactly as the original did — no test or readme defines an
alternate intent for it.

---

## 5. Intentional divergences — exactly two classes

The port diverges from the original's *observable* behavior in exactly the two
sanctioned bug-fix classes above, and nowhere else. This is not asserted; it is
measured. The committed divergence report
`tools/reports/divergence_soundex_20260803_50000.json` fuzzes the fixed Rust
port against **original** Soundex semantics (same seed `20260803`, same 50,000
cases as the leg-3 campaign):

- **14,187** divergent cases out of 50,000, and **every one** falls into a
  sanctioned class:
  - **11,605** × class `soundex_size_gt4_padding_bug_14` (pad-to-`size`
    removed for `size > 4`),
  - **2,582** × class `soundex_non_ascii_input_bug_15` (non-ASCII input
    normalized instead of raising),
  - **0** unclassified.
- NYSIIS and DMetaphone have **zero** divergence from the original (their leg-3
  fuzz reports in section 2 show `mismatch_count: 0` against exact
  transcriptions / the original C).

The classification is deterministic: a re-run of the seeded corpus through
`fuzzy-cli` and `oracle_py --mode original`, where `expected == "ERROR
non-ASCII input…"` maps to #15 and `expected == actual` right-padded with
`'0'` maps to #14; anything else would be unclassified (none occurred). The
report records the first 100 mismatch entries verbatim
(`MAX_RECORDED_MISMATCHES` cap in `tools/fuzz_diff.py`); `mismatch_count` is
the authoritative total.

Reproduce the divergence evidence (exits **non-zero by design** — the
mismatches ARE the bug-fix evidence):

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" tools\fuzz_diff.py --algo soundex --mode original --count 50000 --out $env:TEMP\fuzzval
```

---

## 6. Pass rates and known limitations

Honest scoreboard (source: `tools/reports/pass_rates.json`, regenerated from
live runs by `tools/build_pass_rates.py`, which fails loudly rather than write
an unverifiable number):

| Surface | Result |
|---|---|
| Original suite (unmodified, hash-pinned) | **2 passed, 3 xpassed**, 0 failed, 0 errors, exit 0 |
| Native Rust tests (`cargo test --workspace`) | **165 passed, 0 failed** |
| Differential fuzz (3 campaigns × 50,000) | **150,000 cases, 0 mismatches** |
| Intentional divergences vs original Soundex | 14,187 cases, 100% inside the 2 sanctioned bug-fix classes, 0 unclassified |

The pass rate is 100% on every surface **given the two documented bug fixes**;
there is no hidden failure category. Known limitations, disclosed:

1. **DMetaphone Latin-1 arms are unreachable via the Python API.** The C code
   has case labels for bytes `0xC7` ('Ç' → S) and `0xD1` ('Ñ' → N). Python
   `str` input is UTF-8 and DMetaphone rejects non-ASCII by design, so those
   raw bytes can never arrive through `import fuzzy`. The arms are ported for
   fidelity and covered natively through `dmetaphone_bytes` (`latin1` tests).
2. **Words containing spaces are not expressible in the `fuzzy-cli` batch
   protocol** (the word field is one whitespace-delimited token), so the CLI
   fuzz corpora cannot include them; whitespace-in-word behavior is covered by
   native Rust tests instead.
3. **DMetaphone non-ASCII input raises `UnicodeEncodeError` through the Python
   API by design** — original behavior preserved (only Soundex bugs #14/#15
   are sanctioned fixes).
4. **Soundex intentionally diverges from the original in exactly the two
   bug-fix classes** (section 5) — these are the 3 XPASS results in the
   original suite and the 14,187 cases in the committed divergence report.
5. **The divergence report records at most 100 mismatch entries**
   (`MAX_RECORDED_MISMATCHES` cap); `mismatch_count` (14,187) is the
   authoritative total, and the per-class counts come from a deterministic
   re-run of the seeded corpus.

---

## 7. Trade-offs

Decisions where a faithful port pulled against idiom, and what we chose:

- **Bytes-level Double Metaphone API.** The C algorithm is byte-oriented
  (`char *`, Latin-1 case labels), so `fuzzy-core` exposes
  `dmetaphone(&str) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), NonAsciiError>`
  plus a raw `dmetaphone_bytes(&[u8])` entry point, instead of pretending the
  codes are UTF-8 `String`s. Cost: Rust callers handle `Vec<u8>`. Benefit: the
  port is byte-faithful to the C (including the unreachable-from-Python Latin-1
  arms), and the PyO3 layer returns `bytes` exactly like the original module.
- **The simplified Soundex is preserved, not "improved".** The original
  implements a simplified dedup (append digit `d` iff `written == 1` or
  `last_written != d`; no H/W adjacency special-casing from classic Soundex).
  We replicated it exactly — including the `written == 1` bypass — because the
  goal is equivalence with *this* library, not with a textbook. The fuzz
  campaigns (50,000 cases, 0 mismatches) validate that choice.
- **No `unsafe`, anywhere in the port.** `fuzzy-core` and `fuzzy-cli` are
  `#![forbid(unsafe_code)]`; `fuzzy-py` contains no handwritten raw blocks of
  its own (only PyO3 0.23 macro internals). The C metastring (`MetaphAdd` /
  `GetAt` / `SetAt` over raw buffers) became a bounds-checked `Vec<u8>` model
  with the C's out-of-range-returns-`0` semantics replicated in safe code.
  Cost: some C idioms (pointer lookahead into 5-space padding) became index
  arithmetic with explicit bounds behavior. Benefit: memory safety is enforced
  by the compiler, and `cargo clippy --workspace --all-targets -- -D warnings`
  plus `cargo fmt --check` are both clean.
- **CLI batch limitations.** `fuzzy-cli` trades generality for fuzz
  throughput: one whitespace-delimited word token per line, no quoting, no
  spaces inside words, `ERROR <message>` lines instead of exceptions, and the
  process never aborts a batch on a bad line. That is deliberate — the CLI
  exists to drive 150,000-case differential campaigns over stdin, and the
  expressiveness gap (spaces in words) is covered by native tests.
- **Defensive `size` validation.** The Rust API takes `usize`; the PyO3 layer
  rejects a negative `size` with `ValueError`. The original C `int` had no
  validation (undefined behavior territory). This is a divergence only in
  input the original never defined.
- **README.md / README.rst coexistence.** The repo root now carries both:
  `README.md` (the submission readme, written for this port) and the original
  upstream `README.rst`, kept byte-untouched as a historical artifact. This is
  safe because GitHub renders `README.md` first when both exist in the
  repository root (Markdown is registered before reStructuredText in GitHub's
  markup renderer), so the port's readme is the face of the repo while the
  upstream file remains preserved.

---

## Evidence index

| Artifact | What it proves |
|---|---|
| `tests/original/` + `SHA256SUMS.txt` + `KICKOFF.md` | Original tests preserved byte-identical (SHA-256 pinned at kickoff) |
| `tools/reports/original_suite_output.txt` | Leg 1: 2 passed, 3 xpassed |
| `tools/reports/fuzz_{soundex,nysiis,dmetaphone}_20260803_50000.json` | Leg 3: 50,000 cases each, 0 mismatches |
| `tools/reports/divergence_soundex_20260803_50000.json` | Section 5: 14,187 divergences, all in the 2 sanctioned classes |
| `tools/reports/pass_rates.json` | The aggregate scoreboard quoted in sections 1, 2, and 6 |
| `RULEBOOK.md` | The translation rules and gap inventory the port was built against |
| `DEMO.md` | The 2–3 minute live demo script |
