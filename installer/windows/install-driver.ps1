<#
.SYNOPSIS
    Installs the DsHidMini driver package bundled with PS3Pad.

.DESCRIPTION
    Adds the nssmkig filter INF and the dshidmini main INF to the Windows
    driver store using pnputil (built into Windows), then triggers PnP
    installation so the connected DualShock 3 is bound to DsHidMini and
    exposed as a native XInput gamepad.

    Requires elevation (run as administrator). Invoked from the Inno Setup
    [Run] section with the runas flag.

.PARAMETER DriverDir
    Directory containing the extracted DsHidMini driver package files.
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)]
    [string]$DriverDir
)

$ErrorActionPreference = "Stop"

Write-Host "=== Installing DsHidMini driver package ==="
Write-Host "Driver dir: $DriverDir"

$igFilterInf = Join-Path $DriverDir "nssmkig_x64\igfilter.inf"
$dshidminiInf = Join-Path $DriverDir "dshidmini.inf"

if (-not (Test-Path $igFilterInf)) { throw "igfilter.inf not found: $igFilterInf" }
if (-not (Test-Path $dshidminiInf)) { throw "dshidmini.inf not found: $dshidminiInf" }

# 1. Add the filter driver first.
Write-Host "Adding nssmkig filter driver..."
$out = & pnputil /add-driver $igFilterInf 2>&1
Write-Host $out
if ($LASTEXITCODE -ne 0) { throw "pnputil /add-driver igfilter.inf failed" }

# 2. Add the main DsHidMini driver.
Write-Host "Adding dshidmini driver..."
$out = & pnputil /add-driver $dshidminiInf 2>&1
Write-Host $out
if ($LASTEXITCODE -ne 0) { throw "pnputil /add-driver dshidmini.inf failed" }

Write-Host "Driver packages added to store. Windows will bind the DS3 to DsHidMini."
Write-Host "If the controller is already connected, it should re-enumerate as XInput."
Write-Host "=== DsHidMini driver installation complete ==="
