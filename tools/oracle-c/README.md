# oracle-c — Double Metaphone ground-truth oracle

Compiles the ORIGINAL `src/double_metaphone.c` (read-only ground truth —
Latin-1 bytes, never modify or re-save) into `dmoracle.exe`, a stdin batch
filter that serves as the behavioral reference for the Rust Double Metaphone
port and the differential fuzzer (`tools/fuzz_diff.py --algo dmetaphone`).

## Build

```powershell
PowerShell -ExecutionPolicy Bypass -File tools\oracle-c\build_oracle.ps1
```

The script is idempotent and self-contained. vcvars64.bat does not put
cl.exe on PATH on this machine, so the script sets PATH/INCLUDE/LIB manually
for MSVC 14.44.35207 + Windows SDK 10.0.26100.0 (see the mission library
`environment.md`, quirk #2), then runs:

```
cl.exe /nologo /TC /I ..\..\src main.c ..\..\src\double_metaphone.c /Fe:dmoracle.exe
```

Intermediate `.obj` files are removed after the build. `dmoracle.exe` is
gitignored — rebuild locally, never commit the binary.

## Usage

One word per line on stdin (ASCII; an empty line means the empty string).
Exactly one output line per input line: `<primary>|<secondary>`.

```powershell
@('mayer','fuzzy','') | .\tools\oracle-c\dmoracle.exe
# MR|MR
# FS|FS
# |
```

Sanity anchors: `mayer` → `MR|MR`, `fuzzy` → `FS|FS`, empty line → `|`.

Semantics notes:

- Output is the RAW C library result: no wrapper semantics. Equal codes are
  printed twice (`MR|MR`, not collapsed), empty codes print as empty strings,
  and the only truncation is the C's own 4-character cap. The Python/Rust
  wrapper rules (equal → `None`, empty → `None`, `size` truncation) are
  applied by consumers on top of these raw codes.
- Lines may be arbitrarily long; CRLF and LF endings are both accepted.
- A stray UTF-8 BOM on the first piped line is stripped defensively
  (PowerShell 5.1 can prepend one — `environment.md` quirk #10), mirroring
  fuzzy-cli.
- Each line's code buffers are freed after printing (the C hands ownership
  of both codes to the caller).

## Files

- `main.c` — the batch harness (stdin loop → `DoubleMetaphone` → stdout).
- `build_oracle.ps1` — the idempotent MSVC build script.
- `dmoracle.exe` — build product (gitignored).
