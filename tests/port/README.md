# tests/port — port-team tests (NEW)

These are **new tests written by the port team** for the Rust port of `fuzzy`
(Port Mortem 2026, Track D). They are **not** the preserved original suite —
that lives untouched in [`tests/original/`](../original/) (byte-identical,
SHA-256 pinned at kickoff; see `tests/original/KICKOFF.md` and
`tools/verify_original_hashes.py`).

## What is here

- `test_port_api.py` — a pytest suite exercising the installed Python module
  `fuzzy` (the PyO3 adapter over the Rust core, installed into the venv with
  `maturin develop`). It pins:
  - the two **fixed upstream bugs**:
    [yougov/fuzzy#14](https://github.com/yougov/fuzzy/issues/14) —
    `Soundex(8)('Test') == 'T23'` (size > 4 is a maximum, not a pad target);
    [yougov/fuzzy#15](https://github.com/yougov/fuzzy/issues/15) —
    `Soundex(8)('Jéroboam') == 'J615'` (non-ASCII input no longer raises);
  - anchor data points shared with the original README/tests
    (`Soundex(4)('fuzzy') == 'F200'`, `nysiis('fuzzy') == 'FASY'`,
    `DMetaphone()('mayer') == [b'MR', None]`);
  - **type fidelity** — `DMetaphone` returns a `list` of two elements, each
    `bytes` or `None`;
  - **error fidelity** — `Soundex(-1)` / `DMetaphone(-1)` raise
    `ValueError`; `DMetaphone()('Jéroboam')` raises `UnicodeEncodeError`
    (original Double Metaphone ASCII-only behavior preserved);
    `Soundex('fuzzy')` / `Soundex()` raise `TypeError`;
  - the **size-0 edge cases** — `Soundex(0)('fuzzy') == ''`;
    `DMetaphone(0)` means unlimited (the core 4-char cap still applies).

## Running

From the repo root, with the venv interpreter (the one `maturin develop`
installed `fuzzy` into):

```powershell
& G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe -m pytest tests/port -v
```

Expected: all tests pass (14 passed).

## The native Rust suites

The bulk of the port's test coverage is **native Rust**, not Python:

- `rust/fuzzy-core/tests/` — **145 integration tests** (`soundex.rs`,
  `nysiis.rs`, `dmetaphone.rs`), including every binding data point from the
  behavioral spec and a data-driven test over
  `tools/vectors/dmetaphone_vectors.json` (each vector validated against the
  compiled original C oracle);
- `rust/fuzzy-cli/tests/` — **20 integration tests** (`cli.rs`) covering the
  pinned batch line protocol.

Run them from the repo root with:

```powershell
$env:PATH = "C:\Users\Lenovo\.cargo\bin;$env:PATH"
cargo test --workspace --manifest-path rust\Cargo.toml
```

Expected: 165 passed, 0 failed (145 fuzzy-core + 20 fuzzy-cli integration
tests; the crates have no unit-test modules of their own).

See also: the preserved original suite (`tests/original/`, expected
`2 passed, 3 xpassed` — the 3 xpasses are the intentional bug fixes) and the
differential-fuzz evidence in `tools/reports/`.
