param(
  [ValidateRange(30,14400)][int]$Seconds = 1800,
  [string]$OutputDirectory = 'artifacts/native-resource'
)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$utf8 = [Text.UTF8Encoding]::new($false)
$root = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $root) { throw 'Output directory must be new for this observation.' }
[IO.Directory]::CreateDirectory($root) | Out-Null
$raw = Join-Path $root 'process-samples.jsonl'
$summaryPath = Join-Path $root 'observation.json'
$exe = Join-Path $root 'arcvera-studio.exe'
$reportPath = Join-Path $root 'self-test.json'
$summary = [ordered]@{
  runId = [Guid]::NewGuid().ToString(); workflowCommit = $env:GITHUB_SHA; ciRun = $env:GITHUB_RUN_ID
  executableSourceCommit = '3460042da2024c7a3ecbdcead1655c98bbf06241'
  expectedSha256 = '60ef6aeda944b6ba65baa14c5124c4d04abe2e25df7a1e2a087ce232534aeb49'
  requestedSeconds = $Seconds; sampleIntervalSeconds = 5; observedSamples = 0
  startedAt = [DateTime]::UtcNow.ToString('o'); completed = $false; missingMeasurements = @()
  scope = 'Public v1.8.0 headless native PTY suite; no GUI, WebView, actual CLI/provider or physical IME'
}
function Append-Sample($value) {
  [IO.File]::AppendAllText($raw, ($value | ConvertTo-Json -Depth 8 -Compress) + "`n", $utf8)
}
$clock = [Diagnostics.Stopwatch]::StartNew()
$probe = $null
$previousSample = $null
try {
  $os = Get-CimInstance Win32_OperatingSystem
  $drive = [IO.DriveInfo]::new([IO.Path]::GetPathRoot($root))
  $summary.environment = @{ os = $os.Version; powershell = $PSVersionTable.PSVersion.ToString(); logicalCpu = [Environment]::ProcessorCount; freeMemoryKB = $os.FreePhysicalMemory; freeDiskBytes = $drive.AvailableFreeSpace }
  if ($os.FreePhysicalMemory -lt 2GB/1KB -or $drive.AvailableFreeSpace -lt 1GB) { throw 'Insufficient initial CI headroom.' }
  Invoke-WebRequest -Uri 'https://github.com/jaeseongs95/Arcvera-Studio/releases/download/v1.8.0/arcvera-studio.exe' -OutFile $exe
  $summary.executableSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $exe).Hash.ToLowerInvariant()
  $summary.executableBytes = (Get-Item -LiteralPath $exe).Length
  if ($summary.executableBytes -ne 16093184 -or $summary.executableSha256 -cne $summary.expectedSha256) { throw 'Pinned executable identity mismatch.' }
  $arguments = @('--self-test','--duration-seconds',[string]$Seconds,'--report',('"{0}"' -f $reportPath))
  $summary.arguments = $arguments
  $summary.processStartedAt = [DateTime]::UtcNow.ToString('o')
  $probe = Start-Process -FilePath $exe -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $root 'stdout.txt') -RedirectStandardError (Join-Path $root 'stderr.txt')
  $created = $probe.StartTime.ToUniversalTime().ToString('o')
  $summary.processId = $probe.Id; $summary.processCreatedAt = $created
  $running = [Diagnostics.Stopwatch]::StartNew()
  while (!$probe.HasExited) {
    $probe.Refresh()
    if ($probe.HasExited) { break }
    $sample = [ordered]@{ kind = 'sample'; runId = $summary.runId; processId = $probe.Id; processCreatedAt = $created; utc = [DateTime]::UtcNow.ToString('o'); elapsedMs = $running.Elapsed.TotalMilliseconds; privateBytes = $null; workingSetBytes = $null; handles = $null; cpuSeconds = $null; error = $null }
    try {
      if ($probe.StartTime.ToUniversalTime().ToString('o') -cne $created) { throw 'Process generation changed.' }
      $sample.privateBytes = $probe.PrivateMemorySize64; $sample.workingSetBytes = $probe.WorkingSet64
      $sample.handles = $probe.HandleCount; $sample.cpuSeconds = $probe.TotalProcessorTime.TotalSeconds
    } catch {
      $sample.error = $_.Exception.Message
      $summary.missingMeasurements += $sample.error
    }
    Append-Sample $sample
    $summary.observedSamples++
    if ($null -ne $sample.error) { throw 'Own-process measurement failed; partial sample retained.' }
    if ($null -ne $previousSample -and $sample.elapsedMs - $previousSample -gt 15000) { throw 'Process sample scheduling gap exceeded 15seconds.' }
    $previousSample = $sample.elapsedMs
    if ($running.Elapsed.TotalSeconds -gt $Seconds + 900) { throw 'Bounded native observation deadline exceeded.' }
    # Waiting on this exact process handle neither enumerates nor touches peers.
    $null = $probe.WaitForExit(5000)
  }
  $probe.WaitForExit()
  $summary.exitCode = $probe.ExitCode
  if ($summary.exitCode -ne 0) { throw "Native suite exit code $($summary.exitCode)." }
  $report = Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
  $summary.reportSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $reportPath).Hash.ToLowerInvariant()
  if ($report.fail -ne 0 -or $report.errors.Count -ne 0 -or $report.pass -ne $report.tests -or $report.iterations -lt 1 -or $report.durationSeconds -lt $Seconds) { throw 'Native suite report did not satisfy NR02.' }
  if ($summary.observedSamples -lt [Math]::Ceiling($Seconds/5) + 1) { throw 'Insufficient own-process samples.' }
  $memory = @($report.sampleMemory)
  if ($memory.Count -lt 3) { throw 'Missing native in-process resource observations.' }
  for ($index = 0; $index -lt $memory.Count; $index++) {
    foreach ($name in @('workingSetBytes','privateBytes','handles','ownedProcesses')) {
      if ($null -eq $memory[$index].$name) {
        $summary.missingMeasurements += "sampleMemory[$index].$name"
        if (!($index -eq 0 -and $name -eq 'ownedProcesses')) { throw 'Missing mandatory in-process resource metric.' }
      }
    }
  }
  if ($memory[-1].ownedProcesses -ne 1) { throw 'Owned job did not return to executable-only baseline.' }
  $tree = $report.processTree
  if ($null -eq $tree -or $tree.processes.Count -lt 1 -or @($tree.processes | Where-Object { $_.exitSignaled -ne $true }).Count -ne 0) { throw 'Fixture process identity cleanup was not demonstrated.' }
  $summary.completed = $true
} catch {
  $summary.error = $_.Exception.Message
  throw
} finally {
  if ($null -ne $probe) {
    if (!$probe.HasExited) {
      # Only the handle returned by our Start-Process is eligible for termination.
      $summary.forcedOwnProcessTermination = $true
      try { $probe.Kill(); $summary.ownProcessExited = $probe.WaitForExit(10000) } catch { $summary.cleanupError = $_.Exception.Message }
    } else { $summary.ownProcessExited = $true }
  }
  $summary.finishedAt = [DateTime]::UtcNow.ToString('o'); $summary.elapsedMs = $clock.Elapsed.TotalMilliseconds
  [IO.File]::WriteAllText($summaryPath, ($summary | ConvertTo-Json -Depth 10) + "`n", $utf8)
  Append-Sample @{ kind = 'finish'; summary = $summary }
}
