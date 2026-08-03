# fuzz/ — differential fuzz campaigns

This directory is the entry point for the project's **differential fuzzing**:
the Rust port (`fuzzy-cli`, batch stdin protocol) is compared line-by-line
against ground-truth oracles over a seeded, deterministic corpus.

- `harness.py` — the runnable entry point. A thin wrapper that forwards argv
  to the pinned differential engine, [`tools/fuzz_diff.py`](../tools/fuzz_diff.py),
  and propagates its exit code (0 iff zero mismatches).
- `log.txt` — the human-readable log of the 60-second Differential Fuzz
  Survivor run (one continuous dmetaphone campaign, zero divergences).

## Running a campaign

From the repo root:

```powershell
# dmetaphone: Rust port vs the compiled ORIGINAL C oracle (tools/oracle-c/dmoracle.exe)
python fuzz/harness.py --algo dmetaphone --count 50000

# soundex / nysiis: Rust port vs the pure-Python oracle (tools/oracle_py.py)
python fuzz/harness.py --algo soundex --count 50000
python fuzz/harness.py --algo nysiis --count 50000

# negative control (must exit non-zero): proves the harness detects mismatches
python fuzz/harness.py --algo dmetaphone --selftest --out $env:TEMP\fuzzval
```

Useful flags (all defined by `tools/fuzz_diff.py`): `--seed S` (default
`20260803`), `--out <dir>` (default `tools/reports`; pass a `%TEMP%` dir for
throwaway runs so the repo stays clean), `--mode {fixed|original}` (soundex
only; `original` reproduces the pre-fix upstream semantics for the
documented-divergence report).

## Reports

Every run writes `fuzz_<algo>_<seed>_<count>.json` to the output directory
with the pinned schema: `algo, seed, cases, corpora{...}, mismatch_count,
mismatches[...], elapsed_s, timestamp`. Committed campaign reports live in
[`tools/reports/`](../tools/reports/) — see `fuzz_dmetaphone_20260803_*.json`
for the zero-divergence dmetaphone campaigns, including the >=60s survivor
run documented in `log.txt`.

The generator is deterministic: the same seed and count reproduce the
identical corpus, and a smaller `--count` with the same seed replays a prefix
of a larger run.
