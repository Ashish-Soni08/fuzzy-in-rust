#!/usr/bin/env python3
"""fuzz/harness.py -- anatomy-required entry point to the pinned differential engine.

This is a THIN wrapper: it forwards its argv verbatim to ``tools/fuzz_diff.py``
via subprocess and propagates that process's exit code. All campaign logic
(seeded deterministic corpus generation, batch driving of fuzzy-cli vs the
ground-truth oracle over stdin, mismatch collection, JSON report writing)
lives in ``tools/fuzz_diff.py`` -- the pinned differential engine
(architecture.md section 7.0). This file exists so that ``fuzz/`` has a
runnable entry point; see ``fuzz/README.md`` and ``fuzz/log.txt``.

Exit code: exactly the exit code of ``tools/fuzz_diff.py`` (0 iff
mismatch_count == 0; non-zero on mismatches or harness failure).

Examples (run from the repo root):

    python fuzz/harness.py --algo dmetaphone --count 50000
    python fuzz/harness.py --algo soundex --count 50000 --seed 1337 --out %TEMP%\\fuzzval
    python fuzz/harness.py --algo dmetaphone --selftest --out %TEMP%\\fuzzval

Reports are written to ``tools/reports/`` by default (or ``--out <dir>``).
"""

from __future__ import annotations

import subprocess
import sys
from pathlib import Path

ENGINE = Path(__file__).resolve().parent.parent / "tools" / "fuzz_diff.py"


def main() -> int:
    """Forward argv to tools/fuzz_diff.py; return its exit code unchanged."""
    proc = subprocess.run([sys.executable, str(ENGINE), *sys.argv[1:]])
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main())
