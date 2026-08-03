#Requires -Version 5.1
<#
.SYNOPSIS
  Builds tools\oracle-c\dmoracle.exe from the ORIGINAL src\double_metaphone.c.

.DESCRIPTION
  Idempotent: safe to re-run from a clean or dirty state; always rebuilds
  dmoracle.exe in place and removes intermediate .obj files.

  Machine quirk (mission library\environment.md #2): vcvars64.bat does not
  put cl.exe on PATH on this machine, so this script sets PATH/INCLUDE/LIB
  manually for MSVC 14.44.35207 + Windows SDK 10.0.26100.0.

  The original sources under src\ are READ-ONLY ground truth (Latin-1
  bytes); this script only ever reads them.
#>
$ErrorActionPreference = 'Stop'

$MSVC   = 'C:\Program Files (x86)\Microsoft Visual Studio\2022\BuildTools\VC\Tools\MSVC\14.44.35207'
$SDK    = 'C:\Program Files (x86)\Windows Kits\10'
$SDKVer = '10.0.26100.0'

foreach ($p in @("$MSVC\bin\Hostx64\x64\cl.exe",
                 "$MSVC\include",
                 "$MSVC\lib\x64",
                 "$SDK\Include\$SDKVer\ucrt",
                 "$SDK\Lib\$SDKVer\ucrt\x64",
                 "$SDK\Lib\$SDKVer\um\x64")) {
    if (-not (Test-Path $p)) {
        Write-Error "MSVC/SDK path missing: $p (see mission library\environment.md quirk #2)"
    }
}

$env:PATH    = "$MSVC\bin\Hostx64\x64;$env:PATH"
$env:INCLUDE = "$MSVC\include;$SDK\Include\$SDKVer\ucrt;$SDK\Include\$SDKVer\um;$SDK\Include\$SDKVer\shared;$SDK\Include\$SDKVer\winrt"
$env:LIB     = "$MSVC\lib\x64;$SDK\Lib\$SDKVer\ucrt\x64;$SDK\Lib\$SDKVer\um\x64"

Push-Location $PSScriptRoot
try {
    & cl.exe /nologo /TC /I ..\..\src main.c ..\..\src\double_metaphone.c /Fe:dmoracle.exe
    if ($LASTEXITCODE -ne 0) {
        Write-Error "cl.exe failed with exit code $LASTEXITCODE"
    }

    Remove-Item .\*.obj -Force -ErrorAction SilentlyContinue

    if (-not (Test-Path .\dmoracle.exe)) {
        Write-Error "dmoracle.exe was not produced"
    }

    Write-Host "Built $PSScriptRoot\dmoracle.exe"
}
finally {
    Pop-Location
}
