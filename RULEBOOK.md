# RULEBOOK.md — Translation Rulebook & Gap Inventory

**Project:** `fuzzy` (yougov/fuzzy @ `e15b195467223a684a26fadb53997bf6f36be2c4`) — Python/Cython/C → Rust port, Port Mortem 2026, Track D.

This is the translation rulebook for the port, written **before implementation** per the Anthropic large-scale-migration playbook: rules first, gap inventory second, code third. Every rule below is mechanically checkable and carries a verification hook (§9). Where a translation cannot be mechanical, it is listed in the gap inventory (§8) with an explicit, reviewed resolution.

**Authority order (binding):**

1. `architecture.md` §5 — the normative behavioral spec (including the two sanctioned bug fixes).
2. This rulebook — restates the spec as translation rules.
3. The original sources under `src/` — ground truth for everything EXCEPT the two sanctioned bug fixes (#14 Soundex padding, #15 Soundex non-ASCII), where architecture §2/§5.1 wins.

---

## 1. Source & target inventory

| Source (ground truth, read-only — NEVER modify) | Lines | Translates to |
|---|---|---|
| `src/fuzzy.pyx` | 263 | `rust/fuzzy-core/src/{soundex,nysiis}.rs` + DMetaphone wrapper semantics; PyO3 surface in `rust/fuzzy-py` |
| `src/double_metaphone.c` | 1184 | `rust/fuzzy-core/src/dmetaphone.rs` (arm-by-arm) |
| `src/double_metaphone.h` | 48 | metastring model → §6.1 |
| `test/test_fuzzy.py` | 37 | preserved byte-identical in `tests/original/`; re-expressed as native Rust tests |

Target crates: `fuzzy-core` (pure safe Rust, `#![forbid(unsafe_code)]`, std-only, zero external crates), `fuzzy-cli` (thin batch-stdin driver), `fuzzy-py` (PyO3 0.23 bindings, module name `fuzzy`).

Rust-native core API (pinned by architecture §4):

```rust
pub fn soundex(size: usize, s: &str) -> String
pub fn nysiis(s: &str) -> String
pub fn dmetaphone(s: &str) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), NonAsciiError>
pub fn dmetaphone_with_size(size: usize, s: &str) -> Result<(Option<Vec<u8>>, Option<Vec<u8>>), NonAsciiError>
/// Raw bytes-level entry point (no ASCII validation, no None-collapse, no size
/// truncation) — faithful exposure of the C algorithm incl. the Latin-1 arms.
pub fn dmetaphone_bytes(input: &[u8]) -> (Vec<u8>, Vec<u8>)
```

---

## 2. Type mappings

| Python / Cython / C | Rust | Notes |
|---|---|---|
| Python `str` (unicode) | `&str` | UTF-8-valid by construction; see encoding rules §3 |
| Python `bytes` | `Vec<u8>` / `&[u8]` | DMetaphone codes are bytes, not str |
| Python `int` (`size` param) | `usize` | PyO3 layer rejects negative ints with `ValueError` (§7) |
| Python `None` | `Option::None` | e.g. collapsed/empty DMetaphone secondary code |
| Python list `[bytes|None, bytes|None]` | `(Option<Vec<u8>>, Option<Vec<u8>>)` in core | PyO3 layer converts the tuple to a Python `list` |
| C `char *` (NUL-terminated) | `Vec<u8>` with explicit length | NUL is not significant in Rust; length is data |
| C `metastring` struct | `Vec<u8>` + helper rules (§6.1) | growable buffer, append, bounds-checked indexing |
| C `int` index / offset | `isize` for index arithmetic, `usize` for lengths | negative lookbehind must not underflow (gap G5) |
| C `char` (a byte) | `u8` | NOT `char` — the C is byte-oriented (ASCII/Latin-1) |
| Cython `cdef class Soundex` / `DMetaphone` | `#[pyclass]` in `fuzzy-py` over plain fns in `fuzzy-core` | callable classes via `__call__` |
| Python module-level `def nysiis(s)` | `pub fn nysiis(&str) -> String` + `#[pyfunction]` | |
| Python dict lookup tables (4 NYSIIS maps) | `match` on `&str` / byte-slice keys (std-only) | no external crates in fuzzy-core |
| C varargs `StringAt(s, start, len, ...)` | slice parameter `&[&[u8]]` | NOT mechanical — gap G3 |
| C `toupper()` per byte | `u8::to_ascii_uppercase()` | NOT `str::to_uppercase` — gap G4 |
| Python `str.upper()` | `str::to_uppercase()` | full Unicode semantics (`ß`→`SS`) |

---

## 3. String & encoding rules

### 3.1 The Cython directive (the root encoding rule)

`src/fuzzy.pyx` line 1:

```cython
# cython: c_string_type=unicode, c_string_encoding=ascii
```

Meaning: every coercion of a Python `str` to a C `char *` (the `cs = s` assignments) **ASCII-encodes** the string; any non-ASCII character raises `UnicodeEncodeError` on Python 3. Coercions from `char *` back to a Python `str` ASCII-decode; coercions into variables declared `bytes` copy raw bytes with no decoding.

### 3.2 Consequences per entry point

- **`Soundex.__call__`**: `cs = s` ASCII-encodes the input → non-ASCII input raises `UnicodeEncodeError` (upstream bug #15). The port **fixes** this (rule S1 below): the Rust core takes `&str`, unicode-uppercases, then filters to `A–Z`. No Soundex error path remains.
- **`DMetaphone.__call__`**: same coercion → non-ASCII input raises `UnicodeEncodeError`. The port **preserves** this: `fuzzy-core` ASCII-validates and returns `Err(NonAsciiError)`; `fuzzy-py` maps it to `UnicodeEncodeError` (§7).
- **`nysiis(s)`**: never coerces the input to `char *` — it is pure Python-level string logic. Unicode input is accepted, uppercased, then stripped to `A–Z`. The port preserves this exactly (§5).
- **Soundex return path**: `pout = out` ASCII-decodes the C buffer to `str`. Rust: `String` — always ASCII letters/digits by construction.
- **DMetaphone return path**: `cdef bytes o1 = out[0]` copies raw bytes. Rust: `Vec<u8>`; PyO3 returns `bytes`.

### 3.3 Rust encoding model

- `&str` input is valid UTF-8 by construction — the original's implicit codec-failure class can only reappear as an **explicit** ASCII validation (`dmetaphone`), never as a hidden codec error.
- Soundex and NYSIIS never fail on encoding: normalization drops everything outside `A–Z`.
- `dmetaphone_bytes(&[u8])` is the raw byte-level entry point with NO validation — the faithful exposure of the C algorithm, including the Latin-1 arms (gap G1).

---

## 4. Soundex — pinned FIXED semantics (architecture §5.1)

Letter→digit map (classic, indexed by `c - b'A'`):

```text
"01230120022455012623010202"    (A=0 … Z=25)
```

Rules S0–S5 (binding):

- **S0.** If `size == 0`, return `""` immediately (short-circuit; matches the original's `Soundex(0)` → `''`).
- **S1. Normalize (#15 fix):** unicode-uppercase (`str::to_uppercase()`, the Python `str.upper()` equivalent), then keep only ASCII `A–Z` — drop everything else: accented letters, digits, spaces, punctuation.
- **S2.** If the filtered string is empty, the code is empty; go to padding (S5).
- **S3.** Emit the first character verbatim.
- **S4.** For each subsequent character `c`, let `d = map[c - 'A']`. If `d == '0'`, skip. Otherwise append `d` iff `written == 1` OR `last_written != d` — the `written == 1` clause bypasses dedup for the first digit; replicate exactly. Stop as soon as `written == size`.
- **S5. Padding (#14 fix):** if `size <= 4`, right-pad with `'0'` to exactly `size`. If `size > 4`, do NOT pad — `size` is a maximum length only.

Binding data points (all seven are contract-binding; each becomes a native Rust test with `binding` in the name):

| Call | Result | Source |
|---|---|---|
| `Soundex(4)('fuzzy')` | `'F200'` | README doctest |
| `Soundex(4)('FancyFree')` | `'F521'` | `test_soundex_result` (xfail, issue #14) |
| `Soundex(8)('Test')` | `'T23'` | `test_soundex_Test` (xfail, issue #14) — size 8 is a maximum, NOT a pad target; the original produced `'T2300000'` |
| `Soundex(8)('Jéroboam')` | `'J615'` | `test_soundex_non_ascii` (xfail, issue #15) — `é` uppercases to `É`, then the `A–Z` filter drops it; no error |
| `Soundex(4)('')` | `'0000'` | original behavior preserved |
| `Soundex(4)('123')` | `'0000'` | original behavior preserved (digits filtered out, then padded) |
| `Soundex(0)('anything')` | `''` | original behavior preserved |

Do-not-"improve" notes:

- The dedup rule is the original's **simplified** one: compare only against the last *written* character. Vowels and H/W do NOT reset dedup (no classic H/W adjacency special-casing). `BABAB` → `B100` (classic Soundex would give `B110`); `Tymczak` → `T520`.
- Lowercase input works (S1 uppercases).
- `size` is `usize`; the original took a C int with no validation. The PyO3 layer rejects negative ints with `ValueError` — a documented, defensive divergence (the original had UB there).

---

## 5. NYSIIS — exact port (architecture §5.2)

Port `nysiis()` from `src/fuzzy.pyx` lines 81–185 **exactly**. Pipeline (order is binding):

1. `s.upper()` (Unicode-aware), then strip all non-`A–Z` (regex `[^A-Z]` equivalent).
2. `first` = first char of the filtered string (or empty).
3. Strip trailing `S`/`Z` (BEFORE any prefix handling).
4. Initial `MAC` → `MC` with `stop = stop - 1`; initial `PF` → `start = 1` (the `P` is dropped from the scanned slice).
5. Suffix loop: while `(stop - start) > 2`, map the trailing 2 chars via `_nysiis_suffix_map`, accumulating `suffix = mapped + suffix`, `stop -= 2`; break at the first unmapped pair.
6. `s = s[start:stop] + suffix`; the main scan restarts at index 0 of this new string.
7. Main scan left→right: try 3-, 2-, 1-char keys in `_nysiis_transforms` (longest first, first match wins); else, if `i > start`, try `_nysiis_trans_not_first`; else, if `i < stop - 1`, try `_nysiis_trans_middle` (`Y→A`). Replacements may be multi-char (`EV→AF`). Advance by the matched key length; unmatched chars are copied verbatim.
8. Trim trailing vowels from the result list (this records a trim length).
9. First-vowel restore: if `first` is a vowel, force output position 0 back to `first`. **Quirk:** in Python `'' in 'AEIOU'` is `True`, so empty input takes this branch too (`nysiis('') == ''`). Implement as: if `first` is empty OR a vowel → set/replace the first output char with `first` (gap G2).
10. Collapse CONSECUTIVE duplicate chars only; join.

Lookup tables — copy verbatim from `src/fuzzy.pyx` (`_nysiis_suffix_map` 10 entries, `_nysiis_transforms` 18, `_nysiis_trans_not_first` 18, `_nysiis_trans_middle` 1):

```python
_nysiis_suffix_map = {'IX':'IC','EX':'EC','YE':'Y','EE':'Y','IE':'Y',
                      'DT':'D','RT':'D','RD':'D','NT':'D','ND':'D'}
_nysiis_transforms = {'AY':'Y','DG':'G','E':'A','EY':'Y','GHT':'GT','K':'C',
                      'KN':'N','I':'A','IY':'Y','O':'A','OY':'Y','PH':'F',
                      'SH':'S','SCH':'S','U':'A','UY':'Y','WR':'R','YW':'Y'}
_nysiis_trans_not_first = {'AH':'A','AW':'A','EH':'A','EV':'AF','EW':'A',
                           'HA':'A','HE':'A','HI':'A','HO':'A','HU':'A',
                           'IH':'A','IW':'A','M':'N','OH':'A','OW':'A',
                           'Q':'G','UH':'A','UW':'A','Z':'S'}
_nysiis_trans_middle = {'Y':'A'}
```

Quirk list (each quirk MUST survive the port):

- **Q1.** Unicode-uppercase THEN ASCII strip: `ß`.upper() → `SS` survives the filter (`nysiis('Straße') == 'STRAS'`, identical to `'Strasse'`); `é` → `É` is dropped.
- **Q2.** Empty-string vowel quirk: `'' in 'AEIOU' == True` → `nysiis('') == ''` via the first-vowel branch.
- **Q3.** The trailing `S`/`Z` strip runs BEFORE the `MAC`/`PF` prefix handling and the suffix mapping.
- **Q4.** `MAC`→`MC` adjusts `stop` (`stop - 1`); `PF` sets `start = 1`, but the main scan restarts at index 0 of the sliced string.
- **Q5.** The suffix loop chains (`BIXIX` → suffix applied twice → `BACAC`) and stops at the first unmapped pair; the guard is `(stop - start) > 2`.
- **Q6.** Scan precedence: 3-char keys before 2-char before 1-char; the not-first map applies only when `i > start`; middle-`Y→A` only when `i > start` AND `i < stop - 1`.
- **Q7.** Replacements may change length (`EV→AF`).
- **Q8.** Trailing-vowel trim happens BEFORE the first-vowel restore, and the trim length still governs the dedup slice (`AEIOU` → `''`).
- **Q9.** Dedup collapses adjacent duplicates only (`BAB` stays `BAB`).

Binding data points: `nysiis('fuzzy') == 'FASY'`; `nysiis('') == ''`; `nysiis('123') == ''`.

---

## 6. Double Metaphone — C → Rust structural mapping (architecture §5.3)

Port `src/double_metaphone.c` faithfully, arm by arm. **Ground truth:** the original C compiled with cl.exe (`tools/oracle-c`). Any doubt about a rule → diff against the oracle, not against intuition.

### 6.1 The metastring model → `Vec<u8>`

| C | Rust |
|---|---|
| `metastring { str, length, bufsize, free_string_on_destroy }` | `Vec<u8>` (capacity ≈ `bufsize`; length is `len()`) |
| `NewMetaString(s)` (bufsize = len + 7) | `Vec::with_capacity(len + 7)` + extend |
| `MetaphAdd(s, x)` (strcat + grow) | `s.extend_from_slice(x)` |
| `GetAt(s, pos)`: `pos < 0` or `pos >= length` → `'\0'` | helper `get_at(s: &[u8], pos: isize) -> u8` returning `0` out of range — this bounds rule is load-bearing: ALL lookahead/lookbehind relies on it |
| `SetAt(s, pos, c)`: out of range → silent no-op | guarded write (only used for the 4-char cap; see D6) |
| `IsVowel(s, pos)` = A/E/I/O/U/**Y** | same set (Y included), via `get_at` |
| `SlavoGermanic(s)` = crude substring check for `W`, `K`, `CZ`, `WITZ` | same crude substring checks — replicate, don't fix |
| `MakeUpper(s)` = per-byte `toupper` | per-byte `u8::to_ascii_uppercase` (gap G4/G8) |
| `StringAt(s, start, len, ...)` varargs, `""`-terminated, compares exactly `len` bytes; `start < 0` or `start >= length` → 0 | helper `string_at(s, start: isize, len: usize, pats: &[&[u8]]) -> bool` (gap G3/G9) |

### 6.2 Function-level rules

- **D1.** Compute `length`/`last` BEFORE padding; then pad the input with **5 spaces**. All lookahead relies on the padding (e.g. the CH arm's lookahead list matches a padding space; `ach` → `AK|-`).
- **D2.** `MakeUpper` runs AFTER padding (spaces are unaffected).
- **D3.** Initial skip: `GN`/`KN`/`PN`/`WR`/`PS` at position 0 → `current += 1`. Initial `X` → emit `S`/`S`, `current += 1`.
- **D4.** Main loop: `while (primary.len < 4 || secondary.len < 4)` with `if current >= length break`. The **OR** condition is load-bearing — the loop stays alive for the secondary after the primary reaches 4 (`tagliarb` → `TKLR|TLRP`; an AND-rewrite yields the wrong `TKLR|TLR`).
- **D5.** Big switch over `get_at(original, current)` — port arm by arm, in source order; anything unmatched hits `default: current += 1` (digits/punctuation/spaces are skipped silently, no error).
- **D6.** After the loop: truncate both codes to max **4 chars**. (The C writes `'\0'` at index 4 via `SetAt` without updating `length`; Rust uses `Vec::truncate(4)` — observably equivalent because the code escapes only as a C string. See gap G7.)
- **D7.** Byte-oriented throughout: the two Latin-1 case labels `0xC7` ('Ç' → `S`) and `0xD1` ('Ñ' → `N`) become `u8` match arms — keep them for fidelity even though the Python-facing path never produces them (gap G1).

### 6.3 Wrapper semantics (from the .pyx; applied in `dmetaphone`/`dmetaphone_with_size`, NOT in `dmetaphone_bytes`)

- **W1.** ASCII-validate the input first; non-ASCII → `Err(NonAsciiError)` (§7).
- **W2.** Collapse: if primary == secondary, secondary → `None`. An empty code → `None`.
- **W3.** THEN truncate each code to `size` (`size == 0` = unlimited; the C already caps at 4, so only size 1–3 matter). Order is binding: **collapse BEFORE truncation** (`dmetaphone 1 bier` → `P|P`; a truncate-then-collapse port wrongly yields `P|-`).
- **W4.** Return `(Option<Vec<u8>>, Option<Vec<u8>>)` — bytes, not str (the Python API returns `bytes`).

Binding data points: `DMetaphone()('mayer') == [b'MR', None]`; `DMetaphone()('fuzzy') == [b'FS', None]`; `DMetaphone()('') == [None, None]`.

---

## 7. Error behavior

| Condition | Original | Port (Rust core) | Port (Python surface via PyO3) |
|---|---|---|---|
| `Soundex` non-ASCII input | `UnicodeEncodeError` (bug #15) | no error — filtered per S1 | no error (fixed) |
| `DMetaphone` non-ASCII input | `UnicodeEncodeError` | `Err(NonAsciiError)` carrying the offending character and its byte position | `UnicodeEncodeError` (original behavior preserved) |
| negative `size` (Soundex or DMetaphone) | C `int`, no validation (UB / bizarre slicing) | unrepresentable (`usize`) | `ValueError` (documented, defensive) |
| non-`str` argument | `TypeError` | n/a (typed API) | `TypeError` (natural PyO3 extraction) |
| `Soundex(0)` | `''` | `""` | `''` |

`NonAsciiError` is a small error type carrying the offending character and its byte position; the PyO3 layer maps it to `pyo3::exceptions::PyUnicodeEncodeError`.

---

## 8. Gap inventory — what cannot be translated mechanically

Per the migration playbook, every non-mechanical translation point is listed here with its reviewed resolution. These are the places where a naive transliteration compiles but produces wrong behavior.

- **G1. Latin-1 case labels.** `double_metaphone.c` contains `case 'Ç':` (byte `0xC7`) and `case 'Ñ':` (byte `0xD1`); the file is Latin-1, not valid UTF-8. Rust match arms must use byte literals `0xC7`/`0xD1`, and the arms are unreachable from `&str` input (UTF-8 encodes Ç/Ñ as two bytes, and the ASCII validator rejects them anyway). **Resolution:** expose `dmetaphone_bytes(&[u8])` as the faithful byte-level entry point; cover the arms with native `latin1` tests.
- **G2. Python `'' in 'AEIOU'` quirk.** Python substring containment is `True` for the empty string, so `nysiis('')` silently takes the first-vowel branch. A mechanical `contains` port compiles but hides the intent. **Resolution:** explicit branch — `first.is_empty() || "AEIOU".contains(first)` — pinned by the `nysiis('') == ''` binding test.
- **G3. C varargs `StringAt`.** `StringAt(s, start, len, "GN", "KN", ..., "")` — variadic, empty-string-terminated — has no Rust equivalent. **Resolution:** `string_at(s, start: isize, len: usize, pats: &[&[u8]])`; rewrite the ~100 call sites from variadic lists to array literals, dropping the `""` terminator. The compiler checks types; the oracle and the fuzz campaign referee the semantics.
- **G4. Two different "uppercase" operations.** Python `str.upper()` is full-Unicode (`ß`→`SS`); C `toupper()` is byte-wise ASCII; Soundex-original does a manual `c - 32`. **Resolution:** `str::to_uppercase()` for the Soundex/NYSIIS Python-path normalization; `u8::to_ascii_uppercase()` for the C `MakeUpper`. Never substitute one for the other.
- **G5. Negative index arithmetic.** The C freely computes `current - 2`, `current - 4`, etc., relying on `GetAt`/`StringAt` returning 0 for negative positions. `usize` arithmetic would underflow and panic. **Resolution:** signed (`isize`) index math at every lookahead/lookbehind, with the bounds rule centralized in `get_at`/`string_at`.
- **G6. Python truthiness idioms.** `size or 99999` and `o1 and o1[:size] or None` rely on 0/empty falsiness. **Resolution:** explicit forms — `size == 0` → unlimited; `code.is_empty()` → `None`; collapse-before-truncate per W3.
- **G7. NUL-write truncation.** The C caps codes at 4 by writing `'\0'` at index 4 WITHOUT updating `length` — correct only because the value escapes as a C string. **Resolution:** `Vec::truncate(4)`; stated here so nobody "faithfully" replicates the stale-length bug.
- **G8. C `toupper` on high bytes.** `toupper(*i)` on bytes ≥ 0x80 is technically UB (negative int); MSVC passes them through unchanged. **Resolution:** defined behavior — ASCII-only uppercase, high bytes unchanged (matches the observed oracle behavior).
- **G9. `strncmp` length-exact matching over padding.** `StringAt` compares exactly `length` bytes and may legitimately match padding spaces (e.g. the CH arm's lookahead list contains `" "`). **Resolution:** compare exact-length byte slices against the 5-space-padded buffer; never trim the padding early.
- **G10. The ctypes mutation probe.** `test_soundex_does_not_mutate_strings` uses ctypes to prove the input is not mutated. In Rust, `&str` makes mutation unrepresentable — the property holds by construction; the original test still runs unmodified against the PyO3 module.

---

## 9. Verification hooks (compiler & tests as referee)

- Every §4/§5/§6 binding data point is a native `#[test]` with `binding` in the name (contract-filterable).
- DM vectors (`tools/vectors/dmetaphone_vectors.json`, ≥ 50) are each validated against the C oracle; a data-driven native test (`vectors` in the name) replays all of them.
- Non-ASCII rejection tests contain `non_ascii`; Latin-1 arm tests contain `latin1`.
- Differential fuzzing: Rust vs the C oracle (dmetaphone, exact parity) and Rust vs a Python oracle (soundex/nysiis, fixed semantics); zero mismatches required. A documented-divergence run records the intentional #14/#15 differences against original semantics.
- The original pytest suite runs unmodified against the PyO3 module: expected **2 passed, 3 xpassed** (the three xfail tests XPASS because the port fixes #14/#15).
- `cargo clippy -- -D warnings` and `cargo fmt --check` are gates; `#![forbid(unsafe_code)]` makes any unsafe block a compile error in `fuzzy-core`/`fuzzy-cli`.
