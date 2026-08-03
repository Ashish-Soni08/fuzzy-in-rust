# AGENTS.md — How AI agents executed this port

This repository was ported, proven, and documented by a **team of AI agents** —
one orchestrator plus specialized workers (a Factory Droid mission) — in a single
time-boxed day, 2026-08-03, under the Port Mortem 2026 hackathon rules. The human
set the goal, the bug policy, and the deadline; the agents did everything else:
translation rulebook, Rust implementation, equivalence machinery, fuzz campaigns,
and the documentation you are reading.

Two Anthropic engineering pieces were the operating manual, and this file maps
each of their lessons to what concretely happened here:

1. **Anthropic's playbook for large-scale code migrations** — rulebook-first,
   gap inventory, stress-testing the rules, a mechanical resumable work queue,
   adversarial review, the compiler/tests as referee, fix-the-loop-not-the-code,
   and front-loaded human judgment.
2. **Anthropic's "Effective harnesses for long-running agents"** — feature lists
   as persistent state, incremental progress, clean-state commits with progress
   notes, end-to-end verification with real tools, and never declaring victory
   early.

Nothing below is aspirational; every claim points at a file, commit, or command
in this repository.

---

## Part 1 — The migration playbook, applied to `fuzzy`

### 1. Rulebook first → `RULEBOOK.md` before any Rust

The playbook's first rule: write the translation rulebook before writing code.
The first mission commits were not code — they were `tests/original/` preservation
(`dbcd5fe`) and [`RULEBOOK.md`](RULEBOOK.md) (`c65ff5a`), a Cython/C → Rust
translation rulebook derived from the normative behavioral spec (type mappings,
encoding rules, the pinned Soundex semantics, the NYSIIS quirk list, the
DMetaphone structural mapping, error behavior). Every worker ported *against the
rulebook*, with a stated authority order: spec > rulebook > original source
(except the two sanctioned bug fixes, where the spec wins).

### 2. Gap inventory → `RULEBOOK.md` §8, reviewed resolutions

The playbook warns that not everything translates mechanically, and demands an
explicit **gap inventory** instead of silent improvisation. Ours names each
non-mechanical translation and its reviewed resolution: C varargs
`StringAt(start, len, ...)` became a `&[&[u8]]` slice parameter; C per-byte
`toupper()` became `u8::to_ascii_uppercase()` (deliberately *not*
`str::to_uppercase()`); negative C index lookbehind became `isize` arithmetic
that cannot underflow; the C `metastring` became a bounds-checked `Vec<u8>`
model replicating out-of-range-returns-`0`. Each gap has a rule number and a
verification hook, so no worker ever "winged it" at the keyboard.

### 3. Stress-test the rules → oracles and a negative control

The playbook says to stress-test the translation rules before trusting them at
scale. We did that with ground truth, not intuition:

- **The original C code is the referee's rulebook made executable**:
  `tools/oracle-c/` compiles the untouched `src/double_metaphone.c` with MSVC
  into `dmoracle.exe`. Every curated DMetaphone vector
  (`tools/vectors/dmetaphone_vectors.json`) was validated against that oracle
  before entering the tree.
- **The fuzz harness must prove it can fail**: `tools/fuzz_diff.py --selftest`
  injects a deliberately wrong expectation and is *required* to exit non-zero.
  A differential harness that has never detected a mismatch proves nothing.
- **The Python oracle is a statement-for-statement transcription** of
  `src/fuzzy.pyx` (`tools/oracle_py.py`), so Soundex/NYSIIS rules were checked
  against the original's logic, not a re-remembered spec.

### 4. A mechanical, resumable work queue → the mission feature list

The playbook's queue — small, mechanical, independently verifiable units that
any worker can pick up and resume — was literal here. The mission's
`features.json` decomposed the port into 16 implementation features across four
milestones (`foundation`, `dmetaphone`, `parity`, `submission`), each with a
typed worker (`rust-core-worker`, `tooling-worker`, `integration-worker`,
`docs-worker`), explicit preconditions, and the contract assertions it fulfills.
Any feature could be re-run or resumed from committed state; the seeded fuzz
generator (seed `20260803`) makes even the 50,000-case campaigns deterministic
and resumable — a validator re-running with the same seed reproduces the
identical corpus prefix.

### 5. Adversarial review → a 126-assertion validation contract + milestone gates

Review was not a vibe; it was **adversarial** and machine-checkable. The
mission's validation contract holds **126 assertions** (VAL-SDX 24, VAL-NYS 27,
VAL-DM 18, VAL-FUZZ 9, VAL-PAR 17, VAL-REPO 13, VAL-DOCS 12, VAL-CROSS 6), each
with a tool and a pass/fail criterion. Every milestone ended with two
independent validator roles — a scrutiny validator (re-runs builds, tests,
linters, hash verification) and a user-testing validator (exercises the
contract surfaces end-to-end) — whose explicit job is to find reasons the work
is *not* done. The original test file's SHA-256 is re-verified at every gate.

### 6. Compiler and tests as referee → gates that cannot be argued with

Every worker handoff required the same **referee** calls to pass: `cargo test`
(scoped), `cargo clippy -- -D warnings`, `cargo fmt --check`, plus the feature's
behavioral check (oracle diff or pytest run). `#![forbid(unsafe_code)]` in
`fuzzy-core`/`fuzzy-cli` makes memory safety a compiler-enforced fact, not a
review comment. Final standings from the referees: **165 native Rust tests
passed, 0 failed**; the unmodified original suite: **2 passed, 3 xpassed**;
clippy and rustfmt clean across the workspace.

### 7. Fix the loop, not the code → process fixes over heroics

When the environment bit us, we fixed the *loop* so no worker would hit it
twice, instead of patching instances:

- `core.autocrlf=true` would have silently normalized the hash-pinned original
  test file's CRLF line endings on commit, breaking byte-identity. The fix was
  not "be careful" — it was a `.gitattributes` `-text` rule plus
  `tools/verify_original_hashes.py` at every gate.
- pytest emits ANSI escapes even when piped on this machine; the fix was a
  documented `$env:NO_COLOR='1'` capture convention, not per-worker
  re-discovery.
- A broken cmd AutoRun, cargo not on PATH, cl.exe needing manual
  PATH/INCLUDE/LIB: each became a written environment quirk in the shared
  mission library and a verbatim command in `services.yaml` — the single source
  of truth for *how to run things*, so workers never re-derive commands.

### 8. Front-loaded human judgment → the hard calls made before any code

The playbook's most important lesson: **front-load** the human hours. Before the
first worker started, the human and orchestrator pinned every judgment call the
agents should not make on the fly: the bug policy (fix exactly upstream #14 and
#15 — the behaviors the original's own tests define as intended — and replicate
everything else, quirks included); the normative per-algorithm spec with binding
data points (`Soundex(8)('Test') == 'T23'`, `nysiis('') == ''`,
`DMetaphone()('mayer') == [b'MR', None]`); the pinned CLI and fuzz-report
interfaces; the safe-Rust rule; the deadline. Agents never decided *what* to
build mid-flight — they executed a spec.

---

## Part 2 — Harness lessons for long-running agents, applied to this mission

### 1. Feature lists as persistent state → `features.json` + the contract

The harness lesson: an agent's memory is its artifacts, so keep the plan in a
durable, structured feature list, not in a chat transcript. The mission ran on
exactly that: `features.json` (id, status, milestone, skill, preconditions,
fulfills) was the shared state every worker read first and the orchestrator
updated after each completion; the validation contract was the machine-checkable
definition of done. Context windows ended; the state did not.

### 2. Incremental progress → one feature, one green increment

No worker was asked to "port the library." Each did one increment — Soundex
core, then NYSIIS core, then the DMetaphone state machine, then the C oracle,
then the fuzz driver, then the PyO3 bindings — each landing green (tests +
clippy + fmt) before the next started. The port you see is the sum of small,
verified steps, not one heroic generation.

### 3. Clean-state commits + progress notes → real history on `origin/master`

Every completed feature ended with a scoped, imperative commit pushed to
`origin/master` — the hackathon requires real incremental history, and the
harness lesson explains why it works: each commit is a clean, working state a
future worker can resume from, and the message is the progress note. The log
tells the story in order:

```text
dbcd5fe preserve: original test suite byte-identical under tests/original + hash verifier
c65ff5a docs: RULEBOOK.md - translation rulebook + gap inventory
d74e940 scaffold: rust workspace (fuzzy-core stubs + fuzzy-cli batch protocol)
026860e port: soundex core + native tests
5b28455 port: nysiis core + native tests
0b5a27c tools: C oracle harness for Double Metaphone ground truth
c88c3d7 port: dmetaphone core + native tests
4d83ed1 tools: oracle-validated dmetaphone vectors + data-driven native test
f796a20 tools: differential fuzz driver + 50k dmetaphone campaign (0 mismatches)
86c20a4 parity: pyo3 bindings (module fuzzy) + original suite output (2 passed, 3 xpassed)
7da9260 tools: python oracle (soundex/nysiis) + 50k fuzz campaigns + divergence report
3a6f850 parity: honest pass-rate report (tools/reports/pass_rates.json + generator)
b88be17 docs: submission README.md (build/test, usage, bugs fixed, equivalence proof)
04f4071 docs: DECISIONS.md - equivalence proof, fuzz stats, bug #14/#15 write-ups, trade-offs
```

### 4. End-to-end verification with real tools → nothing mocked

The harness lesson: verify the way a user would, with the real tools. The
original pytest suite runs **unmodified** against the real PyO3 module installed
by a real `maturin develop` into a real venv. The DMetaphone oracle is the
**actual original C** compiled with the actual MSVC toolchain. The fuzz
campaigns are real 50,000-case runs whose JSON reports are committed as
evidence. `tools/build_pass_rates.py` regenerates
[`tools/reports/pass_rates.json`](tools/reports/pass_rates.json) from *live*
runs and **fails loudly rather than write an unverifiable number** — the
scoreboard cannot drift from reality.

### 5. Never declare victory early → proof obligations, honest divergences

The mission's definition of done was deliberately hard to satisfy prematurely:
three independent equivalence legs (original suite, native tests, differential
fuzzing), a negative control proving the harness can fail, hash verification of
the preserved tests at every gate, and a final sync-verification feature whose
only job is to re-check everything after the last doc lands. Most importantly,
the mission did **not** declare victory on the flattering metric: the port
intentionally diverges from the original in two bug-fix classes, and instead of
hand-waving that, a dedicated fuzz run against *original* Soundex semantics
produced the honest count — **14,187 divergent cases out of 50,000, 100%
inside the two sanctioned classes, 0 unclassified** — committed as
`tools/reports/divergence_soundex_20260803_50000.json` and written up in
[DECISIONS.md](DECISIONS.md). Victory was declared only when the evidence said
so, and the evidence includes the failures-that-are-features.

---

## The receipts

| Claim | Evidence |
|---|---|
| Original tests preserved byte-identical | `tests/original/` + `SHA256SUMS.txt`; `python tools\verify_original_hashes.py` exits 0 |
| Original suite green against the port | **2 passed, 3 xpassed** (`tools/reports/original_suite_output.txt`) |
| Native Rust tests | **165 passed, 0 failed** (`cargo test --workspace`) |
| Differential fuzz vs ground truth | **150,000 cases, 0 mismatches** (`tools/reports/fuzz_*_20260803_50000.json`) |
| Intentional divergences, honestly counted | **14,187**, all in bug-fix classes #14/#15, 0 unclassified |
| Safe Rust | `#![forbid(unsafe_code)]` in fuzzy-core/fuzzy-cli; no handwritten unsafe in fuzzy-py; clippy `-D warnings` + rustfmt clean |
| Process | 16 features, 4 milestones, 126 contract assertions, commit+push per feature |

## Pointers

- [RULEBOOK.md](RULEBOOK.md) — the translation rules and gap inventory (playbook lessons 1–2).
- [DECISIONS.md](DECISIONS.md) — the equivalence proof, fuzz statistics, bug root-cause write-ups, trade-offs.
- [DEMO.md](DEMO.md) — the 2–3 minute live demo script.
- [README.md](README.md) — the submission readme (build, test, usage, bugs fixed).
