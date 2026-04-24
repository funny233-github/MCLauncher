#Requires -Version 5.1
[CmdletBinding()]
param(
    [switch]$Uninstall,
    [switch]$Help
)

$ErrorActionPreference = "Stop"

$Repo = "funny233-github/MCLauncher"
$BinaryName = "gluon"
$InstallDir = Join-Path $env:LOCALAPPDATA "Programs" "Gluon"

$ScriptDir = Split-Path -Parent $MyInvocation.MyCommand.Path
$ConfFile = Join-Path $ScriptDir "install.conf"

if (Test-Path $ConfFile) {
    Get-Content $ConfFile | Where-Object { $_ -match "^[A-Z_]+=" } | ForEach-Object {
        $key, $value = $_ -split "=", 2
        switch ($key) {
            "REPO"        { $Repo = $value }
            "BINARY_NAME" { $BinaryName = $value }
        }
    }
}

if ($Help) {
    Write-Host @"
Usage: .\install.ps1 [OPTIONS]

Install or uninstall Gluon Minecraft Launcher.

Options:
    -Uninstall    Remove Gluon from the system
    -Help         Show this help message

Install path: $InstallDir\$BinaryName.exe
"@
    exit 0
}

function Write-Info($msg)  { Write-Host -ForegroundColor Cyan "[INFO]  $msg" }
function Write-Warn($msg)  { Write-Host -ForegroundColor Yellow "[WARN]  $msg" }
function Write-Err($msg)   { Write-Host -ForegroundColor Red "[ERROR] $msg"; exit 1 }

function Check-Conflict {
    $existing = Get-Command $BinaryName -ErrorAction SilentlyContinue
    if ($existing) {
        $cargoPath = Join-Path $env:USERPROFILE ".cargo\bin\$BinaryName.exe"
        if ($existing.Source -eq $cargoPath) {
            Write-Err "Found $BinaryName at $($existing.Source) (installed via cargo). Run 'cargo uninstall $BinaryName' first to avoid conflicts."
        }
        elseif ($existing.Source -ne (Join-Path $InstallDir "$BinaryName.exe")) {
            Write-Warn "Found $BinaryName at $($existing.Source), which is not the script-managed path."
        }
    }
}

function Add-To-Path {
    $pathParts = $env:PATH -split ";"
    if ($InstallDir -notin $pathParts) {
        $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($userPath -notlike "*$InstallDir*") {
            [Environment]::SetEnvironmentVariable(
                "PATH",
                "$InstallDir;$userPath",
                "User"
            )
            Write-Info "Added $InstallDir to user PATH."
            Write-Info "Restart your terminal to apply the change."
        }
    }
    $env:PATH = "$InstallDir;$env:PATH"
}

function Uninstall-Gluon {
    $target = Join-Path $InstallDir "$BinaryName.exe"
    if (Test-Path $target) {
        Remove-Item $target -Force
        Write-Info "Removed $target"

        $dirEmpty = @(Get-ChildItem $InstallDir -ErrorAction SilentlyContinue).Count -eq 0
        if ($dirEmpty) {
            Remove-Item $InstallDir -Force
            Write-Info "Removed empty directory $InstallDir"
        }

        $userPath = [Environment]::GetEnvironmentVariable("PATH", "User")
        if ($userPath -like "*$InstallDir*") {
            $newPath = ($userPath -split ";" | Where-Object { $_ -ne $InstallDir }) -join ";"
            [Environment]::SetEnvironmentVariable("PATH", $newPath, "User")
            Write-Info "Removed $InstallDir from user PATH."
        }

        Write-Info "Gluon has been uninstalled."
    }
    else {
        Write-Warn "Gluon is not installed at $target"
    }
    exit 0
}

function Get-Platform {
    $os = ""
    $arch = ""

    if ($IsWindows -or $env:OS -match "Windows") {
        $os = "windows"
    }
    else {
        Write-Err "This script is designed for Windows. Use install.sh for Unix systems."
    }

    switch ($env:PROCESSOR_ARCHITECTURE) {
        "AMD64"  { $arch = "amd64" }
        "ARM64"  { $arch = "arm64" }
        default  { Write-Err "Unsupported architecture: $env:PROCESSOR_ARCHITECTURE" }
    }

    return "${os}_${arch}"
}

function Install-Gluon {
    Check-Conflict

    $platform = Get-Platform
    $assetName = "$BinaryName-$platform.exe"
    $apiUrl = "https://api.github.com/repos/$Repo/releases/latest"

    Write-Info "Fetching latest release info..."
    $release = Invoke-RestMethod -Uri $apiUrl -Headers @{ "User-Agent" = "gluon-installer" }

    $tagName = $release.tag_name
    if (-not $tagName) {
        Write-Err "Could not parse tag name from GitHub API response."
    }

    $asset = $release.assets | Where-Object { $_.name -eq $assetName } | Select-Object -First 1
    if (-not $asset) {
        Write-Err "Could not find asset '$assetName' in latest release $tagName."
    }

    $downloadUrl = $asset.browser_download_url

    New-Item -ItemType Directory -Path $InstallDir -Force | Out-Null

    $target = Join-Path $InstallDir "$BinaryName.exe"
    $tmpFile = Join-Path $env:TEMP "$BinaryName-$tagName.exe"

    Write-Info "Downloading Gluon $tagName for $platform..."
    try {
        Invoke-WebRequest -Uri $downloadUrl -OutFile $tmpFile -UseBasicParsing
    }
    catch {
        Remove-Item $tmpFile -Force -ErrorAction SilentlyContinue
        Write-Err "Download failed: $_"
    }

    Move-Item -Path $tmpFile -Destination $target -Force

    Write-Info "Installed Gluon to $target"

    $pathParts = $env:PATH -split ";"
    if ($InstallDir -notin $pathParts) {
        Write-Warn "$InstallDir is not in your PATH."
        Add-To-Path
    }

    Write-Info "Run '$BinaryName --help' to get started."
}

if ($Uninstall) {
    Uninstall-Gluon
}
else {
    Install-Gluon
}