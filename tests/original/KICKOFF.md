# KICKOFF — Original Test Suite Preservation

This directory preserves the original `fuzzy` test suite **byte-identical** as the
test-parity proof for the Port Mortem 2026 hackathon (Track D, Python → Rust).

## Provenance

- **Source repository**: `yougov/fuzzy`, forked as `Ashish-Soni08/fuzzy-in-rust`
- **Source commit (upstream base)**: `e15b195467223a684a26fadb53997bf6f36be2c4` (branch `master`, HEAD at mission start)
- **Original path**: `test/test_fuzzy.py` (untouched; see mission rules)
- **Preserved path**: `tests/original/test_fuzzy.py` (this directory)
- **Kickoff**: 2026-07-31T18:00:00Z (unix 1785522000)
- **Preserved at**: 2026-08-03T03:52:26Z

## Pinned hash

```
SHA-256(test_fuzzy.py) = 6DD19F9A38F848001D990CCB3745213A60EFBB36A11293642F1B3BDBD5510AE5
```

Recorded in `SHA256SUMS.txt` (standard `<hash>  <filename>` format) and verified by
`tools/verify_original_hashes.py`.

## Why these files are immutable

The pinned SHA-256 is the hackathon's proof that the ported library runs the
**original, unmodified** test suite. The hash was recorded at kickoff
(2026-07-31) before any porting work began, and it is re-verified at every
milestone gate and by the judges. Any modification — even whitespace or
line-ending normalization — would change the hash and invalidate the parity
proof. Accordingly:

- `tests/original/test_fuzzy.py` must never be edited, reformatted, or re-saved.
- A root `.gitattributes` rule (`-text`) disables git line-ending conversion for
  the file so it stays byte-identical in the repo and on every checkout.
- The original `src/` and `test/` directories are likewise never modified.

To verify integrity at any time:

```powershell
& G:\AI\Projects\Github\Code-Resurrection\.venv\Scripts\python.exe tools\verify_original_hashes.py
```

Exit code 0 means every preserved file matches its pinned hash.
