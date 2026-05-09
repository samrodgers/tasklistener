# Build the Windows .exe installer.
#
# Requires (on a Windows machine):
#   - Visual Studio 2022 with .NET Desktop + Windows App SDK workloads
#   - .NET 8 SDK
#   - Rust with x86_64-pc-windows-msvc target
#   - Inno Setup 6 (https://jrsoftware.org/isinfo.php) — `iscc` on PATH
#
# Usage:
#   pwsh ./scripts/build-windows.ps1 [-Configuration Release]

param(
    [string]$Configuration = "Release",
    [string]$Platform = "x64"
)

$ErrorActionPreference = "Stop"

$Root = (Resolve-Path "$PSScriptRoot/..").Path
$Dist = Join-Path $Root "dist"
New-Item -ItemType Directory -Force -Path $Dist | Out-Null

$RustTarget = if ($Platform -eq "ARM64") { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" }
$RustProfile = $Configuration.ToLower()

Write-Host "==> Building Rust core ($Configuration / $RustTarget)"
Push-Location $Root
try {
    if ($Configuration -eq "Release") {
        cargo build -p tasklistener-ffi --release --target $RustTarget
    } else {
        cargo build -p tasklistener-ffi --target $RustTarget
    }
} finally {
    Pop-Location
}

$DllPath = Join-Path $Root "target/$RustTarget/$RustProfile/tasklistener.dll"
if (-not (Test-Path $DllPath)) {
    throw "Rust dll not produced: $DllPath"
}

Write-Host "==> Building WinUI app ($Configuration / $Platform)"
$ProjectDir = Join-Path $Root "apps/windows/TaskListener"
Push-Location $ProjectDir
try {
    dotnet restore -r "win-$($Platform.ToLower())"
    if ($LASTEXITCODE -ne 0) { throw "dotnet restore failed" }
    dotnet publish -c $Configuration -r "win-$($Platform.ToLower())" --self-contained true `
        -p:PublishSingleFile=false -p:PublishReadyToRun=true `
        -o "$Root/build/win-publish/$Platform"
    if ($LASTEXITCODE -ne 0) { throw "dotnet publish failed" }
} finally {
    Pop-Location
}

# Inno Setup compiles the publish output into a single .exe installer.
$IscPath = Get-Command iscc -ErrorAction SilentlyContinue
if (-not $IscPath) {
    throw "Inno Setup not installed. Install from https://jrsoftware.org/isinfo.php"
}

Write-Host "==> Building installer with Inno Setup"
$IssFile = Join-Path $Root "scripts/windows-installer.iss"
& iscc.exe `
    "/DAppRoot=$Root" `
    "/DPublishDir=$Root/build/win-publish/$Platform" `
    "/DOutputDir=$Dist" `
    "/DPlatform=$Platform" `
    $IssFile

Write-Host ""
Write-Host "Built installer in $Dist"
Get-ChildItem $Dist -Filter "*.exe" | ForEach-Object { Write-Host "  $($_.FullName)" }
