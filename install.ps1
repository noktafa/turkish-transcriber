# Turkish Transcriber - Windows Installer
# Usage: irm https://raw.githubusercontent.com/noktafa/turkish-transcriber/main/install.ps1 | iex

$ErrorActionPreference = "Stop"

$repo = "noktafa/turkish-transcriber"
$installDir = "$env:LOCALAPPDATA\turkish-transcriber"

Write-Host ""
Write-Host "  Turkish Transcriber Installer" -ForegroundColor Cyan
Write-Host "  =============================" -ForegroundColor Cyan
Write-Host ""

# Get latest release info from GitHub API
Write-Host "  Fetching latest release..." -ForegroundColor Yellow
$release = Invoke-RestMethod "https://api.github.com/repos/$repo/releases/latest"
$version = $release.tag_name
$asset = $release.assets | Where-Object { $_.name -like "*windows-x64*" } | Select-Object -First 1

if (-not $asset) {
    Write-Host "  ERROR: No Windows x64 asset found in $version" -ForegroundColor Red
    exit 1
}

Write-Host "  Found $version" -ForegroundColor Green

# Download
$zipUrl = $asset.browser_download_url
$zipFile = "$env:TEMP\turkish-transcriber.zip"

Write-Host "  Downloading $($asset.name)..." -ForegroundColor Yellow
Invoke-WebRequest -Uri $zipUrl -OutFile $zipFile -UseBasicParsing

# Extract
if (Test-Path $installDir) {
    Write-Host "  Removing previous installation..." -ForegroundColor Yellow
    Remove-Item $installDir -Recurse -Force
}

Write-Host "  Extracting to $installDir..." -ForegroundColor Yellow
Expand-Archive -Path $zipFile -DestinationPath $installDir -Force

# Handle nested folder (zip might contain a subfolder)
$nested = Get-ChildItem $installDir -Directory | Select-Object -First 1
if ($nested -and (Test-Path "$($nested.FullName)\turkish-transcriber.exe")) {
    Get-ChildItem $nested.FullName | Move-Item -Destination $installDir -Force
    Remove-Item $nested.FullName -Recurse -Force
}

# Verify exe exists
$exe = Join-Path $installDir "turkish-transcriber.exe"
if (-not (Test-Path $exe)) {
    Write-Host "  ERROR: turkish-transcriber.exe not found after extraction" -ForegroundColor Red
    exit 1
}

# Add to PATH
$userPath = [Environment]::GetEnvironmentVariable("Path", "User")
if ($userPath -notlike "*$installDir*") {
    Write-Host "  Adding to PATH..." -ForegroundColor Yellow
    [Environment]::SetEnvironmentVariable("Path", "$userPath;$installDir", "User")
    $env:Path = "$env:Path;$installDir"
}

# Cleanup
Remove-Item $zipFile -Force -ErrorAction SilentlyContinue

Write-Host ""
Write-Host "  Installed: turkish-transcriber $version" -ForegroundColor Green
Write-Host "  Location:  $installDir" -ForegroundColor Gray
Write-Host "  Command:   turkish-transcriber <audio-file>" -ForegroundColor Gray
Write-Host ""
Write-Host "  Restart your terminal, then run:" -ForegroundColor Yellow
Write-Host "    turkish-transcriber recording.mp3" -ForegroundColor White
Write-Host ""
