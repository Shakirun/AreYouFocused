# Sign an unsigned release APK with the standard Android debug keystore (local testing only).
# Play Store and production builds need a real upload keystore — see ANDROID.md.
param(
    [string]$ApkPath = "",
    [string]$OutPath = ""
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent

$defaultApk = Join-Path $repoRoot 'src-tauri\gen\android\app\build\outputs\apk\universal\release\app-universal-release-unsigned.apk'
if (-not $ApkPath) { $ApkPath = $defaultApk }

if (-not (Test-Path $ApkPath)) {
    Write-Error "APK not found: $ApkPath`nBuild first: npm run tauri:android:build:apk"
}

if (-not $env:ANDROID_HOME) {
    Write-Error 'ANDROID_HOME is not set. Run .\scripts\setup-android-env.ps1 and open a new terminal.'
}

$buildToolsRoot = Join-Path $env:ANDROID_HOME 'build-tools'
$buildTools = Get-ChildItem $buildToolsRoot -Directory -ErrorAction SilentlyContinue |
    Sort-Object Name -Descending |
    Select-Object -First 1
if (-not $buildTools) {
    Write-Error "No build-tools under $buildToolsRoot — install Android SDK Build-Tools in Android Studio."
}

$apksigner = Join-Path $buildTools.FullName 'apksigner.bat'
if (-not (Test-Path $apksigner)) {
    Write-Error "apksigner not found: $apksigner"
}

$debugKs = Join-Path $env:USERPROFILE '.android\debug.keystore'
if (-not (Test-Path $debugKs)) {
    Write-Error @"
Debug keystore missing: $debugKs
Run once: npm run tauri:android:build:debug
(or open Android Studio / run any Gradle debug build to create ~/.android/debug.keystore)
"@
}

if (-not $OutPath) {
    $dir = Split-Path $ApkPath -Parent
    $base = [System.IO.Path]::GetFileNameWithoutExtension($ApkPath)
    $OutPath = Join-Path $dir ($base + '-signed-debug.apk')
}

Copy-Item -LiteralPath $ApkPath -Destination $OutPath -Force

& $apksigner sign `
    --ks $debugKs `
    --ks-pass pass:android `
    --key-pass pass:android `
    --ks-key-alias androiddebugkey `
    $OutPath

& $apksigner verify --verbose $OutPath | Out-Host

Write-Host ""
Write-Host "Signed APK: $OutPath"
Write-Host "Install: adb install -r `"$OutPath`""
