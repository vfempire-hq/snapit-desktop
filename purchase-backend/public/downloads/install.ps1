# ==========================================================================
# SnapIT — one-line Windows install (prebuilt).
#
#   iex(iwr -useb https://snapit.vfempire.com/downloads/install.ps1).Content
#
# Prebuilt installers land here after every tagged release. Until v0.1.0
# ships to the CDN R2 bucket this script points at the latest GitHub Release
# artefact instead.
# ==========================================================================
$ErrorActionPreference = 'Stop'
$owner = 'vfempire-hq'
$repo  = 'snapit-desktop'

Write-Host ''
Write-Host '  SnapIT  ' -BackgroundColor DarkYellow -ForegroundColor Black
Write-Host '  Own your photo library.'
Write-Host ''

# Prefer the CDN installer if it exists (post-M4). Otherwise use the latest
# GitHub Release artefact — always a legitimate signed build.
$cdn = "https://snapit.vfempire.com/downloads/snapit-latest-x64-setup.exe"
$release = "https://api.github.com/repos/$owner/$repo/releases/latest"

$tmp = Join-Path $env:TEMP "snapit-install"
New-Item -ItemType Directory -Force -Path $tmp | Out-Null
$exe = "$tmp\snapit-setup.exe"

$got = $false
try {
    Invoke-WebRequest -Uri $cdn -OutFile $exe -UseBasicParsing
    $got = $true
} catch { }

if (-not $got) {
    Write-Host 'CDN copy not up yet, using latest GitHub Release…'
    $latest = Invoke-RestMethod -Uri $release
    $asset = $latest.assets | Where-Object { $_.name -like '*-setup.exe' } | Select-Object -First 1
    if (-not $asset) { throw 'no Windows setup asset in latest release yet' }
    Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $exe -UseBasicParsing
}

Write-Host '==> Installing…'
Start-Process -Wait -FilePath $exe -ArgumentList '/S'
Write-Host '==> Done. Search for SnapIT in Start.'
