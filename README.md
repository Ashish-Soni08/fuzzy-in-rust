# fuzzy-in-rust

**Port Mortem 2026 (Code Resurrection Hackathon), Track D: Python → Rust.**

A faithful port of the [`fuzzy`](https://github.com/yougov/fuzzy) phonetic-algorithms
library (**Soundex**, **NYSIIS**, **Double Metaphone**) from Python/Cython+C to
idiomatic, 100% safe Rust, with *proof* that it behaves identically, plus fixes for
two long-standing upstream bugs
([yougov/fuzzy#14](https://github.com/yougov/fuzzy/issues/14),
[yougov/fuzzy#15](https://github.com/yougov/fuzzy/issues/15)).

> **Note on the two READMEs.** `README.rst` is the original upstream readme, kept
> untouched as a historical artifact. This `README.md` is the face of the port:
> GitHub renders `README.md` in preference to `README.rst` when both exist in the
> repository root (Markdown is registered first in GitHub's markup renderer, and
> GitHub's README docs define the directory order `.github` → root → `docs`).

## What this is

The upstream `fuzzy` library (forked here as `Ashish-Soni08/fuzzy-in-rust`, base
commit `e15b195`) is ~1,600 lines: one Cython module (`src/fuzzy.pyx`: Soundex,
NYSIIS, and a Double Metaphone wrapper) plus the full Double Metaphone algorithm in
C (`src/double_metaphone.c`, 1,184 lines). This repository resurrects it as a safe
Rust workspace with PyO3 bindings that are a drop-in replacement for the original
Python module, and an equivalence-proof harness that compares the port against the
original code, not against a re-remembered spec.

**Bug policy:** the port fixes exactly the two known upstream bugs (#14 Soundex
padding, #15 Soundex non-ASCII input), implementing the behavior the original
project's own test suite and readme define as intended. Everything else is
replicated exactly, quirks included (for example, DMetaphone still raises
`UnicodeEncodeError` on non-ASCII input through the Python API, as the original
did). The original suite marks the bug-#14/#15 tests as non-strict `xfail`, so
against the fixed port they **XPASS** and the suite stays green.

## Repository layout

```
src/                  ORIGINAL Cython + C sources (untouched; ground truth)
test/                 ORIGINAL test directory (untouched)
tests/original/       original pytest suite, byte-identical (SHA-256 pinned at
                      kickoff) + SHA256SUMS.txt + KICKOFF.md provenance
rust/
  fuzzy-core/         the port: pure safe Rust, #![forbid(unsafe_code)], std-only
  fuzzy-cli/          batch-mode CLI over fuzzy-core (demo + fuzz driver)
  fuzzy-py/           PyO3 0.23 bindings exposing the Python module `fuzzy`
tools/
  oracle-c/           cl.exe harness over the ORIGINAL double_metaphone.c
  oracle_py.py        pure-Python oracle (Soundex/NYSIIS, fixed semantics)
  fuzz_diff.py        seeded differential fuzz driver
  verify_original_hashes.py
  build_pass_rates.py regenerates tools/reports/pass_rates.json from live runs
  vectors/            curated Double Metaphone vectors (validated vs the C oracle)
  reports/            committed evidence: fuzz campaigns, divergence report,
                      original-suite output, pass_rates.json
RULEBOOK.md           Cython/C → Rust translation rules + gap inventory
DECISIONS.md          equivalence proof, bug root-cause write-ups, trade-offs
DEMO.md               2–3 minute demo script (commands + expected output)
AGENTS.md             how AI agents executed this port (migration/harness lessons)
```

## Build and test

All commands are PowerShell, run from the repository root. Verified end-to-end on
Windows with Rust 1.97.1 (MSVC) and Python 3.13.7.

**Prerequisites**

- A Rust toolchain. On this machine cargo is not on the default PATH, so every
  cargo block below starts with the `$env:PATH` prefix; drop it if your cargo is
  already on PATH.
- A Python virtualenv with `maturin` and `pytest`. This workspace's venv lives at
  `G:\AI\Projects\Github\Code-Resurrection\.venv` (one level above the repo) and
  the commands below use it directly; on a fresh machine, create any venv with
  maturin + pytest installed and activate that instead. The port itself needs no
  third-party Python packages.
- MSVC Build Tools (`cl.exe`), only needed to rebuild the C oracle for the
  dmetaphone fuzz campaign.

### 1. Native Rust tests

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"
cargo test --manifest-path rust\Cargo.toml --workspace
```

Expected: **165 passed, 0 failed** across `fuzzy-core`, `fuzzy-cli`, and `fuzzy-py`.

### 2. Build and install the Python bindings

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\Activate.ps1"
Push-Location rust\fuzzy-py
maturin develop
Pop-Location
```

This builds the `fuzzy-py` crate and installs the Python module `fuzzy` into the
venv (editable).

### 3. Run the original test suite against the port

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" -m pytest tests/original -v
```

Expected: **2 passed, 3 xpassed** (exit code 0). The three XPASS results are the
two fixed upstream bugs; see "Known upstream bugs fixed" below.

### 4. Verify the preserved tests are still byte-identical

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" tools\verify_original_hashes.py
```

Re-hashes `tests/original/` and compares against the kickoff SHA-256
(`6DD19F9A…5510AE5` for `test_fuzzy.py`); exits 0 on match.

### 5. Lint and format gates

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"
cargo clippy --manifest-path rust\Cargo.toml --workspace --all-targets -- -D warnings
cargo fmt --manifest-path rust\Cargo.toml --all -- --check
```

Both exit 0: the workspace is clippy-clean with warnings denied and rustfmt-clean.

## Usage

### Rust library (`fuzzy-core`)

Add the crate (path dependency within this repo; std-only, no external crates):

```toml
[dependencies]
fuzzy-core = { path = "rust/fuzzy-core" }
```

```rust
use fuzzy_core::{dmetaphone, nysiis, soundex};

assert_eq!(soundex(4, "fuzzy"), "F200");
assert_eq!(nysiis("fuzzy"), "FASY");

let (primary, secondary) = dmetaphone("mayer").unwrap();
assert_eq!(primary, Some(b"MR".to_vec()));
assert_eq!(secondary, None); // secondary == primary collapses to None
```

API: `soundex(size, &str) -> String`, `nysiis(&str) -> String`,
`dmetaphone(&str)` / `dmetaphone_with_size(size, &str)` returning
`Result<(Option<Vec<u8>>, Option<Vec<u8>>), NonAsciiError>`, and
`dmetaphone_bytes(&[u8])` for the raw bytes-level algorithm (including the
Latin-1 arms that UTF-8 input can never reach).

### Python module (`import fuzzy`)

After `maturin develop` (step 2 above), the module is a drop-in replacement for
the original:

```python
>>> import fuzzy
>>> soundex = fuzzy.Soundex(4)
>>> soundex('fuzzy')
'F200'
>>> soundex('FancyFree')
'F521'
>>> dmeta = fuzzy.DMetaphone()
>>> dmeta('mayer')
[b'MR', None]
>>> fuzzy.nysiis('fuzzy')
'FASY'
```

Types match the original exactly: `Soundex(size)` returns `str`,
`DMetaphone(size=0)` returns a two-element list of `bytes`/`None`, `nysiis`
returns `str`.

### Command line (`fuzzy-cli`)

A batch-mode CLI over `fuzzy-core`, used by the fuzz driver and handy for
spot checks. It reads one `algo [size] word` per line on stdin
(`soundex <size> <word>`, `nysiis <word>`, `dmetaphone <size> <word>`), prints
exactly one result line per input (`<primary>|<secondary>` for dmetaphone, `-`
for `None`), prints `ERROR <message>` for malformed lines, and never aborts the
batch:

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"
cargo build --manifest-path rust\Cargo.toml -p fuzzy-cli
@('soundex 4 fuzzy','nysiis fuzzy','dmetaphone 0 mayer') | .\rust\target\debug\fuzzy-cli.exe
```

Output:

```text
F200
FASY
MR|-
```

## Known upstream bugs fixed

The port deliberately diverges from the original in exactly two behaviors, both
documented upstream bugs. One-paragraph root causes below; the full write-ups
(repro evidence, original-vs-fixed tables, upstream links) are in
[DECISIONS.md](DECISIONS.md).

### #14 — Soundex padding semantics ([yougov/fuzzy#14](https://github.com/yougov/fuzzy/issues/14))

**Root cause:** in `src/fuzzy.pyx`, `Soundex.__call__` ends with the loop
`for i from written <= i < self.size: out[i] = 48`, which unconditionally
right-pads the code with `'0'` characters up to `size`. That makes `size` a pad
target instead of a maximum length, so `Soundex(8)('Test')` returns `'T2300000'`
where fuzzy 1.1 (and the issue reporter, and the project's own xfail test)
expected `'T23'`. The port right-pads only when `size <= 4`, preserving classic
4-character Soundex (`Soundex(4)('fuzzy') == 'F200'`) and the original's
empty-input behavior (`Soundex(4)('') == '0000'`), and treats `size > 4` as a
pure maximum: `Soundex(8)('Test') == 'T23'`.

### #15 — Soundex non-ASCII input ([yougov/fuzzy#15](https://github.com/yougov/fuzzy/issues/15))

**Root cause:** `src/fuzzy.pyx` opens with the Cython directive
`# cython: c_string_type=unicode, c_string_encoding=ascii`. When
`Soundex.__call__` assigns the incoming Python `str` to a `char *`, Cython
encodes it as ASCII, so any non-ASCII input (the reporter's example:
`Soundex(8)('Jéroboam')`) dies with `UnicodeEncodeError` at the Python→C
boundary before the algorithm ever runs. NYSIIS in the very same file already
did the right thing in pure Python (`s.upper()`, then strip everything outside
`A–Z`). The port applies that same normalize-first rule to Soundex —
unicode-uppercase, then filter to `A–Z` — so `Soundex(8)('Jéroboam') == 'J615'`
instead of raising.

Both fixes are exactly what the original suite's three `xfail` tests assert, so
those tests now **XPASS** against the port. No other behavior diverges.

## Proof of equivalence

Three independent legs (full narrative and numbers in
[DECISIONS.md](DECISIONS.md); machine-readable source:
[tools/reports/pass_rates.json](tools/reports/pass_rates.json)):

| Leg | What runs | Result |
|---|---|---|
| 1. Original suite, unmodified | `pytest tests/original` (byte-identical file, SHA-256 pinned at kickoff) against the PyO3 module | **2 passed, 3 xpassed** |
| 2. Native Rust tests | `cargo test --workspace` reimplementing the spec 1:1 | **165 passed, 0 failed** |
| 3. Differential fuzzing | `tools/fuzz_diff.py`, seed `20260803`, committed reports | **150,000 cases, 0 mismatches** |

Leg 3 compares the port against ground truth, per algorithm:

| Algorithm | Compared against | Cases | Mismatches | Committed report |
|---|---|---|---|---|
| soundex | `tools/oracle_py.py` (fixed semantics) | 50,000 | 0 | `tools/reports/fuzz_soundex_20260803_50000.json` |
| nysiis | `tools/oracle_py.py` (exact transcription) | 50,000 | 0 | `tools/reports/fuzz_nysiis_20260803_50000.json` |
| dmetaphone | the compiled ORIGINAL C code (`tools/oracle-c`) | 50,000 | 0 | `tools/reports/fuzz_dmetaphone_20260803_50000.json` |

**Intentional divergences, honestly counted.** The only behavioral differences
from the original are the two bug fixes. The committed divergence report
(`tools/reports/divergence_soundex_20260803_50000.json`) records **14,187**
divergent cases out of 50,000 against original Soundex semantics, and every one
falls into a sanctioned class: **11,605** from the #14 padding fix and **2,582**
from the #15 non-ASCII fix, **0** unclassified. NYSIIS and DMetaphone have zero
divergence from the original.

### Reproduce the fuzz campaigns

```powershell
$py = "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe"
& $py tools\fuzz_diff.py --algo soundex    --count 50000 --out $env:TEMP\fuzzval
& $py tools\fuzz_diff.py --algo nysiis     --count 50000 --out $env:TEMP\fuzzval
& $py tools\fuzz_diff.py --algo dmetaphone --count 50000 --out $env:TEMP\fuzzval
```

Each exits 0 with `mismatch_count: 0` and writes a report into `$env:TEMP\fuzzval`
(`--out` keeps the committed reports untouched; drop it to regenerate
`tools/reports/` in place — the generator is seeded and deterministic). The
dmetaphone campaign needs the C oracle, built once from the original source:

```powershell
powershell -NoProfile -ExecutionPolicy Bypass -File .\tools\oracle-c\build_oracle.ps1
```

The divergence evidence (bug fixes vs original semantics) is reproducible with:

```powershell
& $py tools\fuzz_diff.py --algo soundex --mode original --count 50000 --out $env:TEMP\fuzzval
```

This one exits **non-zero by design**: the 14,187 mismatches ARE the bug-fix
evidence. To regenerate the aggregate numbers in
`tools/reports/pass_rates.json` from live runs, run
`python tools\build_pass_rates.py` (pass `--out <dir>` to write elsewhere).

## Documentation map

- [DEMO.md](DEMO.md) — the 2–3 minute demo script, commands plus expected output.
- [DECISIONS.md](DECISIONS.md) — equivalence-proof narrative, fuzz statistics,
  bug #14/#15 root-cause write-ups, trade-offs, honest pass rates.
- [RULEBOOK.md](RULEBOOK.md) — the Cython/C → Rust translation rulebook and gap
  inventory that the port was built against.
- [AGENTS.md](AGENTS.md) — how AI agents executed this port: the migration
  playbook and long-running-harness lessons applied.
- [tests/original/](tests/original) — the preserved original test suite
  (byte-identical, SHA-256 pinned) with kickoff provenance.
- [tools/reports/](tools/reports) — committed evidence: fuzz campaign reports,
  the divergence report, the original-suite output, and `pass_rates.json`.

## License

MIT (see [LICENSE](LICENSE)), inherited from the upstream `fuzzy` project.
