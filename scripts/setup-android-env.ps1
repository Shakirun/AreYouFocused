# Detect Android Studio JBR and SDK paths on Windows.
# Prints session export commands; optionally sets User env vars (no admin).
param(
    [switch]$SetUserEnv
)

$ErrorActionPreference = 'Stop'

$jbrCandidates = @(
    Join-Path $env:ProgramFiles 'Android\Android Studio\jbr'
    Join-Path $env:LOCALAPPDATA 'Programs\Android Studio\jbr'
)

function Get-AndroidStudioSdkPath {
    $roots = @(
        (Join-Path $env:APPDATA 'Google')
        (Join-Path $env:LOCALAPPDATA 'Google')
    )
    foreach ($root in $roots) {
        if (-not (Test-Path $root)) { continue }
        $studioDirs = Get-ChildItem -Path $root -Filter 'AndroidStudio*' -Directory -ErrorAction SilentlyContinue
        foreach ($dir in $studioDirs) {
            $sdkXml = Join-Path $dir.FullName 'options\android.sdk.path.xml'
            if (-not (Test-Path $sdkXml)) { continue }
            $content = Get-Content $sdkXml -Raw
            if ($content -match '<option name="ANDROID_SDK_HOME" value="([^"]+)"') {
                return $Matches[1].Replace('/', '\')
            }
        }
    }
    return $null
}

function Test-SdkPopulated {
    param([string]$SdkRoot)
    if (-not (Test-Path $SdkRoot)) { return $false }
    $marker = Join-Path $SdkRoot 'platform-tools\adb.exe'
    return Test-Path $marker
}

function Get-SdkComponentStatus {
    param([string]$SdkRoot)
    if (-not (Test-Path $SdkRoot)) {
        return @{ Exists = $false; Populated = $false; Missing = @('SDK folder') }
    }
    $checks = @{
        'platform-tools' = Join-Path $SdkRoot 'platform-tools\adb.exe'
        'build-tools'    = Join-Path $SdkRoot 'build-tools'
        'platforms'      = Join-Path $SdkRoot 'platforms'
        'ndk'            = Join-Path $SdkRoot 'ndk'
        'cmdline-tools'  = Join-Path $SdkRoot 'cmdline-tools'
    }
    $missing = @()
    foreach ($name in $checks.Keys) {
        $path = $checks[$name]
        if (-not (Test-Path $path)) { $missing += $name }
    }
    @{
        Exists    = $true
        Populated = (Test-Path $checks['platform-tools'])
        Missing   = $missing
    }
}

$sdkCandidates = @(
    $env:ANDROID_HOME
    (Get-AndroidStudioSdkPath)
    (Join-Path $env:LOCALAPPDATA 'Android\Sdk')
    (Join-Path $env:USERPROFILE 'AppData\Local\Android\Sdk')
    'C:\Android\Sdk'
    (Join-Path $env:ProgramFiles 'Android\Sdk')
    (Join-Path ${env:ProgramFiles(x86)} 'Android\android-sdk')
    (Join-Path $env:USERPROFILE 'Android\Sdk')
) | Where-Object { $_ } | Select-Object -Unique

$javaHome = $null
foreach ($candidate in $jbrCandidates) {
    $javaExe = Join-Path $candidate 'bin\java.exe'
    if (Test-Path $javaExe) {
        $javaHome = $candidate
        break
    }
}

if (-not $javaHome -and $env:JAVA_HOME) {
    $fromEnv = Join-Path $env:JAVA_HOME 'bin\java.exe'
    if (Test-Path $fromEnv) {
        $javaHome = $env:JAVA_HOME
    }
}

$pathJava = Get-Command java -ErrorAction SilentlyContinue

$androidHome = $null
foreach ($candidate in $sdkCandidates) {
    if (Test-Path $candidate) {
        $androidHome = $candidate
        break
    }
}

if (-not $androidHome) {
    $androidHome = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
}

$sdkStatus = Get-SdkComponentStatus -SdkRoot $androidHome

Write-Host '=== AreYouFocused Android environment (Windows) ===' -ForegroundColor Cyan
Write-Host ''

Write-Host 'JBR candidates checked:'
foreach ($candidate in $jbrCandidates) {
    $found = Test-Path (Join-Path $candidate 'bin\java.exe')
    Write-Host "  $candidate : $(if ($found) { 'FOUND' } else { 'not found' })"
}

Write-Host ''
Write-Host 'SDK candidates checked:'
foreach ($candidate in $sdkCandidates) {
    $exists = Test-Path $candidate
    $populated = if ($exists) { Test-SdkPopulated $candidate } else { $false }
    $label = if (-not $exists) { 'not found' } elseif ($populated) { 'FOUND (has platform-tools)' } else { 'folder exists but empty / incomplete' }
    Write-Host "  $candidate : $label"
}

Write-Host ''
Write-Host "JAVA_HOME (current): $(if ($env:JAVA_HOME) { $env:JAVA_HOME } else { '(not set)' })"
Write-Host "ANDROID_HOME (current): $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { '(not set)' })"
Write-Host "java on PATH: $(if ($pathJava) { $pathJava.Source } else { 'not found' })"
Write-Host "Selected SDK: $androidHome"
Write-Host ''

if (-not $javaHome) {
    Write-Host 'Java/JBR not found.' -ForegroundColor Yellow
    Write-Host 'Install Android Studio, then re-run this script:'
    Write-Host '  winget install Google.AndroidStudio'
    Write-Host ''
    Write-Host 'After install, open Android Studio once, then set JAVA_HOME to one of:'
    foreach ($candidate in $jbrCandidates) {
        Write-Host "  $candidate"
    }
    exit 1
}

Write-Host "Detected JBR (set JAVA_HOME to this): $javaHome" -ForegroundColor Green
Write-Host "Detected SDK path: $androidHome" -ForegroundColor $(if ($sdkStatus.Populated) { 'Green' } else { 'Yellow' })
Write-Host ''

if (-not $sdkStatus.Populated) {
    Write-Host 'SDK is missing components. Install them before tauri android init:' -ForegroundColor Yellow
    Write-Host '  1. Open Android Studio (first launch completes SDK download)'
    Write-Host '  2. Settings - Languages and Frameworks - Android SDK'
    Write-Host '  3. Note "Android SDK Location" (should match path above)'
    Write-Host '  4. SDK Platforms tab: Android 14 (API 34) or newer'
    Write-Host '  5. SDK Tools tab: Android SDK Build-Tools, NDK (Side by side),'
    Write-Host '     Android SDK Platform-Tools, Android SDK Command-line Tools'
    Write-Host '  6. Apply, OK, then re-run this script'
    Write-Host ''
    Write-Host 'Alternative: when tauri android init asks to install cmdline-tools, answer yes.'
    Write-Host ''
    if ($sdkStatus.Missing.Count -gt 0) {
        Write-Host "Missing under $androidHome : $($sdkStatus.Missing -join ', ')"
        Write-Host ''
    }
}

Write-Host 'Session exports (current terminal only):' -ForegroundColor Cyan
Write-Host ('$env:JAVA_HOME = "' + $javaHome + '"')
Write-Host ('$env:ANDROID_HOME = "' + $androidHome + '"')
Write-Host '$env:PATH += ";$env:ANDROID_HOME\platform-tools"'
Write-Host ''

if ($SetUserEnv) {
    [Environment]::SetEnvironmentVariable('JAVA_HOME', $javaHome, 'User')
    [Environment]::SetEnvironmentVariable('ANDROID_HOME', $androidHome, 'User')

    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    $platformTools = Join-Path $androidHome 'platform-tools'
    if ($userPath -notlike "*$platformTools*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$platformTools", 'User')
    }

    $javaBin = Join-Path $javaHome 'bin'
    $userPath = [Environment]::GetEnvironmentVariable('Path', 'User')
    if ($userPath -notlike "*$javaBin*") {
        [Environment]::SetEnvironmentVariable('Path', "$userPath;$javaBin", 'User')
    }

    Write-Host 'Set User environment variables (JAVA_HOME, ANDROID_HOME, PATH).' -ForegroundColor Green
    Write-Host 'Open a new PowerShell window, then run: java -version'
    if (-not $sdkStatus.Populated) {
        Write-Host 'ANDROID_HOME is set, but SDK components are still missing. Install via Android Studio (see above).' -ForegroundColor Yellow
    }
} else {
    Write-Host 'To persist for your user account (no admin):'
    Write-Host '  .\scripts\setup-android-env.ps1 -SetUserEnv'
}

Write-Host ''
Write-Host 'Verify in a new terminal:'
Write-Host '  java -version'
Write-Host '  $env:JAVA_HOME'
Write-Host '  $env:ANDROID_HOME'
if ($sdkStatus.Populated) {
    Write-Host '  adb version'
}

if (-not $sdkStatus.Populated) {
    exit 2
}
