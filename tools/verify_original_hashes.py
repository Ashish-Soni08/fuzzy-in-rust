#!/usr/bin/env python3
"""Verify the preserved original test suite against its pinned SHA-256 hashes.

Re-hashes every file listed in ``tests/original/SHA256SUMS.txt`` and compares
the digest against the pinned value. The preserved files are the hackathon's
test-parity proof (see ``tests/original/KICKOFF.md``); any byte change — edits,
reformatting, line-ending normalization — must be caught here.

Exit code: 0 if every listed file matches, 1 on any mismatch, missing file,
or unreadable/malformed SHA256SUMS.txt.

Stdlib only. Works from any working directory (paths resolve relative to the
repo root, derived from this script's location in ``tools/``).
"""

from __future__ import annotations

import hashlib
import sys
from pathlib import Path

REPO_ROOT = Path(__file__).resolve().parent.parent
SUMS_FILE = REPO_ROOT / "tests" / "original" / "SHA256SUMS.txt"


def parse_sums(path: Path) -> list[tuple[str, str]]:
    """Parse standard sha256sum lines: '<hash>  <name>' or '<hash> *<name>'.

    Blank lines and '#' comments are ignored. Returns (hash, filename) pairs.
    Raises ValueError on a malformed line.
    """
    entries: list[tuple[str, str]] = []
    for lineno, raw in enumerate(path.read_text(encoding="utf-8").splitlines(), 1):
        line = raw.strip()
        if not line or line.startswith("#"):
            continue
        parts = line.split()
        if len(parts) != 2:
            raise ValueError(f"{path.name}:{lineno}: malformed line: {raw!r}")
        digest, filename = parts
        entries.append((digest.lower(), filename.lstrip("*")))
    return entries


def sha256(path: Path) -> str:
    h = hashlib.sha256()
    with path.open("rb") as fh:
        for chunk in iter(lambda: fh.read(65536), b""):
            h.update(chunk)
    return h.hexdigest()


def main() -> int:
    if not SUMS_FILE.is_file():
        print(f"FAIL: {SUMS_FILE} not found")
        return 1
    try:
        entries = parse_sums(SUMS_FILE)
    except (OSError, ValueError) as exc:
        print(f"FAIL: cannot read {SUMS_FILE.name}: {exc}")
        return 1
    if not entries:
        print(f"FAIL: {SUMS_FILE.name} lists no files")
        return 1

    failures = 0
    for expected, filename in entries:
        target = SUMS_FILE.parent / filename
        if not target.is_file():
            print(f"MISMATCH {filename}: file missing")
            failures += 1
            continue
        actual = sha256(target)
        if actual == expected:
            print(f"OK {filename} {actual.upper()}")
        else:
            print(f"MISMATCH {filename}: expected {expected.upper()}, got {actual.upper()}")
            failures += 1

    if failures:
        print(f"FAIL: {failures} of {len(entries)} preserved file(s) do not match {SUMS_FILE.name}")
        return 1
    print(f"OK: all {len(entries)} preserved file(s) match {SUMS_FILE.name} — original test suite intact")
    return 0


if __name__ == "__main__":
    sys.exit(main())
