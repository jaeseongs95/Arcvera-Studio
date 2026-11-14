# CI development executable only. No installation, service startup or user profile reuse.
param([ValidateSet('debug','release')][string]$FixtureProfile = 'debug')
$ErrorActionPreference = 'Stop'
if ($env:CI -ne 'true' -or $env:GITHUB_ACTIONS -ne 'true' -or -not $IsWindows) { throw 'Use the isolated Windows CI checkout for this fixture build' }
$fixtureRepo = [IO.Path]::GetFullPath((Get-Location).Path)
$fixtureRoot = Join-Path $fixtureRepo 'artifacts/native-adapters'
if ($FixtureProfile -eq 'release') { $fixtureRoot = Join-Path $fixtureRoot 'release-profile' }
$fixtureApp = Join-Path $fixtureRoot 'app'
if (Test-Path -LiteralPath $fixtureApp) { throw 'Existing executable evidence must not be overwritten' }
$fixtureCommit = (& git rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0 -or $fixtureCommit -notmatch '^[0-9a-f]{40}$') { throw 'Cannot bind the checkout commit' }
& git diff --quiet HEAD --
if ($LASTEXITCODE -ne 0) { throw 'Tracked source differs from the checkout commit' }
$fixtureIdentifier = 'io.agentgrid.desktop.fixture' + $fixtureCommit.Substring(0, 12)
if ($FixtureProfile -eq 'release') { $fixtureIdentifier += 'release' }
[IO.Directory]::CreateDirectory($fixtureRoot) | Out-Null
$fixtureConfig = Join-Path $fixtureRoot 'fixture-config.json'
@{ identifier = $fixtureIdentifier } | ConvertTo-Json | Set-Content -LiteralPath $fixtureConfig -Encoding utf8NoBOM
$fixtureArgs = @('node_modules/@tauri-apps/cli/tauri.js', 'build')
if ($FixtureProfile -eq 'debug') { $fixtureArgs += '--debug' }
$fixtureArgs += @('--no-bundle', '--ci', '--config', $fixtureConfig, '--', '--locked', '-j1')
& node @fixtureArgs
if ($LASTEXITCODE -ne 0) { throw 'Isolated development executable build failed' }
& git diff --quiet HEAD --
if ($LASTEXITCODE -ne 0) { throw 'Build modified tracked source; refusing a source-bound fixture' }
$fixtureExe = Join-Path $fixtureRepo "src-tauri/target/$FixtureProfile/arcvera-studio.exe"
if (-not (Test-Path -LiteralPath $fixtureExe -PathType Leaf)) { throw 'Expected development executable is absent' }
[IO.Directory]::CreateDirectory($fixtureApp) | Out-Null
Copy-Item -LiteralPath $fixtureExe -Destination (Join-Path $fixtureApp 'arcvera-studio.exe')
# Derive sidecars from the actual resource map instead of maintaining another file list.
$fixtureTauri = Get-Content -LiteralPath (Join-Path $fixtureRepo 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json
foreach ($fixtureResource in $fixtureTauri.bundle.resources.PSObject.Properties) {
    $fixtureSource = [IO.Path]::GetFullPath((Join-Path (Join-Path $fixtureRepo 'src-tauri') $fixtureResource.Name))
    $fixtureDestination = [IO.Path]::GetFullPath((Join-Path $fixtureApp $fixtureResource.Value))
    if ((-not $fixtureSource.StartsWith($fixtureRepo + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) -or
        (-not $fixtureDestination.StartsWith($fixtureApp + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) -or
        (-not (Test-Path -LiteralPath $fixtureSource -PathType Leaf))) { throw 'Resource map escaped the checkout/app or is not a file' }
    [IO.Directory]::CreateDirectory([IO.Path]::GetDirectoryName($fixtureDestination)) | Out-Null
    Copy-Item -LiteralPath $fixtureSource -Destination $fixtureDestination
}
$fixtureFiles = @(& git -c core.quotepath=false ls-files)
if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate tracked source' }
$fixtureSourceManifest = foreach ($fixturePath in $fixtureFiles) {
    $fixtureAbsolute = [IO.Path]::GetFullPath((Join-Path $fixtureRepo $fixturePath))
    if ((-not $fixtureAbsolute.StartsWith($fixtureRepo + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase)) -or
        (-not (Test-Path -LiteralPath $fixtureAbsolute -PathType Leaf))) { throw 'Tracked source is not a bounded checkout file' }
    @{ path = $fixturePath; sha256 = (Get-FileHash -LiteralPath $fixtureAbsolute -Algorithm SHA256).Hash.ToLowerInvariant() }
}
$fixtureSourceManifest | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $fixtureRoot 'source-manifest.json') -Encoding utf8NoBOM
$fixtureGenerated = @(Get-ChildItem -LiteralPath (Join-Path $fixtureRepo 'dist') -File -Recurse) + @(Get-Item -LiteralPath (Join-Path $fixtureRepo 'THIRD_PARTY_NOTICES.md'), $fixtureConfig)
$fixtureGenerated | ForEach-Object { @{ path = [IO.Path]::GetRelativePath($fixtureRepo, $_.FullName); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() } } | ConvertTo-Json -Depth 3 | Set-Content -LiteralPath (Join-Path $fixtureRoot 'generated-manifest.json') -Encoding utf8NoBOM
@{ schemaVersion = '1.0.0'; commit = $fixtureCommit; identifier = $fixtureIdentifier;
    buildKind = "development-$FixtureProfile-no-bundle"; buildProfile = $FixtureProfile; buildArguments = $fixtureArgs;
    executable = 'app/arcvera-studio.exe'; executableSha256 = (Get-FileHash -LiteralPath (Join-Path $fixtureApp 'arcvera-studio.exe') -Algorithm SHA256).Hash.ToLowerInvariant();
    sourceManifestSha256 = (Get-FileHash -LiteralPath (Join-Path $fixtureRoot 'source-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant();
    generatedManifestSha256 = (Get-FileHash -LiteralPath (Join-Path $fixtureRoot 'generated-manifest.json') -Algorithm SHA256).Hash.ToLowerInvariant();
    resourceRoot = 'app'; verification = 'build-and-hashes-only'; realCoreIntegration = 'NOT_RUN'
} | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $fixtureRoot 'fixture.json') -Encoding utf8NoBOM
