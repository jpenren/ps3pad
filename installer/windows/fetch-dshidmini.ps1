<#
.SYNOPSIS
    Downloads the pinned DsHidMini driver MSI, verifies its SHA256, and
    extracts the driver package files needed by the PS3Pad installer.

.DESCRIPTION
    The DsHidMini MSI (22 MB) requires .NET 10 Desktop Runtime for its
    ControlApp, but the driver itself installs fine without it. This script
    downloads the MSI, verifies the pinned SHA256, and extracts only the
    driver files (INF, CAT, DLL, SYS) plus license files into
    installer\windows\drivers\dshidmini\ so Inno Setup can bundle them.

    The extracted driver files are redistributable under the DsHidMini
    BSD-3-Clause license and the nssmkig binary redistribution permission.
    See installer\windows\drivers\dshidmini\ for the sources.

.NOTES
    DsHidMini: https://github.com/nefarius/DsHidMini (BSD-3-Clause)
    nssmkig filter: permits binary redistribution without modification.
#>
[CmdletBinding()]
param(
    [string]$MsiUrl = "https://github.com/nefarius/DsHidMini/releases/download/setup-v3.27.0/Nefarius_DsHidMini_Drivers_x64_arm64_v3.27.0.msi",
    [string]$ExpectedSha256 = "8887ABD79AAD3BA45B22D2D2050EBEAEB6DA9EAB18EDE11C5464149E393F6410",
    [string]$OutputDir = "installer\windows\drivers\dshidmini"
)

$ErrorActionPreference = "Stop"
$scriptDir = Split-Path -Parent $MyInvocation.MyCommand.Definition
$repoRoot = Resolve-Path (Join-Path $scriptDir "..\..")
$outDir = Join-Path $repoRoot $OutputDir
$msiPath = Join-Path $env:TEMP "ps3pad-dshidmini.msi"

Write-Host "=== PS3Pad: fetch DsHidMini driver package ==="
Write-Host "MSI URL: $MsiUrl"
Write-Host "Expected SHA256: $ExpectedSha256"
Write-Host "Output dir: $outDir"

# Clean output directory.
if (Test-Path $outDir) { Remove-Item -Recurse -Force $outDir | Out-Null }
New-Item -ItemType Directory -Force -Path $outDir | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $outDir "x64") | Out-Null
New-Item -ItemType Directory -Force -Path (Join-Path $outDir "nssmkig_x64") | Out-Null

# Download the MSI.
Write-Host "Downloading MSI..."
Invoke-WebRequest -Uri $MsiUrl -OutFile $msiPath

# Verify SHA256.
$actual = (Get-FileHash $msiPath -Algorithm SHA256).Hash.ToUpperInvariant()
Write-Host "Actual SHA256:   $actual"
Write-Host "Expected SHA256: $ExpectedSha256"
if ($actual -ne $ExpectedSha256.ToUpperInvariant()) {
    throw "SHA256 mismatch! Expected $ExpectedSha256, got $actual."
}
Write-Host "SHA256 verified."

# Extract with administrative image install (/a) into a temp dir.
$extractDir = Join-Path $env:TEMP "ps3pad-dshidmini-extracted"
if (Test-Path $extractDir) { Remove-Item -Recurse -Force $extractDir | Out-Null }
Write-Host "Extracting MSI to $extractDir..."
$p = Start-Process -FilePath "msiexec.exe" -ArgumentList @("/a", "`"$msiPath`"", "/qn", "TARGETDIR=`"$extractDir`"") -Wait -PassThru
if ($p.ExitCode -ne 0) { throw "msiexec /a failed with exit code $($p.ExitCode)" }

# Copy driver files (x64 only).
$srcRoot = Join-Path $extractDir "PFiles64\Nefarius Software Solutions\DsHidMini\drivers"
Copy-Item (Join-Path $srcRoot "dshidmini.inf") (Join-Path $outDir "dshidmini.inf")
Copy-Item (Join-Path $srcRoot "dshidmini.cat") (Join-Path $outDir "dshidmini.cat")
Copy-Item (Join-Path $srcRoot "x64\dshidmini.dll") (Join-Path $outDir "x64\dshidmini.dll")
Copy-Item (Join-Path $srcRoot "nssmkig_x64\igfilter.inf") (Join-Path $outDir "nssmkig_x64\igfilter.inf")
Copy-Item (Join-Path $srcRoot "nssmkig_x64\igfilter.cat") (Join-Path $outDir "nssmkig_x64\igfilter.cat")
Copy-Item (Join-Path $srcRoot "nssmkig_x64\nssmkig.sys") (Join-Path $outDir "nssmkig_x64\nssmkig.sys")
Copy-Item (Join-Path $srcRoot "nssmkig_x64\LICENSE") (Join-Path $outDir "nssmkig_x64\LICENSE")
Copy-Item (Join-Path $srcRoot "nssmkig_ARM64\LICENSE") (Join-Path $outDir "nssmkig_ARM64_LICENSE")

# Cleanup.
Remove-Item -Recurse -Force $extractDir -ErrorAction SilentlyContinue
Remove-Item $msiPath -ErrorAction SilentlyContinue

Write-Host "=== DsHidMini driver package extracted to $outDir ==="
Get-ChildItem -Recurse $outDir | ForEach-Object { Write-Host "  $($_.FullName)" }

exit 0
