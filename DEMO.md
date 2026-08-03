# DEMO.md — 2–3 minute demo script

**Port Mortem 2026, Track D: `fuzzy` Python → Rust.** A timed walkthrough of the
port and its proof. **Total runtime: about 2–3 minutes** (section times below sum
to ~2 min 50 s of presenting; the commands themselves take under a minute on this
machine).

**Setup (already done once, not part of the demo):** the Rust workspace is built,
`maturin develop` has installed the PyO3 module `fuzzy` into the venv, and the C
oracle is compiled. All commands are PowerShell, run from the repository root
`G:\AI\Projects\Github\Code-Resurrection\fuzzy-in-rust`, and match `services.yaml`.
Every expected-output block below is a real capture from this machine (2026-08-03);
only elapsed-time values will vary run to run.

---

## 1. Intro — 0:00–0:15 (talking, no command)

Say: "This is `yougov/fuzzy` — Soundex, NYSIIS, and Double Metaphone — resurrected
as 100% safe Rust with PyO3 bindings that are a drop-in replacement for the
original Python module. It also fixes two long-standing upstream bugs, and it
proves equivalence against the original code, not a re-remembered spec. Three
quick proofs: the test suites, the CLI, and 150,000 fuzz cases."

## 2. Build the workspace — 0:15–0:35

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"; Set-Location rust; cargo build --workspace
```

Expected (exit code 0; warm cache, so only the changed crate recompiles — on a
fully warm cache just the `Finished` line prints):

```text
   Compiling fuzzy-py v0.1.0 (G:\AI\Projects\Github\Code-Resurrection\fuzzy-in-rust\rust\fuzzy-py)
    Finished `dev` profile [unoptimized + debuginfo] target(s) in 7.02s
```

Say: "Three crates: `fuzzy-core` — the port, pure safe Rust with
`#![forbid(unsafe_code)]`; `fuzzy-cli` — a batch driver; `fuzzy-py` — the PyO3
bindings."

## 3. Native Rust tests — 0:35–1:05

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"; Set-Location rust; cargo test --workspace
```

Expected (exit code 0; abridged to the per-target summaries — **165 passed,
0 failed** in total):

```text
test result: ok. 20 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # fuzzy-cli tests/cli.rs
test result: ok. 93 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # fuzzy-core tests/dmetaphone.rs
test result: ok. 30 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # fuzzy-core tests/nysiis.rs
test result: ok. 22 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out   # fuzzy-core tests/soundex.rs
```

Say: "Every binding data point of the spec is a native test, and the 93-case
Double Metaphone suite is data-driven from vectors pre-validated against the
compiled original C."

## 4. `fuzzy-cli` demo words — 1:05–1:35

The CLI reads `algo [size] word` lines on stdin and prints one result per line
(`<primary>|<secondary>` for dmetaphone, `-` for `None`). The UTF-8 preamble is
required on Windows PowerShell 5.1 so `Jéroboam` survives the pipe:

```powershell
$OutputEncoding = New-Object System.Text.UTF8Encoding($false); [Console]::OutputEncoding = New-Object System.Text.UTF8Encoding($false)
@('soundex 4 fuzzy','soundex 8 Test','soundex 8 Jéroboam','nysiis fuzzy','dmetaphone 0 mayer') | .\rust\target\debug\fuzzy-cli.exe
```

Expected (exit code 0):

```text
F200
T23
J615
FASY
MR|-
```

Say: "`F200` is the readme's classic example. `T23` and `J615` are the two bug
fixes: the original pads `Soundex(8)('Test')` to `T2300000` (bug #14) and raises
`UnicodeEncodeError` on `Jéroboam` (bug #15); the port returns what the original
project's own tests intend. `MR|-` is Double Metaphone on `mayer` — the secondary
code equals the primary, so it collapses to `None`, exactly like the original."

## 5. The original pytest suite, unmodified — 1:35–2:05

The original `test/test_fuzzy.py` is preserved byte-identical at
`tests/original/test_fuzzy.py` (SHA-256 pinned at kickoff) and runs against the
installed PyO3 module (`$env:NO_COLOR='1'` just keeps the capture free of ANSI
color escapes on this machine):

```powershell
$env:NO_COLOR='1'; & "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" -m pytest tests/original/ -v
```

Expected (exit code 0 — **2 passed, 3 xpassed**):

```text
============================= test session starts =============================
platform win32 -- Python 3.13.7, pytest-9.1.1, pluggy-1.6.0 -- G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe
cachedir: .pytest_cache
rootdir: G:\AI\Projects\Github\Code-Resurrection\fuzzy-in-rust
configfile: pytest.ini
collecting ... collected 5 items

tests/original/test_fuzzy.py::test_soundex_does_not_mutate_strings PASSED [ 20%]
tests/original/test_fuzzy.py::test_soundex_result XPASS (issue #14)      [ 40%]
tests/original/test_fuzzy.py::test_soundex_Test XPASS (issue #14)        [ 60%]
tests/original/test_fuzzy.py::test_soundex_non_ascii XPASS (issue #15)   [ 80%]
tests/original/test_fuzzy.py::test_DMetaphone PASSED                     [100%]

======================== 2 passed, 3 xpassed in 0.21s =========================
```

Say: "Two pass outright. The three XPASS are the point: the upstream suite marks
the bug-#14/#15 tests non-strict `xfail`, and because the port implements the
behavior those tests intend, they XPASS — the suite stays green and the fixes are
visible." (The `0.21s` varies; the committed capture in
`tools/reports/original_suite_output.txt` shows the same counts.)

## 6. Fuzz report summary — 2:05–2:35

The three committed differential-fuzz campaigns (seed `20260803`, 50,000 cases
each) — summarized jq-free, straight from the committed JSON reports:

```powershell
Get-Content tools\reports\fuzz_soundex_20260803_50000.json, tools\reports\fuzz_nysiis_20260803_50000.json, tools\reports\fuzz_dmetaphone_20260803_50000.json -Raw | ConvertFrom-Json | Select-Object algo, cases, mismatch_count, elapsed_s | Format-Table -AutoSize
```

Expected:

```text
algo       cases mismatch_count elapsed_s
----       ----- -------------- ---------
soundex    50000              0     0.966
nysiis     50000              0     8.204
dmetaphone 50000              0     0.785
```

Then the honesty beat — the intentional divergences from *original* Soundex
semantics (the two bug fixes), counted by the committed divergence report:

```powershell
$d = Get-Content tools\reports\divergence_soundex_20260803_50000.json -Raw | ConvertFrom-Json; "divergence report: $($d.mismatch_count) of $($d.cases) cases differ from ORIGINAL soundex semantics (all inside bug fixes #14/#15)"
```

Expected:

```text
divergence report: 14187 of 50000 cases differ from ORIGINAL soundex semantics (all inside bug fixes #14/#15)
```

Say: "150,000 cases, zero mismatches — dmetaphone against the compiled original C
itself, soundex and nysiis against a statement-exact Python oracle. And the only
divergences from the original anywhere are the two documented bug fixes: 14,187
cases, every one inside the sanctioned classes, zero unclassified."

## 7. Wrap — 2:35–2:50 (talking, no command)

Say: "Full proof narrative, bug root-cause write-ups, and trade-offs are in
`DECISIONS.md`; the translation rulebook is `RULEBOOK.md`; how AI agents executed
the port is `AGENTS.md`. Everything you saw is reproducible from `services.yaml`."

---

### If you have extra time (optional, not counted in the 2–3 minutes)

Re-run a fuzz campaign live against a temp dir (keeps the committed reports
untouched; deterministic generator, exits 0 with `mismatch_count: 0`):

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" tools\fuzz_diff.py --algo soundex --count 5000 --out $env:TEMP\fuzzdemo
```

Verify the preserved original tests are still byte-identical (exits 0):

```powershell
& "G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe" tools\verify_original_hashes.py
```
