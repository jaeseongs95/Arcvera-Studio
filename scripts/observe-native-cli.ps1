param([string]$OutputDirectory = 'artifacts/native-cli', [switch]$DiagnoseStartup)
$ErrorActionPreference = 'Stop'
$ProgressPreference = 'SilentlyContinue'
$utf8 = [Text.UTF8Encoding]::new($false)
$root = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $root) { throw 'Output directory must be new.' }
[IO.Directory]::CreateDirectory($root) | Out-Null
$summary = [ordered]@{ workflowCommit=$env:GITHUB_SHA; ciRun=$env:GITHUB_RUN_ID; completed=$false; processes=@(); errors=@(); scope='Actual unauthenticated CLI startup only; GUI, IME and long-run acceptance excluded.' }
function Write-Json($path, $value) { [IO.File]::WriteAllText($path, ($value | ConvertTo-Json -Depth 12) + "`n", $utf8) }
try {
  # portable-pty imports these registry environments after the parent environment.
  $sensitive = @()
  foreach ($key in @('HKLM:\SYSTEM\CurrentControlSet\Control\Session Manager\Environment','HKCU:\Environment')) {
    if (Test-Path -LiteralPath $key) {
      $sensitive += (Get-Item -LiteralPath $key).GetValueNames() | Where-Object { $_ -match '(?i)codex|openai|anthropic|api.?key|token|secret|credential|^home$|^userprofile$|^appdata$|^localappdata$|proxy' }
    }
  }
  $summary.registrySensitiveNames = @($sensitive | Sort-Object -Unique)
  if ($sensitive.Count) { throw 'Registry environment does not provide credential isolation.' }
  $cliFiles = @(Get-ChildItem -LiteralPath 'artifacts/cli-dependency/node_modules/@openai' -Recurse -File -Filter codex.exe)
  if ($cliFiles.Count -ne 1) { throw 'Expected exactly one native CLI dependency.' }
  $cli = $cliFiles[0].FullName
  $summary.cliSha256 = (Get-FileHash -LiteralPath $cli -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($summary.cliSha256 -cne '9d563ee4e0441ff9d59d612013e0460ddf8e5709735c2d9f6b98b7ef8ca4eb42') { throw 'CLI hash mismatch.' }
  if (Test-Path -LiteralPath (Join-Path $cliFiles[0].DirectoryName 'codex-app-server.exe')) { throw 'Unexpected sibling app-server.' }
  $exe = Join-Path $root 'arcvera-studio.exe'
  Invoke-WebRequest -Uri 'https://github.com/jaeseongs95/Arcvera-Studio/releases/download/v1.8.0/arcvera-studio.exe' -OutFile $exe
  $summary.applicationSha256 = (Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()
  if ($summary.applicationSha256 -cne '60ef6aeda944b6ba65baa14c5124c4d04abe2e25df7a1e2a087ce232534aeb49' -or (Get-Item -LiteralPath $exe).Length -ne 16093184) { throw 'Application identity mismatch.' }
  $fresh = Join-Path $root 'fresh-profile'
  foreach ($name in @('','codex','roaming','local','temp')) { [IO.Directory]::CreateDirectory((Join-Path $fresh $name)) | Out-Null }
  $isolated = @{ SystemRoot=$env:SystemRoot; WINDIR=$env:WINDIR; PATH=(Join-Path $env:SystemRoot 'System32'); HOME=$fresh; USERPROFILE=$fresh; APPDATA=(Join-Path $fresh 'roaming'); LOCALAPPDATA=(Join-Path $fresh 'local'); TEMP=(Join-Path $fresh 'temp'); TMP=(Join-Path $fresh 'temp'); CODEX_HOME=(Join-Path $fresh 'codex'); ARCVERA_STUDIO_CODEX=$cli }
  function Run-Owned($file, [string[]]$arguments, [string]$label, [int]$seconds) {
    $info = [Diagnostics.ProcessStartInfo]::new($file)
    $info.UseShellExecute=$false; $info.CreateNoWindow=$true
    $info.RedirectStandardOutput=$true; $info.RedirectStandardError=$true; $info.WorkingDirectory=$root
    $info.Environment.Clear()
    foreach ($item in $isolated.GetEnumerator()) { $info.Environment[$item.Key]=$item.Value }
    foreach ($argument in $arguments) { $info.ArgumentList.Add($argument) }
    $process=[Diagnostics.Process]::new(); $process.StartInfo=$info
    $record=[ordered]@{label=$label; arguments=$arguments; processId=$null; createdAt=$null; exitCode=$null; deadlineCleanup=$false; exitConfirmed=$false}
    $summary.processes += $record
    try {
      if (!$process.Start()) { throw 'Process did not start.' }
      $record.processId=$process.Id; $record.createdAt=$process.StartTime.ToUniversalTime().ToString('o')
      $stdout=$process.StandardOutput.ReadToEndAsync(); $stderr=$process.StandardError.ReadToEndAsync()
      if (!$process.WaitForExit($seconds*1000)) { $record.deadlineCleanup=$true; $process.Kill($true); if (!$process.WaitForExit(10000)) { throw 'Owned process termination not confirmed.' } }
      $record.exitConfirmed=$process.HasExited; $record.exitCode=$process.ExitCode
      if (![Threading.Tasks.Task]::WaitAll([Threading.Tasks.Task[]]@($stdout,$stderr),10000)) { throw "$label output did not close after process exit." }
      [IO.File]::WriteAllText((Join-Path $root "$label.stdout.txt"),$stdout.GetAwaiter().GetResult(),$utf8)
      [IO.File]::WriteAllText((Join-Path $root "$label.stderr.txt"),$stderr.GetAwaiter().GetResult(),$utf8)
      if ($record.deadlineCleanup) { throw "$label exceeded its deadline." }
      if ($process.ExitCode -ne 0) { throw "$label returned exit $($process.ExitCode)." }
      return [IO.File]::ReadAllText((Join-Path $root "$label.stdout.txt"))
    } finally {
      if ($null -ne $record.processId -and !$process.HasExited) { $record.deadlineCleanup=$true; $process.Kill($true); $record.exitConfirmed=$process.WaitForExit(10000) }
      $process.Dispose()
    }
  }
  $null=Run-Owned $cli @('--version') 'cli-version' 20
  $null=Run-Owned $cli @('app-server','--help') 'server-help' 20
  $help=Run-Owned $cli @('app-server','daemon','start','--help') 'daemon-help' 20
  if ($help -match '--existing-installation|--first-installation') { throw 'Unexpected shared-daemon capability; startup not authorized by this case.' }
  $reportPath=Join-Path $root 'smoke.json'
  $smokeArguments=@('--codex-smoke','--sessions','1','--report',$reportPath)
  if ($DiagnoseStartup) { $smokeArguments += '--diagnostic-terminal' }
  try { $null=Run-Owned $exe $smokeArguments 'smoke' 150 }
  finally {
    if ($DiagnoseStartup -and (Test-Path -LiteralPath $reportPath)) {
      # Inspect only this invocation's fresh diagnostic. Never upload terminal text.
      $partial=Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
      $expectedDirectory=Join-Path $root ('agent-grid-codex-smoke-*')
      $directory=[IO.Path]::GetFullPath($partial.testDirectory)
      if ((Split-Path -Parent $directory) -ne $root -or $directory -notlike $expectedDirectory) { throw 'Diagnostic directory is outside this invocation.' }
      $signals=@()
      foreach ($session in $partial.sessions) {
        $sessionId=[Guid]::Parse($session.sessionId).ToString()
        $diagnostic=Join-Path $directory ('startup-terminal-'+$sessionId+'.json')
        if (!(Test-Path -LiteralPath $diagnostic)) { continue }
        $content=Get-Content -LiteralPath $diagnostic -Raw | ConvertFrom-Json
        if ($content.sessionId -ne $sessionId) { throw 'Diagnostic session identity mismatch.' }
        $prefix=[string]$content.redactedStartupPrefix
        $signals += [ordered]@{sessionId=$sessionId; diagnosticSha256=(Get-FileHash -LiteralPath $diagnostic -Algorithm SHA256).Hash.ToLowerInvariant(); authenticationPrompt=($prefix -match '(?i)sign in|log in|login|API key|authenticate'); trustPrompt=($prefix -match '(?i)trust this|trust the|trust.*folder|trust.*directory'); welcome=($prefix -match '(?i)welcome to'); terminalError=($prefix -match '(?i)terminal.*(error|unsupported)|cursor position.*could not'); errorWord=($prefix -match '(?i)\berror\b'); prefixCharacters=$prefix.Length}
      }
      Write-Json (Join-Path $root 'startup-signals.json') $signals
    }
  }
  $report=Get-Content -LiteralPath $reportPath -Raw | ConvertFrom-Json
  if ($report.schema -cne 'agent-grid-codex-smoke-v1' -or !$report.pass -or $report.startupMode -cne 'isolated-fresh-cwd-startup' -or $report.errors.Count -ne 0 -or $report.sessions.Count -ne 1) { throw 'Smoke report did not satisfy NC04.' }
  $session=$report.sessions[0]; $thread=[Guid]::Empty
  if (![Guid]::TryParse($session.threadId,[ref]$thread) -or $session.capturedPtyBytes -le 0 -or $session.observedState -notin @('idle','running','waiting') -or $null -ne $session.error) { throw 'Actual thread/PTY binding missing.' }
  if ($report.memory.Count -lt 3) { throw 'Memory samples missing.' }
  foreach ($sample in $report.memory) {
    foreach ($field in @('workingSetBytes','privateBytes','handles','ownedProcesses')) { if ($null -eq $sample.$field) { throw "Missing memory measurement: $field" } }
  }
  if ($report.memory[-1].ownedProcesses -ne 1) { throw 'Final owned-process baseline not restored.' }
  $summary.reportSha256=(Get-FileHash -LiteralPath $reportPath -Algorithm SHA256).Hash.ToLowerInvariant()
  $summary.completed=$true
} catch { $summary.errors += $_.Exception.Message; throw }
finally { Write-Json (Join-Path $root 'observation.json') $summary }
