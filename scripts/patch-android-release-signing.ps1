# One-time patch: wire release signing from gen/android/keystore.properties (Tauri 2 docs).
# Safe to re-run; skips if signingConfigs already present.
param(
    [switch]$Force
)

$ErrorActionPreference = 'Stop'

$repoRoot = Split-Path $PSScriptRoot -Parent
$gradleFile = Join-Path $repoRoot 'src-tauri\gen\android\app\build.gradle.kts'

if (-not (Test-Path $gradleFile)) {
    Write-Error "Gradle project missing. Run: npm run tauri:android:init"
}

$content = Get-Content $gradleFile -Raw

if ($content -match 'signingConfigs\s*\{' -and -not $Force) {
    Write-Host 'signingConfigs already present in build.gradle.kts — nothing to do.'
    exit 0
}

if ($content -notmatch 'import java\.util\.Properties') {
    Write-Error 'Unexpected build.gradle.kts layout (missing Properties import). Patch manually — see ANDROID.md.'
}

if ($content -notmatch 'import java\.io\.FileInputStream') {
    $content = $content -replace '(import java\.util\.Properties)', "`$1`nimport java.io.FileInputStream"
}

$signingBlock = @'
    signingConfigs {
        create("release") {
            val keystorePropertiesFile = rootProject.file("keystore.properties")
            val keystoreProperties = Properties()
            if (keystorePropertiesFile.exists()) {
                keystoreProperties.load(FileInputStream(keystorePropertiesFile))
                keyAlias = keystoreProperties["keyAlias"] as String
                keyPassword = keystoreProperties["password"] as String
                storeFile = file(keystoreProperties["storeFile"] as String)
                storePassword = keystoreProperties["password"] as String
            }
        }
    }
'@

if ($content -notmatch 'signingConfigs\s*\{') {
    $content = $content -replace '(\s+)buildTypes\s*\{', ($signingBlock + "`n`$1buildTypes {")
}

if ($content -match 'getByName\("release"\)\s*\{' -and $content -notmatch 'getByName\("release"\)\s*\{[^}]*signingConfig') {
    $content = $content -replace '(getByName\("release"\)\s*\{)', "`$1`n            signingConfig = signingConfigs.getByName(`"release`")"
}

Set-Content -Path $gradleFile -Value $content -NoNewline
Write-Host "Patched: $gradleFile"
Write-Host 'Copy src-tauri/android/keystore.properties.example to src-tauri/gen/android/keystore.properties and set your paths.'
Write-Host 'Release APKs sign only when keystore.properties exists; otherwise Gradle still emits *-unsigned.apk.'
