<#
.SYNOPSIS
    Uninstalls the DsHidMini driver package bundled with PS3Pad.

.DESCRIPTION
    Finds the oem*.inf driver packages published by the DsHidMini
    installation (provider "Nefarius Software Solutions e.U.", original
    name dshidmini.inf or igfilter.inf) and removes them from the driver
    store using pnputil /delete-driver /force.

    The script parses pnputil output in a language-agnostic way: it looks
    for driver blocks that mention both "Nefarius" and "dshidmini.inf" or
    "igfilter.inf", then extracts the oemXX.inf published name.

    Requires elevation (run as administrator). Invoked from the Inno Setup
    [UninstallRun] section with the runas flag.
#>
[CmdletBinding()]
param()

$ErrorActionPreference = "Stop"

Write-Host "=== Uninstalling DsHidMini driver package ==="

$output = & pnputil /enum-drivers 2>&1

# Split into driver blocks (separated by blank lines).
$blocks = ($output -join "`n") -split "`n`n+"

$deleted = 0
foreach ($block in $blocks) {
    # Look for blocks that belong to Nefarius and reference our INF files.
    if ($block -match "Nefarius" -and ($block -match "dshidmini\.inf" -or $block -match "igfilter\.inf")) {
        if ($block -match "(oem\d+\.inf)") {
            $oem = $matches[1]
            Write-Host "Removing driver package $oem..."
            $out = & pnputil /delete-driver $oem /force 2>&1
            Write-Host $out
            if ($LASTEXITCODE -eq 0) {
                $deleted++
                Write-Host "  removed."
            } else {
                Write-Host "  failed to remove (may still be in use)."
            }
        }
    }
}

Write-Host "Removed $deleted driver package(s)."
Write-Host "=== DsHidMini driver uninstallation complete ==="
