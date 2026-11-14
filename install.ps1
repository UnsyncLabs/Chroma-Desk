<#
.SYNOPSIS
  chroma-desk installer for Windows.

.DESCRIPTION
  Downloads the latest chroma-desk release (MSI) from github.com/UnsyncLabs/Chroma-Desk, verifies it
  against the release's SHA256SUMS and installs it. Offers to install the official OpenRGB with winget.
  (The Razer-capable openrgb-razer build is Linux-only: on Windows the official OpenRGB works.)

  irm https://raw.githubusercontent.com/UnsyncLabs/Chroma-Desk/main/install.ps1 | iex
  # or, with options:
  & ([scriptblock]::Create((irm https://raw.githubusercontent.com/UnsyncLabs/Chroma-Desk/main/install.ps1))) -Yes

.PARAMETER Yes        Ask nothing (installs OpenRGB too when it is missing).
.PARAMETER Uninstall  Remove chroma-desk.
.PARAMETER Version    chroma-desk tag, e.g. v0.2.0 (default: latest).
.PARAMETER DryRun     Show what would be done without installing anything.
.PARAMETER NoOpenRGB  Do not offer to install OpenRGB.
#>
[CmdletBinding()]
param(
  [switch]$Yes,
  [switch]$Uninstall,
  [string]$Version = "",
  [switch]$DryRun,
  [switch]$NoOpenRGB
)

$ErrorActionPreference = "Stop"
Set-StrictMode -Version Latest

$Repo = "UnsyncLabs/Chroma-Desk"
$Api = "https://api.github.com/repos/$Repo"
$DlPrefix = "https://github.com/$Repo/releases/download/"
$Es = (Get-UICulture).TwoLetterISOLanguageName -eq "es"

function T([string]$en, [string]$es) { if ($Es) { $es } else { $en } }
function Info([string]$m) { Write-Host "==> $m" -ForegroundColor Cyan }
function Warn([string]$m) { Write-Warning $m }

function Confirm-Step([string]$en, [string]$es, [bool]$default = $true) {
  if ($Yes) { return $default }
  $hint = if ($default) { "[Y/n]" } else { "[y/N]" }
  $ans = Read-Host "$(T $en $es) $hint"
  if ([string]::IsNullOrWhiteSpace($ans)) { return $default }
  return $ans -match '^(y|s)'
}

if ($Version -and $Version -notmatch '^[A-Za-z0-9._-]+$') { throw "invalid version: $Version" }
if (-not [Environment]::Is64BitOperatingSystem) { throw (T "Only 64-bit Windows is supported." "Solo se admite Windows de 64 bits.") }
# TLS 1.2+ (Windows PowerShell 5.1 defaults to older protocols).
[Net.ServicePointManager]::SecurityProtocol = [Net.ServicePointManager]::SecurityProtocol -bor [Net.SecurityProtocolType]::Tls12

function Get-Json([string]$url) {
  if (-not $url.StartsWith("https://api.github.com/repos/$Repo/")) { throw "refusing $url" }
  Invoke-RestMethod -Uri $url -Headers @{ Accept = "application/vnd.github+json" } -UseBasicParsing
}

function Get-Release {
  if ($Version) { return Get-Json "$Api/releases/tags/$Version" }
  $all = Get-Json "$Api/releases?per_page=100"
  $r = $all | Where-Object { $_.tag_name -match '^v[0-9]' -and -not $_.prerelease -and -not $_.draft } | Select-Object -First 1
  if (-not $r) { throw (T "No chroma-desk release found yet." "Todavia no hay ningun release de chroma-desk.") }
  return $r
}

function Get-Verified($release, [string]$pattern, [string]$dir) {
  $asset = $release.assets | Where-Object { $_.name -match $pattern } | Select-Object -First 1
  if (-not $asset) { throw "no asset matching $pattern in $($release.tag_name)" }
  $name = $asset.name
  $base = "$DlPrefix$($release.tag_name)/"
  if ($name -notmatch '^[A-Za-z0-9._+-]+$') { throw "unexpected file name: $name" }
  if (-not $asset.browser_download_url.StartsWith($base)) { throw "refusing to download from $($asset.browser_download_url)" }
  $dest = Join-Path $dir $name
  if ($DryRun) { Write-Host "  + download $($asset.browser_download_url) (+ SHA256SUMS check)"; return $dest }
  Info (T "Downloading $name" "Descargando $name")
  Invoke-WebRequest -Uri $asset.browser_download_url -OutFile $dest -UseBasicParsing
  $sums = (Invoke-WebRequest -Uri "${base}SHA256SUMS" -UseBasicParsing).Content -split "`n"
  $line = $sums | Where-Object { $_ -match "^\s*([0-9a-fA-F]{64})\s+\*?$([regex]::Escape($name))\s*$" } | Select-Object -First 1
  if (-not $line) { throw (T "$name is not listed in SHA256SUMS: refusing to install it" "$name no aparece en SHA256SUMS: no lo instalo") }
  $expected = ($line -split '\s+')[0]
  $actual = (Get-FileHash -Algorithm SHA256 -Path $dest).Hash
  if ($actual -ne $expected.ToUpper()) { throw (T "checksum mismatch for $name" "la suma SHA-256 de $name no coincide") }
  Info (T "SHA-256 verified: $name" "SHA-256 verificado: $name")
  return $dest
}

function Find-OpenRGB {
  if (Get-Command OpenRGB.exe -ErrorAction SilentlyContinue) { return $true }
  foreach ($v in @($env:ProgramFiles, ${env:ProgramFiles(x86)}, $env:LOCALAPPDATA)) {
    if ($v -and ((Test-Path (Join-Path $v "OpenRGB\OpenRGB.exe")) -or (Test-Path (Join-Path $v "Programs\OpenRGB\OpenRGB.exe")))) { return $true }
  }
  return $false
}

if ($Uninstall) {
  Info (T "Removing chroma-desk" "Quitando chroma-desk")
  $entry = Get-ItemProperty "HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*", "HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall\*" -ErrorAction SilentlyContinue |
    Where-Object { $_.PSObject.Properties["DisplayName"] -and $_.DisplayName -eq "chroma-desk" } | Select-Object -First 1
  if (-not $entry) { Warn (T "chroma-desk is not installed." "chroma-desk no esta instalado."); exit 0 }
  Write-Host "  + $($entry.UninstallString)"
  if (-not $DryRun -and (Confirm-Step "Remove chroma-desk?" "Quitar chroma-desk?" $true)) {
    # MSI entries look like `MsiExec.exe /I{GUID}`; NSIS ones are a plain path.
    if ($entry.UninstallString -match '\{[0-9A-Fa-f-]+\}') {
      Start-Process msiexec.exe -ArgumentList "/x", $Matches[0], "/passive" -Wait -Verb RunAs
    } else {
      Start-Process -FilePath $entry.UninstallString -ArgumentList "/S" -Wait -Verb RunAs
    }
  }
  exit 0
}

Info (T "Windows x64: installing chroma-desk (the Razer fix is Linux-only; the official OpenRGB is used here)." "Windows x64: instalando chroma-desk (el arreglo Razer es solo de Linux; aqui se usa el OpenRGB oficial).")
if ($DryRun) { Info (T "Dry run: nothing will be installed." "Simulacion: no se instalara nada.") }

if (-not $NoOpenRGB -and -not (Find-OpenRGB)) {
  if (Confirm-Step "OpenRGB was not found. Install it with winget?" "No se encontro OpenRGB. Instalarlo con winget?" $true) {
    if (Get-Command winget -ErrorAction SilentlyContinue) {
      Write-Host "  + winget install OpenRGB"
      if (-not $DryRun) { winget install OpenRGB --accept-package-agreements --accept-source-agreements }
    } else {
      Warn (T "winget is not available: download OpenRGB from https://openrgb.org and install it." "winget no esta disponible: descarga OpenRGB desde https://openrgb.org e instalalo.")
    }
  }
}

$release = Get-Release
Info (T "Installing chroma-desk ($($release.tag_name))" "Instalando chroma-desk ($($release.tag_name))")
$tmp = Join-Path ([IO.Path]::GetTempPath()) ("chroma-desk-" + [guid]::NewGuid().ToString("N"))
New-Item -ItemType Directory -Path $tmp | Out-Null
try {
  $msi = Get-Verified $release '\.msi$' $tmp
  Write-Host "  + msiexec /i $msi /passive"
  if (-not $DryRun) {
    # The MSI installs per machine: Windows asks for permission (UAC).
    $p = Start-Process msiexec.exe -ArgumentList "/i", "`"$msi`"", "/passive" -Wait -Verb RunAs -PassThru
    if ($p.ExitCode -ne 0 -and $p.ExitCode -ne 3010) { throw "msiexec exited with code $($p.ExitCode)" }
  }
} finally {
  Remove-Item -Recurse -Force $tmp -ErrorAction SilentlyContinue
}

Write-Host ""
Info (T "Done. Start chroma-desk from the Start menu. Windows may show a SmartScreen warning the first time (the app is not code-signed): choose 'More info' then 'Run anyway'." "Listo. Abre chroma-desk desde el menu Inicio. Windows puede mostrar un aviso de SmartScreen la primera vez (la app no esta firmada): elige 'Mas informacion' y 'Ejecutar de todos modos'.")
