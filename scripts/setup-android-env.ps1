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

$sdkDefault = Join-Path $env:LOCALAPPDATA 'Android\Sdk'
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

Write-Host '=== AreYouFocused Android environment (Windows) ===' -ForegroundColor Cyan
Write-Host ''

Write-Host 'JBR candidates checked:'
foreach ($candidate in $jbrCandidates) {
    $found = Test-Path (Join-Path $candidate 'bin\java.exe')
    Write-Host "  $candidate : $(if ($found) { 'FOUND' } else { 'not found' })"
}

Write-Host ''
Write-Host "JAVA_HOME (current): $(if ($env:JAVA_HOME) { $env:JAVA_HOME } else { '(not set)' })"
Write-Host "ANDROID_HOME (current): $(if ($env:ANDROID_HOME) { $env:ANDROID_HOME } else { '(not set)' })"
Write-Host "java on PATH: $(if ($pathJava) { $pathJava.Source } else { 'not found' })"
Write-Host "Default SDK: $sdkDefault : $(if (Test-Path $sdkDefault) { 'FOUND' } else { 'not found' })"
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

$androidHome = if ($env:ANDROID_HOME -and (Test-Path $env:ANDROID_HOME)) {
    $env:ANDROID_HOME
} elseif (Test-Path $sdkDefault) {
    $sdkDefault
} else {
    $sdkDefault
}

Write-Host "Detected JBR (set JAVA_HOME to this): $javaHome" -ForegroundColor Green
Write-Host ''

Write-Host 'Session exports (current terminal only):' -ForegroundColor Cyan
Write-Host '$env:JAVA_HOME = "' + $javaHome + '"'
Write-Host '$env:ANDROID_HOME = "' + $androidHome + '"'
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
} else {
    Write-Host 'To persist for your user account (no admin):'
    Write-Host '  .\scripts\setup-android-env.ps1 -SetUserEnv'
}

Write-Host ''
Write-Host 'Verify in a new terminal:'
Write-Host '  java -version'
Write-Host '  $env:JAVA_HOME'
Write-Host '  $env:ANDROID_HOME'
