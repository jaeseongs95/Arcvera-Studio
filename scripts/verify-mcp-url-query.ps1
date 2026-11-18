$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false
$env:CARGO_TERM_COLOR = 'never'
$path = 'src-tauri/src/codex_mcp.rs'
$baseline = '405644e5ae4a273a9bb4c8b7a1b2e4e922e5c04a'
$directory = 'artifacts/native-adapters/mcp-url-query'
New-Item -ItemType Directory -Path $directory -Force | Out-Null
$utf8 = [Text.UTF8Encoding]::new($false, $true)
$fixedBytes = [IO.File]::ReadAllBytes($path)
$fixed = $utf8.GetString($fixedBytes)
$oldLines = @(git show "${baseline}:$path")
if ($LASTEXITCODE -ne 0) { throw 'MCP URL baseline source unavailable' }
$old = ($oldLines -join "`n") + "`n"
function ValidatorRange([string]$source) {
    $start = $source.IndexOf('fn validate_url(', [StringComparison]::Ordinal)
    $end = $source.IndexOf('fn transport_value(', [StringComparison]::Ordinal)
    if ($start -lt 0 -or $end -le $start) { throw 'MCP URL validator boundaries unavailable' }
    return @($start, $end)
}
$fixedRange = ValidatorRange $fixed
$oldRange = ValidatorRange $old
$before = $fixed.Substring(0, $fixedRange[0]) + $old.Substring($oldRange[0], $oldRange[1] - $oldRange[0]) + $fixed.Substring($fixedRange[1])
$testOffset = $fixed.IndexOf('#[cfg(test)]', [StringComparison]::Ordinal)
if ($testOffset -lt $fixedRange[1]) { throw 'Inline regression cases unavailable' }
$sha = { param([byte[]]$bytes) [Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant() }
$testSha = & $sha ($utf8.GetBytes($fixed.Substring($testOffset)))
function RunCases([string]$name) {
    cargo test --lib --locked --manifest-path src-tauri/Cargo.toml credential_query_ -- --test-threads=1 2>&1 | Tee-Object -FilePath "$directory/$name.log" | Out-Host
    $code = $LASTEXITCODE
    $text = [IO.File]::ReadAllText("$directory/$name.log")
    $cases = @([regex]::Matches($text, '(?m)^test [\w:]+(credential_query_\w+) \.\.\. (ok|FAILED)') | ForEach-Object { @{name=$_.Groups[1].Value; result=$_.Groups[2].Value} })
    return @{exitCode=$code; cases=$cases}
}
try {
    [IO.File]::WriteAllText($path, $before, $utf8)
    $beforeTestOffset = $before.IndexOf('#[cfg(test)]', [StringComparison]::Ordinal)
    if ((& $sha ($utf8.GetBytes($before.Substring($beforeTestOffset)))) -ne $testSha) { throw 'Before test bytes differ' }
    $red = RunCases 'before'
} finally {
    [IO.File]::WriteAllBytes($path, $fixedBytes)
    [IO.File]::SetLastWriteTimeUtc($path, [DateTime]::UtcNow)
}
if ((& $sha ([IO.File]::ReadAllBytes($path))) -ne (& $sha $fixedBytes)) { throw 'Fixed source restoration failed' }
$green = RunCases 'fixed'
@{baseline=$baseline; candidate=(git rev-parse HEAD).Trim(); beforeSourceSha256=(& $sha ($utf8.GetBytes($before))); fixedSourceSha256=(& $sha $fixedBytes); testModuleSha256=$testSha; before=$red; fixed=$green; scope='Synthetic RPC boundary; no real Core, credentials or user configuration'} | ConvertTo-Json -Depth 6 | Set-Content "$directory/proof.json" -Encoding utf8NoBOM
$expected = @('credential_query_add_is_rejected_before_rpc_without_echoing_secrets','credential_query_edit_is_rejected_before_write_without_echoing_secrets') | Sort-Object
$failed = @($red.cases | Where-Object result -eq 'FAILED' | ForEach-Object name | Sort-Object)
if ($red.exitCode -eq 0 -or $red.cases.Count -ne 2 -or (Compare-Object $expected $failed)) { throw 'Before did not fail at both credential query boundaries; inspect preserved output' }
if ($green.exitCode -ne 0 -or $green.cases.Count -ne 2 -or @($green.cases | Where-Object result -ne 'ok').Count -ne 0) { throw 'Fixed credential query regressions failed' }
