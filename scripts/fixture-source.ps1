# Source binding for isolated CI executables. Generated files have a separate manifest.
function Get-FixtureSourceManifest {
    param([string]$Repository)
    Push-Location -LiteralPath $Repository
    try {
        & git diff --quiet HEAD -- . ':(top,exclude)THIRD_PARTY_NOTICES.md'
        if ($LASTEXITCODE -ne 0) { throw 'Tracked source differs from the checkout commit' }
        $files = @(& git -c core.quotepath=false ls-files)
        if ($LASTEXITCODE -ne 0) { throw 'Cannot enumerate tracked source' }
        foreach ($file in $files) {
            if ($file -eq 'THIRD_PARTY_NOTICES.md') { continue }
            $absolute = [IO.Path]::GetFullPath((Join-Path $Repository $file))
            if (-not $absolute.StartsWith($Repository + [IO.Path]::DirectorySeparatorChar, [StringComparison]::OrdinalIgnoreCase) -or
                -not (Test-Path -LiteralPath $absolute -PathType Leaf)) { throw 'Tracked source is not a bounded checkout file' }
            @{ path = $file; sha256 = (Get-FileHash -LiteralPath $absolute -Algorithm SHA256).Hash.ToLowerInvariant() }
        }
    } finally { Pop-Location }
}
