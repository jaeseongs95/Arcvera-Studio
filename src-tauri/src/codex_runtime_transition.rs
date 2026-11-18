//! 전환 준비와 응답 해석만 담당한다. 실행·설치·재시도·세션 종료는 수행하지 않는다.
use super::*;
use std::{fs, path::PathBuf};

const MANIFEST_LIMIT: u64 = 16 * 1024;
const EXECUTABLE_LIMIT: u64 = 1024 * 1024 * 1024;

// Explicit own-fixture diagnostics only. No domain values enter trace records.
#[derive(Clone, Copy, Default)]
struct RuntimePreviewTrace { enabled: bool }
#[derive(Clone, Copy)]
enum RuntimeTracePhase { PairProbe, UpdateHelp, PackageProbe, FileHash, FileRead, FileSha256, PreviewTotal }
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum RuntimeTraceOutcome { Ok, Err, Blocked, NotRun }
struct RuntimeTraceEvent { phase: RuntimeTracePhase, elapsed: Duration, outcome: RuntimeTraceOutcome, bytes: Option<u64> }

impl RuntimeTraceEvent {
    fn write_to(&self, writer: &mut impl io::Write) -> io::Result<()> {
        let phase = match self.phase {
            RuntimeTracePhase::PairProbe => "pair_probe", RuntimeTracePhase::UpdateHelp => "update_help",
            RuntimeTracePhase::PackageProbe => "package_probe", RuntimeTracePhase::FileHash => "file_hash",
            RuntimeTracePhase::FileRead => "file_read", RuntimeTracePhase::FileSha256 => "file_sha256",
            RuntimeTracePhase::PreviewTotal => "preview_total",
        };
        let outcome = match self.outcome {
            RuntimeTraceOutcome::Ok => "ok", RuntimeTraceOutcome::Err => "err",
            RuntimeTraceOutcome::Blocked => "blocked", RuntimeTraceOutcome::NotRun => "not_run",
        };
        match self.bytes {
            Some(bytes) => writeln!(writer, "runtime_preview_trace phase={phase} elapsed_ms={} outcome={outcome} bytes={bytes}", self.elapsed.as_millis()),
            None => writeln!(writer, "runtime_preview_trace phase={phase} elapsed_ms={} outcome={outcome}", self.elapsed.as_millis()),
        }
    }
    fn emit(self) {
        // A failed diagnostic sink must not replace the original Result.
        let _ = self.write_to(&mut io::stderr().lock());
    }
}
impl RuntimePreviewTrace {
    fn from_setting(setting: Option<&std::ffi::OsStr>) -> Self {
        Self { enabled: setting == Some(std::ffi::OsStr::new("1")) }
    }
    fn from_environment() -> Self { Self::from_setting(env::var_os("ARCVERA_STUDIO_RUNTIME_TRACE").as_deref()) }
    fn start(self) -> Option<Instant> { self.enabled.then(Instant::now) }
    fn measure<T>(self, phase: RuntimeTracePhase, work: impl FnOnce() -> Result<T, String>) -> Result<T, String> {
        self.measure_with(phase, work, RuntimeTraceEvent::emit)
    }
    fn measure_with<T>(self, phase: RuntimeTracePhase, work: impl FnOnce() -> Result<T, String>,
        emit: impl FnOnce(RuntimeTraceEvent)) -> Result<T, String> {
        let started = self.start();
        let result = work();
        if let Some(started) = started {
            emit(RuntimeTraceEvent { phase, elapsed: started.elapsed(),
                outcome: if result.is_ok() { RuntimeTraceOutcome::Ok } else { RuntimeTraceOutcome::Err }, bytes: None });
        }
        result
    }
}

fn runtime_preview_outcome(result: &Result<RuntimeTransitionPreview, String>) -> RuntimeTraceOutcome {
    match result {
        Ok(report) if report.can_apply => RuntimeTraceOutcome::Ok,
        Ok(_) => RuntimeTraceOutcome::Blocked,
        Err(_) => RuntimeTraceOutcome::Err,
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTransitionPreview {
    pub pair: RuntimePairStatus,
    pub can_apply: bool,
    pub reason: Option<String>,
    pub fingerprint: String,
    pub package_root: Option<String>,
    pub management_executable: Option<String>,
    #[serde(skip)]
    target_identity: Option<ReplacementIdentity>,
}

#[derive(Debug)]
struct ReplacementIdentity { kind: String, cli_name: String, cli: Vec<u8>, server: Vec<u8>, helper: Option<Vec<u8>> }

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RuntimeTransitionReply {
    pub status: String,
    pub success: bool,
    pub outcome: String,
    pub message: Option<String>,
}

pub(crate) fn command_args() -> Vec<String> {
    ["app-server", "daemon", "update", "--from-cli", "--no-force", "-y"].map(str::to_owned).to_vec()
}

pub(crate) fn replacement_command_args() -> Vec<String> {
    ["app-server", "daemon", "update", "--from-cli", "-y"].map(str::to_owned).to_vec()
}

fn checkpoint(cancelled: &Arc<AtomicBool>, deadline: Instant) -> Result<(), String> {
    check_cancelled(Some(cancelled))?;
    if Instant::now() >= deadline { return Err("서버 전환 확인 시간이 초과되었습니다.".into()); }
    Ok(())
}

fn bind(hash: &mut Sha256, bytes: &[u8]) {
    hash.update((bytes.len() as u64).to_le_bytes());
    hash.update(bytes);
}

// 디렉터리 열거 없이 manifest와 두 실행 파일만 읽는다. OS의 동기 open/read 자체를
// 중단하는 보장은 없으며, 각 read 전후에 전체 deadline과 취소를 확인한다.
fn file_hash(path: &Path, cancelled: &Arc<AtomicBool>, deadline: Instant) -> Result<Vec<u8>, String> {
    file_hash_traced(path, cancelled, deadline, RuntimePreviewTrace::default())
}

fn file_hash_traced(path: &Path, cancelled: &Arc<AtomicBool>, deadline: Instant, trace: RuntimePreviewTrace) -> Result<Vec<u8>, String> {
    file_hash_observed(path, cancelled, deadline, trace, |events| { for event in events { event.emit(); } })
}

fn file_hash_observed(path: &Path, cancelled: &Arc<AtomicBool>, deadline: Instant, trace: RuntimePreviewTrace,
    emit: impl FnOnce([RuntimeTraceEvent; 3])) -> Result<Vec<u8>, String> {
    let started = trace.start();
    let (mut read_elapsed, mut sha_elapsed) = (Duration::ZERO, Duration::ZERO);
    let (mut read_attempted, mut sha_attempted) = (false, false);
    let (mut read_bytes, mut hashed_bytes) = (0u64, 0u64);
    let result = (|| {
        checkpoint(cancelled, deadline)?;
        let mut file = fs::File::open(path).map_err(|error| format!("실행 파일을 확인할 수 없습니다: {error}"))?;
        let before = file.metadata().map_err(|error| format!("실행 파일 정보를 확인할 수 없습니다: {error}"))?;
        if !before.is_file() || before.len() == 0 || before.len() > EXECUTABLE_LIMIT {
            return Err("실행 파일은 크기가 1 GiB 이하인 일반 파일이어야 합니다.".into());
        }
        let mut hash = Sha256::new();
        let mut count = 0u64;
        let mut block = [0u8; 64 * 1024];
        loop {
            checkpoint(cancelled, deadline)?;
            let read_started = trace.start();
            let read = file.read(&mut block);
            if let Some(started) = read_started {
                read_attempted = true; read_elapsed += started.elapsed();
                if let Ok(count) = &read { read_bytes += *count as u64; }
            }
            let read = read.map_err(|error| format!("실행 파일 해시 조회 실패: {error}"))?;
            checkpoint(cancelled, deadline)?;
            if read == 0 { break; }
            count += read as u64;
            if count > EXECUTABLE_LIMIT { return Err("확인 중 실행 파일 크기가 1 GiB를 초과했습니다.".into()); }
            let sha_started = trace.start();
            hash.update(&block[..read]);
            if let Some(started) = sha_started {
                sha_attempted = true; sha_elapsed += started.elapsed(); hashed_bytes += read as u64;
            }
        }
        let after = file.metadata().map_err(|error| format!("실행 파일 정보를 재확인할 수 없습니다: {error}"))?;
        if count != before.len() || after.len() != before.len() || after.modified().ok() != before.modified().ok() {
            return Err("확인 중 실행 파일이 바뀌었습니다. 다시 확인하세요.".into());
        }
        let sha_started = trace.start();
        let digest = hash.finalize().to_vec();
        if let Some(started) = sha_started { sha_attempted = true; sha_elapsed += started.elapsed(); }
        Ok(digest)
    })();
    if let Some(started) = started {
        let outcome = if result.is_ok() { RuntimeTraceOutcome::Ok } else { RuntimeTraceOutcome::Err };
        // ceiling: three completion records per file, never per read/chunk.
        emit([
            RuntimeTraceEvent { phase: RuntimeTracePhase::FileHash, elapsed: started.elapsed(), outcome, bytes: Some(hashed_bytes) },
            RuntimeTraceEvent { phase: RuntimeTracePhase::FileRead, elapsed: read_elapsed,
                outcome: if read_attempted { outcome } else { RuntimeTraceOutcome::NotRun }, bytes: Some(read_bytes) },
            RuntimeTraceEvent { phase: RuntimeTracePhase::FileSha256, elapsed: sha_elapsed,
                outcome: if sha_attempted { outcome } else { RuntimeTraceOutcome::NotRun }, bytes: Some(hashed_bytes) },
        ]);
    }
    result
}

struct PackageEvidence {
    root: PathBuf, cli: PathBuf, server: PathBuf,
    manifest: Vec<u8>, cli_hash: Vec<u8>, server_hash: Vec<u8>,
    runtime_helper_hash: Option<Vec<u8>>,
}

fn manifest_matches(bytes: &[u8], version: &str) -> Result<&'static str, String> {
    if bytes.len() as u64 > MANIFEST_LIMIT { return Err("패키지 정보 파일이 허용 크기를 넘었습니다. 새 전체 패키지를 선택하세요.".into()); }
    let value: Value = serde_json::from_slice(bytes).map_err(|_| "패키지 정보 파일을 읽을 수 없습니다. 새 전체 패키지를 선택하세요.")?;
    let target = match env::consts::ARCH {
        "x86_64" => "x86_64-pc-windows-msvc",
        "aarch64" => "aarch64-pc-windows-msvc",
        _ => return Err("이 Windows 아키텍처의 패키지 전환을 지원하지 않습니다.".into()),
    };
    // These exact relative entrypoints are the current public package and the
    // legacy layout. Never interpret an arbitrary manifest path as executable.
    let entrypoint = match value["entrypoint"].as_str() {
        Some("bin/arcvera.exe") => "bin/arcvera.exe",
        Some("bin/codex.exe") => "bin/codex.exe",
        _ => return Err("패키지 CLI 진입점이 지원되는 bin/arcvera.exe 또는 bin/codex.exe가 아닙니다.".into()),
    };
    if value["layoutVersion"].as_u64() != Some(1)
        || value["version"].as_str() != Some(version) || value["target"] != target {
        return Err("패키지 구성·전체 버전·Windows 실행 대상이 선택한 CLI와 일치하지 않습니다. 새 전체 패키지를 선택하세요.".into());
    }
    Ok(entrypoint)
}

fn package_evidence(exe: &OsString, server: &OsString, kind: &str, version: &str,
    cancelled: &Arc<AtomicBool>, deadline: Instant) -> Result<PackageEvidence, String> {
    package_evidence_traced(exe, server, kind, version, cancelled, deadline, RuntimePreviewTrace::default())
}

fn package_evidence_traced(exe: &OsString, server: &OsString, kind: &str, version: &str,
    cancelled: &Arc<AtomicBool>, deadline: Instant, trace: RuntimePreviewTrace) -> Result<PackageEvidence, String> {
    checkpoint(cancelled, deadline)?;
    let cli = Path::new(exe).canonicalize().map_err(|_| "선택한 CLI의 실제 경로를 확인할 수 없습니다.")?;
    let server = Path::new(server).canonicalize().map_err(|_| "선택한 앱서버의 실제 경로를 확인할 수 없습니다.")?;
    let bin = cli.parent().ok_or("CLI의 bin 폴더가 없습니다.")?;
    let named = |path: &Path, name: &str| path.file_name().and_then(|value| value.to_str()).is_some_and(|value| value.eq_ignore_ascii_case(name));
    if !(named(&cli, "arcvera.exe") || named(&cli, "codex.exe")) || !named(bin, "bin") || match kind {
        "standalone" => server.parent() != Some(bin) || !named(&server, "codex-app-server.exe"),
        "cli" => server != cli,
        _ => true,
    } {
        return Err("공유 서버를 교체하려면 전체 ZIP의 manifest가 지정한 bin/arcvera.exe 또는 bin/codex.exe를 CLI로, 같은 bin/codex-app-server.exe를 서버로 선택하세요. 실행 파일만 있는 배포본은 CLI의 공유 서버 교체 명령으로 적용할 수 없습니다. 기존 공유 서버는 종료하지 않았습니다.".into());
    }
    let root = bin.parent().ok_or("CLI 패키지 폴더가 없습니다.")?.to_path_buf();
    let manifest_path = root.join("codex-package.json").canonicalize()
        .map_err(|_| "필수 정보 파일이 있는 전체 패키지를 선택하세요. 개별 실행 파일은 자동 설치하지 않습니다.")?;
    if manifest_path.parent() != Some(root.as_path()) { return Err("패키지 정보 파일이 패키지 밖을 가리킵니다.".into()); }
    checkpoint(cancelled, deadline)?;
    let file = fs::File::open(manifest_path).map_err(|error| format!("패키지 정보 파일 조회 실패: {error}"))?;
    let metadata = file.metadata().map_err(|error| format!("패키지 정보 조회 실패: {error}"))?;
    if !metadata.is_file() || metadata.len() > MANIFEST_LIMIT { return Err("패키지 정보 파일 형식이나 크기를 확인할 수 없습니다. 새 전체 패키지를 선택하세요.".into()); }
    let mut manifest = Vec::new();
    file.take(MANIFEST_LIMIT + 1).read_to_end(&mut manifest).map_err(|error| format!("패키지 정보 파일 읽기 실패: {error}"))?;
    checkpoint(cancelled, deadline)?;
    let entrypoint = manifest_matches(&manifest, version)?;
    let entrypoint_name = Path::new(entrypoint).file_name().and_then(|name| name.to_str())
        .ok_or("패키지 CLI 진입점 이름을 확인할 수 없습니다.")?;
    if !named(&cli, entrypoint_name)
        || root.join(entrypoint).canonicalize().map_err(|_| "패키지 CLI 진입점의 실제 경로를 확인할 수 없습니다.")? != cli {
        return Err("선택한 CLI가 패키지 manifest의 CLI 진입점과 다릅니다.".into());
    }
    let cli_hash = file_hash_traced(&cli, cancelled, deadline, trace)?;
    let server_hash = if server == cli { cli_hash.clone() } else { file_hash_traced(&server, cancelled, deadline, trace)? };
    checkpoint(cancelled, deadline)?;
    Ok(PackageEvidence { root, cli, server, manifest, cli_hash, server_hash, runtime_helper_hash: None })
}

// A runtime-only ZIP can reuse the already installed complete package only
// when every supplied executable is byte-identical. This is not a version-
// based fallback and never assembles a package from missing/fabricated helpers.
fn replacement_package_evidence(exe: &OsString, server: &OsString, kind: &str, version: &str,
    managed_cli: Option<&str>, cancelled: &Arc<AtomicBool>, deadline: Instant,
) -> Result<PackageEvidence, String> {
    replacement_package_evidence_traced(exe, server, kind, version, managed_cli, cancelled, deadline, RuntimePreviewTrace::default())
}

fn replacement_package_evidence_traced(exe: &OsString, server: &OsString, kind: &str, version: &str,
    managed_cli: Option<&str>, cancelled: &Arc<AtomicBool>, deadline: Instant, trace: RuntimePreviewTrace,
) -> Result<PackageEvidence, String> {
    match package_evidence_traced(exe, server, kind, version, cancelled, deadline, trace) {
        Ok(evidence) => Ok(evidence),
        Err(original) => {
            let Some(managed_cli) = managed_cli else { return Err(original); };
            let selected_cli = Path::new(exe).canonicalize().map_err(|_| original.clone())?;
            let selected_server = Path::new(server).canonicalize().map_err(|_| original.clone())?;
            let Some(selected_bin) = selected_cli.parent() else { return Err(original); };
            if kind != "standalone" || selected_server != selected_bin.join("codex-app-server.exe")
                || selected_bin.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case("bin"))
                || selected_bin.join("codex-package.json").try_exists().map_err(|_| original.clone())? { return Err(original); }
            let installed_cli = Path::new(managed_cli).canonicalize().map_err(|_| original.clone())?;
            let Some(installed_bin) = installed_cli.parent() else { return Err(original); };
            let installed_server = installed_bin.join("codex-app-server.exe");
            let mut evidence = package_evidence_traced(&installed_cli.as_os_str().to_owned(), &installed_server.as_os_str().to_owned(), kind, version, cancelled, deadline, trace)
                .map_err(|_| original.clone())?;
            // The validated installed manifest chooses the CLI name. Identical
            // bytes under another name cannot substitute for that entrypoint.
            let cli_name = evidence.cli.file_name().and_then(|name| name.to_str()).ok_or_else(|| original.clone())?;
            if !selected_cli.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case(cli_name))
                || selected_bin.join(cli_name).canonicalize().map_err(|_| original.clone())? != selected_cli {
                return Err(original);
            }
            for (selected, installed, is_helper) in [
                (selected_cli.clone(), evidence.cli.clone(), false),
                (selected_server.clone(), evidence.server.clone(), false),
                (selected_bin.join("codex-code-mode-host.exe"), installed_bin.join("codex-code-mode-host.exe"), true),
            ] {
                let selected_hash = file_hash_traced(&selected, cancelled, deadline, trace)?;
                if selected_hash != file_hash_traced(&installed, cancelled, deadline, trace)? {
                    return Err("선택한 실행 파일과 설치된 전체 패키지의 파일이 다릅니다. 해당 버전의 전체 ZIP에서 CLI와 서버를 선택하세요. 기존 서버는 종료하지 않았습니다.".into());
                }
                if is_helper { evidence.runtime_helper_hash = Some(selected_hash); }
            }
            Ok(evidence)
        }
    }
}

fn supports_update(help: &[u8], replace_existing: bool) -> Result<bool, String> {
    let from_cli = help_supports_option(help, "--from-cli")?;
    let no_force = help_supports_option(help, "--no-force")?;
    let text = std::str::from_utf8(help).map_err(|_| "전환 도움말이 올바른 UTF-8이 아닙니다.")?;
    let options = text.lines().skip_while(|line| !matches!(line.trim(), "Options:" | "옵션:")).skip(1)
        .take_while(|line| line.is_empty() || line.starts_with(char::is_whitespace));
    let short_column = options.clone().filter(|line| line.trim_start().starts_with('-'))
        .map(|line| line.len() - line.trim_start().len()).min();
    let short_yes = options.clone().any(|line| Some(line.len() - line.trim_start().len()) == short_column
        && matches!(line.split_ascii_whitespace().next(), Some("-y" | "-y,")));
    // 실제 실행 인자가 -y로 고정되어 있으므로 --yes만 선언한 CLI는 통과시키지 않는다.
    Ok(from_cli && (replace_existing || no_force) && short_yes)
}

fn supports_safe_update(help: &[u8]) -> Result<bool, String> { supports_update(help, false) }

// 현재 UI가 사용하는 preview 경계의 주입점이다. 시험은 실제 daemon/실행 파일을
// 호출하지 않고 동일 명령·deadline과 패키지 증거의 결속을 검증한다.
fn preview_commands(exe: &OsString, server: &OsString, kind: &str, pair: RuntimePairStatus,
    cancelled: &Arc<AtomicBool>, deadline: Instant,
    package: impl FnOnce() -> Result<PackageEvidence, String>,
    query: impl FnMut(&[&str], Instant) -> Result<Vec<u8>, String>,
) -> Result<RuntimeTransitionPreview, String> {
    preview_commands_mode(exe, server, kind, pair, cancelled, deadline, false, package, query)
}

fn preview_commands_mode(exe: &OsString, server: &OsString, kind: &str, pair: RuntimePairStatus,
    cancelled: &Arc<AtomicBool>, deadline: Instant, replace_existing: bool,
    package: impl FnOnce() -> Result<PackageEvidence, String>,
    mut query: impl FnMut(&[&str], Instant) -> Result<Vec<u8>, String>,
) -> Result<RuntimeTransitionPreview, String> {
    checkpoint(cancelled, deadline)?;
    let mut hash = Sha256::new();
    bind(&mut hash, b"opendiy-runtime-transition-v1");
    bind(&mut hash, &[u8::from(replace_existing)]);
    bind(&mut hash, exe.to_string_lossy().as_bytes());
    bind(&mut hash, server.to_string_lossy().as_bytes());
    bind(&mut hash, kind.as_bytes());
    bind(&mut hash, &serde_json::to_vec(&pair).map_err(|_| "전환 버전 상태를 결속할 수 없습니다.")?);
    let mut report = RuntimeTransitionPreview { pair, can_apply: false, reason: None, fingerprint: String::new(), package_root: None, management_executable: None, target_identity: None };
    let candidate = (|| -> Result<(), String> {
        if report.pair.cli_version != report.pair.selected_server_version {
            return Err("선택한 CLI와 앱서버의 전체 버전을 먼저 맞추세요.".into());
        }
        if report.pair.state == "matched" && !replace_existing {
            return Err("선택한 CLI와 관리·실행 서버의 버전이 이미 같습니다. 전환하지 않았습니다.".into());
        }
        if report.pair.state == "stopped" && report.pair.managed_version.is_none()
            && report.pair.running_version.is_none() && report.pair.managed_cli_executable.is_some()
            && report.pair.socket_path.is_some() {
            return Err("공유 서버 패키지가 미설치 상태입니다. 현재 CLI의 안전한 서버 전환 명령은 첫 설치를 지원하지 않으므로 선택한 패키지의 별도 준비가 필요합니다.".into());
        }
        if !(matches!(report.pair.state.as_str(), "mismatch" | "stopped") || replace_existing && report.pair.state == "matched")
            || report.pair.managed_cli_executable.is_none() || report.pair.managed_version.is_none()
            || (matches!(report.pair.state.as_str(), "mismatch" | "matched") && report.pair.running_version.is_none())
            || (report.pair.state == "stopped" && report.pair.running_version.is_some()) || report.pair.socket_path.is_none() {
            return Err("현재 관리·실행 서버의 상태를 확인하지 못해 전환을 준비할 수 없습니다.".into());
        }
        // Check this cheap prerequisite before reading/hashing large executables.
        // Unsupported CLIs must report the missing capability, not a hash timeout.
        let help = query(&["app-server", "daemon", "update", "--help"], deadline)?;
        checkpoint(cancelled, deadline)?;
        bind(&mut hash, &help);
        if !supports_update(&help, replace_existing)? {
            return Err(if replace_existing { "선택한 CLI는 해당 패키지로 공유 서버를 교체하는 명령을 지원하지 않습니다." }
                else { "선택한 CLI는 작업을 강제로 종료하지 않는 서버 전환을 지원하지 않습니다. 새 전체 패키지를 선택하세요." }.into());
        }
        let evidence = package()?;
        checkpoint(cancelled, deadline)?;
        for path in [&evidence.root, &evidence.cli, &evidence.server] {
            bind(&mut hash, path.to_str().ok_or("패키지 경로가 올바른 문자열이 아닙니다.")?.as_bytes());
        }
        bind(&mut hash, &evidence.manifest);
        bind(&mut hash, &evidence.cli_hash);
        bind(&mut hash, &evidence.server_hash);
        if let Some(helper) = &evidence.runtime_helper_hash { bind(&mut hash, helper); }
        report.package_root = Some(evidence.root.to_str().ok_or("패키지 폴더가 올바른 문자열이 아닙니다.")?.to_owned());
        report.management_executable = Some(evidence.cli.to_str().ok_or("관리 CLI 경로가 올바른 문자열이 아닙니다.")?.to_owned());
        let cli_name = evidence.cli.file_name().and_then(|name| name.to_str())
            .ok_or("패키지 CLI 파일 이름을 확인할 수 없습니다.")?.to_owned();
        report.target_identity = Some(ReplacementIdentity { kind: kind.to_owned(), cli_name, cli: evidence.cli_hash, server: evidence.server_hash, helper: evidence.runtime_helper_hash });
        report.can_apply = true;
        // 이 preview는 전체 패키지 무결성을 인증하지 않는다. 트리 검증은 CLI 책임이다.
        report.reason = Some(if replace_existing {
            "선택한 패키지로 기존 공유 서버를 교체합니다. 종료 유예 후 남은 서버 프로세스는 CLI가 종료하며 기존 연결은 끊어집니다."
        } else {
            "확인 후 작업을 강제로 종료하지 않고 서버를 전환합니다. 종료 요청 후에는 취소할 수 없으며 응답이 끊겨도 서버가 나중에 종료될 수 있습니다."
        }.into());
        Ok(())
    })();
    if let Err(reason) = candidate { report.reason = Some(reason); }
    checkpoint(cancelled, deadline)?;
    report.fingerprint = format!("{:x}", hash.finalize());
    Ok(report)
}

// The CLI owns shutdown/start. Reconcile its selected package against the
// pre-update bytes; a matching version alone cannot identify a build.
pub(crate) fn verify_replacement_target(expected: &RuntimeTransitionPreview, pair: &RuntimePairStatus,
    cancelled: &Arc<AtomicBool>,
) -> Result<(), String> {
    let identity = expected.target_identity.as_ref().ok_or("교체 대상 파일 증거가 없습니다.")?;
    if pair.state != "matched" || pair.cli_version != expected.pair.cli_version
        || pair.selected_server_version != expected.pair.selected_server_version {
        return Err("교체 후 서버 버전을 확인하지 못했습니다.".into());
    }
    let managed = pair.managed_cli_executable.as_ref().ok_or("교체 후 관리 패키지 경로가 없습니다.")?;
    let cli = Path::new(managed);
    let server = if identity.kind == "cli" { cli.to_path_buf() } else { cli.with_file_name("codex-app-server.exe") };
    let deadline = Instant::now() + Duration::from_secs(20);
    let actual = package_evidence(&cli.as_os_str().to_owned(), &server.into_os_string(), &identity.kind,
        &expected.pair.cli_version, cancelled, deadline)?;
    if !actual.cli.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.eq_ignore_ascii_case(&identity.cli_name)) {
        return Err("교체 후 관리 패키지의 CLI 진입점 이름이 선택한 패키지와 다릅니다.".into());
    }
    if actual.cli_hash != identity.cli || actual.server_hash != identity.server {
        return Err("교체 후 관리 패키지의 실행 파일이 선택한 파일과 다릅니다.".into());
    }
    if let Some(helper) = &identity.helper {
        if file_hash(&actual.cli.with_file_name("codex-code-mode-host.exe"), cancelled, deadline)? != *helper {
            return Err("교체 후 관리 패키지의 코드 실행 파일이 선택한 파일과 다릅니다.".into());
        }
    }
    Ok(())
}

pub(crate) fn preview(exe: &OsString, server: &OsString, kind: &str, cancelled: &Arc<AtomicBool>) -> Result<RuntimeTransitionPreview, String> {
    preview_mode(exe, server, kind, cancelled, false)
}

pub(crate) fn preview_replacement(exe: &OsString, server: &OsString, kind: &str, cancelled: &Arc<AtomicBool>) -> Result<RuntimeTransitionPreview, String> {
    preview_mode(exe, server, kind, cancelled, true)
}

fn preview_mode(exe: &OsString, server: &OsString, kind: &str, cancelled: &Arc<AtomicBool>, replace_existing: bool) -> Result<RuntimeTransitionPreview, String> {
    let trace = RuntimePreviewTrace::from_environment();
    let started = trace.start();
    let result = (|| {
        let deadline = Instant::now() + Duration::from_secs(20);
        let pair = trace.measure(RuntimeTracePhase::PairProbe,
            || runtime_pair::inspect_until(exe, server, kind, cancelled, deadline))?;
        checkpoint(cancelled, deadline)?;
        let version = pair.cli_version.clone();
        let managed_cli = pair.managed_cli_executable.clone();
        let cwd = env::current_dir().map_err(|error| format!("전환 확인 폴더 조회 실패: {error}"))?;
        preview_commands_mode(exe, server, kind, pair, cancelled, deadline, replace_existing,
            || trace.measure(RuntimeTracePhase::PackageProbe, || if replace_existing {
                replacement_package_evidence_traced(exe, server, kind, &version, managed_cli.as_deref(), cancelled, deadline, trace)
            } else { package_evidence_traced(exe, server, kind, &version, cancelled, deadline, trace) }),
            |args, deadline| trace.measure(RuntimeTracePhase::UpdateHelp, || extensions::command_stdout_until(exe, &cwd,
                &args.iter().map(|value| (*value).to_owned()).collect::<Vec<_>>(), deadline, Some(cancelled))))
    })();
    if let Some(started) = started {
        RuntimeTraceEvent { phase: RuntimeTracePhase::PreviewTotal, elapsed: started.elapsed(),
            outcome: runtime_preview_outcome(&result), bytes: None }.emit();
    }
    result
}

pub(crate) fn parse_update_output(bytes: &[u8]) -> Result<RuntimeTransitionReply, String> {
    if bytes.len() > 64 * 1024 { return Err("서버 전환 응답이 크기 상한을 초과했습니다. 결과는 확인되지 않았으며 자동 재시도하지 마세요.".into()); }
    let value: Value = serde_json::from_slice(bytes)
        .map_err(|_| "서버 전환 응답이 올바른 JSON이 아닙니다. 결과는 확인되지 않았으며 자동 재시도하지 마세요.")?;
    let status = value.get("status").and_then(Value::as_str)
        .filter(|value| !value.is_empty() && value.len() <= 128 && !value.chars().any(char::is_control))
        .ok_or("서버 전환 상태가 누락되거나 올바르지 않습니다. 결과를 재확인하세요.")?.to_owned();
    let message = match value.get("message") {
        None | Some(Value::Null) => None,
        Some(Value::String(text)) if text.len() <= 4096 && !text.chars().any(char::is_control) => Some(text.clone()),
        _ => return Err("서버 전환 설명 형식이 올바르지 않습니다. 결과를 재확인하세요.".into()),
    };
    if status == "updated" {
        return Ok(RuntimeTransitionReply { status, success: true, outcome: "updated".into(), message });
    }
    // 요청/ACK 유실·timeout은 이전 서버 생존을 보장하지 않는다. 늦은 drain도 가능하다.
    let detail = match status.as_str() {
        "shutdownUnconfirmed" | "shutdownTimedOut" | "shutdownOutcomeUnknown" =>
            "종료 결과가 확인되지 않았습니다. 서버가 나중에 종료될 수 있으므로 상태를 확인하고 자동 재시도하지 마세요.",
        _ => "알 수 없는 서버 전환 응답입니다. 성공으로 처리하지 않았으며 자동 재시도하지 마세요.",
    };
    Ok(RuntimeTransitionReply { status, success: false, outcome: "unknown".into(),
        message: Some(match message { Some(message) if !message.is_empty() => format!("{detail} {message}"), _ => detail.into() }) })
}

#[cfg(test)]
mod tests {
    use super::*;
    const VERSION: &str = "0.166.0+native-r33.20261102";
    fn pair() -> RuntimePairStatus {
        RuntimePairStatus { cli_version: VERSION.into(), selected_server_executable: "server.exe".into(),
            selected_server_version: VERSION.into(), managed_cli_executable: Some("managed-cli.exe".into()),
            managed_version: Some("0.165.0+native-r28.20261029".into()), running_version: Some("0.165.0+native-r28.20261029".into()),
            socket_path: Some("fixture.sock".into()), state: "mismatch".into(), message: None }
    }
    fn help() -> Vec<u8> {
        b"Usage: codex app-server daemon update [OPTIONS]\n\nOptions:\n      --from-cli\n      --no-force\n  -y, --yes\n  -h, --help\n".to_vec()
    }
    fn help_without(line: &str) -> Vec<u8> {
        String::from_utf8(help()).unwrap().replace(line, "").into_bytes()
    }
    fn evidence() -> PackageEvidence {
        PackageEvidence { root: PathBuf::from("package"), cli: PathBuf::from("package/bin/codex.exe"),
            server: PathBuf::from("package/bin/codex-app-server.exe"), manifest: b"manifest-bytes".to_vec(), cli_hash: vec![1;32], server_hash: vec![2;32], runtime_helper_hash: None }
    }
    fn fixture(pair: RuntimePairStatus, evidence: PackageEvidence, help: Vec<u8>) -> RuntimeTransitionPreview {
        let deadline = Instant::now() + Duration::from_secs(20);
        let mut calls = 0;
        let report = preview_commands(&OsString::from("cli.exe"), &OsString::from("server.exe"), "standalone", pair,
            &Arc::new(AtomicBool::new(false)), deadline, || Ok(evidence), |args, received| {
                assert_eq!(args, ["app-server", "daemon", "update", "--help"]);
                assert_eq!(received, deadline);
                calls += 1;
                Ok(help.clone())
            }).unwrap();
        assert!(calls <= 1, "자동 retry 금지");
        report
    }

    #[test]
    fn command_plan_is_fixed_and_help_is_the_only_preview_command() {
        assert_eq!(command_args(), ["app-server", "daemon", "update", "--from-cli", "--no-force", "-y"]);
        let report = fixture(pair(), evidence(), help());
        assert!(report.can_apply);
        assert_eq!(report.fingerprint.len(), 64);
        assert!(report.reason.unwrap().contains("나중에 종료"));
    }

    #[test]
    fn replacement_uses_cli_owned_shutdown_without_the_no_force_policy() {
        assert_eq!(replacement_command_args(), ["app-server", "daemon", "update", "--from-cli", "-y"]);
        assert!(supports_update(&help_without("      --no-force\n"), true).unwrap());
        assert!(!supports_update(&help_without("      --from-cli\n"), true).unwrap());
        assert!(!supports_update(&help_without("  -y, --yes\n"), true).unwrap());
    }

    #[test]
    fn selecting_a_different_package_of_the_same_version_still_replaces_the_server() {
        let mut matched = pair(); matched.state = "matched".into();
        matched.managed_version = Some(VERSION.into()); matched.running_version = Some(VERSION.into());
        let report = preview_commands_mode(&OsString::from("cli"), &OsString::from("server"), "standalone", matched,
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20), true,
            || Ok(evidence()), |_, _| Ok(help())).unwrap();
        assert!(report.can_apply);
        assert!(report.reason.unwrap().contains("기존 공유 서버를 교체"));
    }

    #[test]
    fn replacement_fingerprint_cannot_authorize_a_non_forcing_transition() {
        let replacement = preview_commands_mode(&OsString::from("cli.exe"), &OsString::from("server.exe"), "standalone", pair(),
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20), true,
            || Ok(evidence()), |_, _| Ok(help())).unwrap();
        let ordinary = fixture(pair(), evidence(), help());
        assert!(replacement.can_apply && ordinary.can_apply);
        assert_ne!(replacement.fingerprint, ordinary.fingerprint);
    }

    #[test]
    fn bare_server_replacement_is_refused_before_a_shutdown_can_be_requested() {
        let report = preview_commands_mode(&OsString::from("cli"), &OsString::from("server"), "standalone", pair(),
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20), true,
            || Err("전체 ZIP의 bin 폴더에서 서버를 선택하세요".into()),
            |args, _| { assert_eq!(args, ["app-server", "daemon", "update", "--help"]); Ok(help()) }).unwrap();
        assert!(!report.can_apply);
        assert_eq!(report.reason.as_deref(), Some("전체 ZIP의 bin 폴더에서 서버를 선택하세요"));
    }

    fn runtime_only_fixture() -> (PathBuf, OsString, OsString, String) {
        let root = env::temp_dir().join(format!("desktop-replacement-{}", uuid::Uuid::new_v4()));
        let raw = root.join("raw"); let installed = root.join("installed");
        fs::create_dir_all(&raw).unwrap(); fs::create_dir_all(installed.join("bin")).unwrap();
        let target = format!("{}-pc-windows-msvc", env::consts::ARCH);
        fs::write(installed.join("codex-package.json"), serde_json::to_vec(&json!({
            "layoutVersion": 1, "entrypoint": "bin/codex.exe", "version": VERSION, "target": target,
        })).unwrap()).unwrap();
        for name in ["codex.exe", "codex-app-server.exe", "codex-code-mode-host.exe"] {
            fs::write(raw.join(name), name).unwrap(); fs::write(installed.join("bin").join(name), name).unwrap();
        }
        let cli = raw.join("codex.exe").into_os_string();
        let server = raw.join("codex-app-server.exe").into_os_string();
        let manager = installed.join("bin/codex.exe").to_str().unwrap().to_owned();
        (root, cli, server, manager)
    }

    #[test]
    fn runtime_only_selection_reuses_only_an_identical_installed_complete_package() {
        let (root, cli, server, manager) = runtime_only_fixture();
        let evidence = replacement_package_evidence(&cli, &server, "standalone", VERSION, Some(&manager),
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20)).unwrap();
        assert_eq!(evidence.cli, Path::new(&manager).canonicalize().unwrap());
        assert_eq!(evidence.server, Path::new(&manager).with_file_name("codex-app-server.exe").canonicalize().unwrap());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn same_version_different_runtime_bytes_never_selects_the_installed_package() {
        let (root, cli, server, manager) = runtime_only_fixture();
        for name in ["codex.exe", "codex-app-server.exe", "codex-code-mode-host.exe"] {
            fs::write(root.join("raw").join(name), b"different build with the same version").unwrap();
            let error = replacement_package_evidence(&cli, &server, "standalone", VERSION, Some(&manager),
                &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20)).err().expect("different bytes must fail");
            assert!(error.contains("파일이 다릅니다"));
            fs::write(root.join("raw").join(name), name).unwrap();
        }
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn missing_runtime_helper_is_an_error_not_an_installed_package_fallback() {
        let (root, cli, server, manager) = runtime_only_fixture();
        fs::remove_file(root.join("raw/codex-code-mode-host.exe")).unwrap();
        assert!(replacement_package_evidence(&cli, &server, "standalone", VERSION, Some(&manager),
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20)).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn replacement_postcheck_rejects_same_version_different_files_and_missing_observations() {
        let (root, cli, server, manager) = runtime_only_fixture();
        let cancelled = Arc::new(AtomicBool::new(false));
        let evidence = replacement_package_evidence(&cli, &server, "standalone", VERSION, Some(&manager),
            &cancelled, Instant::now() + Duration::from_secs(20)).unwrap();
        let expected = fixture(pair(), evidence, help());
        let mut observed = pair();
        observed.state = "matched".into(); observed.managed_version = Some(VERSION.into());
        observed.running_version = Some(VERSION.into()); observed.managed_cli_executable = Some(manager.clone());
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_ok());
        for name in ["codex.exe", "codex-app-server.exe", "codex-code-mode-host.exe"] {
            let file = Path::new(&manager).with_file_name(name);
            fs::write(&file, b"different bytes, identical version").unwrap();
            assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err(), "{name}");
            fs::write(file, name).unwrap();
        }
        observed.managed_cli_executable = None;
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err());
        observed.managed_cli_executable = Some(manager.clone());
        fs::remove_file(Path::new(&manager).with_file_name("codex-app-server.exe")).unwrap();
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn cli_only_package_postcheck_uses_the_cli_identity_without_a_standalone_server() {
        let (root, _raw_cli, _raw_server, manager) = runtime_only_fixture();
        fs::remove_file(Path::new(&manager).with_file_name("codex-app-server.exe")).unwrap();
        let cli = OsString::from(&manager);
        let cancelled = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_secs(20);
        let evidence = package_evidence(&cli, &cli, "cli", VERSION, &cancelled, deadline).unwrap();
        let expected = preview_commands_mode(&cli, &cli, "cli", pair(), &cancelled, deadline, true,
            || Ok(evidence), |_, _| Ok(help())).unwrap();
        assert!(expected.can_apply);
        let mut observed = pair(); observed.state = "matched".into();
        observed.managed_version = Some(VERSION.into()); observed.running_version = Some(VERSION.into());
        observed.managed_cli_executable = Some(manager.clone());
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_ok());
        fs::write(&manager, b"different CLI with the same version").unwrap();
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err());
        fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn preview_binds_all_candidate_and_observed_generation_evidence() {
        let baseline = fixture(pair(), evidence(), help()).fingerprint;
        for field in 0..7 {
            let mut pair = pair();
            match field {
                0 => pair.cli_version.push('x'), 1 => pair.selected_server_version.push('x'),
                2 => pair.managed_cli_executable = Some("other".into()), 3 => pair.managed_version = Some("other".into()),
                4 => pair.running_version = Some("other".into()), 5 => pair.socket_path = Some("other".into()),
                _ => pair.selected_server_executable.push('x'),
            }
            assert_ne!(fixture(pair, evidence(), help()).fingerprint, baseline);
        }
        for field in 0..4 {
            let mut evidence = evidence(); let mut help = help();
            match field { 0 => evidence.manifest.push(b'x'), 1 => evidence.cli_hash[0] ^= 1,
                2 => evidence.server_hash[0] ^= 1, _ => help.extend_from_slice(b"\nDifferent help\n") }
            assert_ne!(fixture(pair(), evidence, help).fingerprint, baseline);
        }
        assert_eq!(fixture(pair(), evidence(), help()).fingerprint, baseline);
    }

    #[test]
    fn unsupported_runtime_never_inspects_or_transitions_a_shared_package() {
        let mut unsupported = pair();
        unsupported.state = "unavailable".into();
        unsupported.managed_cli_executable = None; unsupported.managed_version = None;
        unsupported.running_version = None; unsupported.socket_path = None;
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", unsupported,
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20),
            || panic!("미지원 실행 파일은 공유 패키지를 읽으면 안 됨"),
            |_, _| panic!("미지원 실행 파일은 전환 도움말도 조회하면 안 됨")).unwrap();
        assert!(!report.can_apply);
        assert!(report.reason.unwrap().contains("상태를 확인하지 못해"));
    }

    #[test]
    fn unknown_pair_skips_help_and_unsupported_update_skips_package_reads() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut unknown = pair(); unknown.state = "unavailable".into();
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", unknown,
            &cancelled, Instant::now() + Duration::from_secs(20), || panic!("unknown pair 패키지 읽기 금지"),
            |_, _| panic!("unknown pair 명령 금지")).unwrap();
        assert!(!report.can_apply);
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", pair(),
            &cancelled, Instant::now() + Duration::from_secs(20), || panic!("미지원 CLI의 큰 실행 파일 읽기 금지"),
            |args, _| { assert_eq!(args, ["app-server", "daemon", "update", "--help"]); Ok(help_without("      --no-force\n")) }).unwrap();
        assert!(!report.can_apply);
        assert!(report.reason.unwrap().contains("지원하지 않습니다"));
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", pair(),
            &cancelled, Instant::now() + Duration::from_secs(20), || Err("완전한 패키지가 아님".into()),
            |args, _| { assert_eq!(args, ["app-server", "daemon", "update", "--help"]); Ok(help()) }).unwrap();
        assert!(!report.can_apply);
        assert_eq!(report.reason.as_deref(), Some("완전한 패키지가 아님"));
    }

    #[test]
    fn matched_is_blocked_and_confirmed_stopped_can_transition_without_running_version() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut matched = pair(); matched.state = "matched".into();
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", matched,
            &cancelled, Instant::now() + Duration::from_secs(20), || panic!("버전 일치 뒤 파일 읽기 금지"),
            |_, _| panic!("버전 일치 뒤 명령 금지")).unwrap();
        assert!(!report.can_apply);
        assert!(report.reason.unwrap().contains("이미 같습니다"));
        let baseline = fixture(pair(), evidence(), help()).fingerprint;
        for managed in [VERSION, "0.165.0+native-r28.20261029"] {
            let mut stopped = pair(); stopped.state = "stopped".into(); stopped.running_version = None;
            stopped.managed_version = Some(managed.into());
            let report = fixture(stopped, evidence(), help());
            assert!(report.can_apply);
            assert_ne!(report.fingerprint, baseline);
        }
        let mut invalid = pair(); invalid.state = "stopped".into();
        assert!(!fixture(invalid, evidence(), help()).can_apply);
    }

    #[test]
    fn uninstalled_stopped_requires_separate_preparation_without_update_or_package_reads() {
        let mut stopped = pair(); stopped.state = "stopped".into();
        stopped.running_version = None; stopped.managed_version = None;
        let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", stopped,
            &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20),
            || panic!("첫 설치 미지원이면 패키지 읽기 금지"),
            |_, _| panic!("첫 설치를 위한 자동 시작·전환 명령 금지")).unwrap();
        assert!(!report.can_apply);
        assert!(report.package_root.is_none());
        assert_eq!(report.fingerprint.len(), 64);
        assert!(report.reason.unwrap().contains("첫 설치를 지원하지 않으므로"));
    }

    #[test]
    fn mismatch_with_missing_managed_or_running_version_remains_blocked() {
        for missing_managed in [true, false] {
            let mut missing = pair();
            if missing_managed { missing.managed_version = None; } else { missing.running_version = None; }
            let report = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", missing,
                &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20),
                || panic!("불명 버전 패키지 읽기 금지"), |_, _| panic!("불명 버전 명령 금지")).unwrap();
            assert!(!report.can_apply);
        }
    }

    #[test]
    fn exact_no_force_declaration_is_required_without_fallback() {
        for invalid in [help_without("      --no-force\n"),
            b"Usage: codex app-server daemon update [OPTIONS]\n\nOptions:\n      --from-cli\n      --no-force-other\n          description: --no-force\n  -y, --yes\n  -h, --help\n".to_vec()] {
            assert!(!fixture(pair(), evidence(), invalid).can_apply);
        }
        let short_only = b"Usage: codex app-server daemon update [OPTIONS]\n\nOptions:\n      --from-cli\n      --no-force\n  -y\n  -h, --help\n";
        assert!(supports_safe_update(short_only).unwrap());
        assert!(!supports_safe_update(&help_without("  -y, --yes\n")).unwrap());
        let long_only = String::from_utf8(help()).unwrap().replace("  -y, --yes\n", "      --yes\n          description -y\n");
        assert!(!supports_safe_update(long_only.as_bytes()).unwrap());
    }

    #[test]
    fn safe_update_accepts_actual_korean_help_with_exact_guards() {
        let help = include_str!("../../tests/fixtures/opendiy-r36-update-help.ko.txt");
        assert!(supports_safe_update(help.as_bytes()).unwrap());
        assert!(supports_safe_update(help.replace('\n', "\r\n").as_bytes()).unwrap());
        assert!(!supports_safe_update(help.replace("--no-force", "--no-force-other").as_bytes()).unwrap());
        assert!(!supports_safe_update(help.replace("  -y, --yes", "      --yes").as_bytes()).unwrap());
    }

    #[test]
    fn manifest_requires_exact_layout_version_entrypoint_target_and_patch() {
        let target = if env::consts::ARCH == "aarch64" { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" };
        let valid = json!({"layoutVersion":1,"entrypoint":"bin/codex.exe","target":target,"version":VERSION});
        assert!(manifest_matches(&serde_json::to_vec(&valid).unwrap(), VERSION).is_ok());
        for (key, invalid) in [("layoutVersion", json!(2)), ("entrypoint", json!("codex.exe")),
            ("target", json!("x86_64-unknown-linux-gnu")), ("version", json!("0.166.0+native-r32.20261101"))] {
            let mut value = valid.clone(); value[key] = invalid;
            assert!(manifest_matches(&serde_json::to_vec(&value).unwrap(), VERSION).is_err());
        }
        assert!(manifest_matches(&vec![b' '; MANIFEST_LIMIT as usize + 1], VERSION).is_err());
    }

    #[test]
    fn shutdown_failures_are_unknown_and_updated_is_the_only_success() {
        for status in ["updated", "shutdownUnconfirmed", "shutdownTimedOut", "shutdownOutcomeUnknown", "noUpdate", "unknown"] {
            let reply = parse_update_output(&serde_json::to_vec(&json!({"status":status,"message":"fixture"})).unwrap()).unwrap();
            assert_eq!(reply.success, status == "updated");
            assert_eq!(reply.outcome, if status == "updated" { "updated" } else { "unknown" });
            if !reply.success { assert!(reply.message.unwrap().contains("자동 재시도")); }
        }
        for invalid in [b"{partial".as_slice(), b"null", br#"{"status":12}"#, br#"{"status":"updated\n"}"#,
            br#"{"status":"updated","message":false}"#] { assert!(parse_update_output(invalid).is_err()); }
    }

    #[test]
    fn cancellation_and_deadline_prevent_preview_success() {
        let cancelled = Arc::new(AtomicBool::new(false));
        let result = preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", pair(), &cancelled,
            Instant::now() + Duration::from_secs(20), || Ok(evidence()), |_, _| {
                cancelled.store(true, Ordering::Release); Ok(help())
            });
        assert!(result.is_err());
        assert!(preview_commands(&OsString::from("cli"), &OsString::from("server"), "standalone", pair(),
            &Arc::new(AtomicBool::new(false)), Instant::now(), || panic!("시간 초과 뒤 파일 읽기 금지"),
            |_, _| panic!("시간 초과 뒤 명령 금지")).is_err());
    }

    #[test]
    fn package_uses_one_canonical_bin_and_hashes_the_selected_files() {
        struct Fixture(PathBuf);
        impl Drop for Fixture { fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); } }
        let fixture = Fixture(env::temp_dir().join(format!("runtime-transition-{}", Uuid::new_v4())));
        let root = &fixture.0;
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir_all(root.join("other/bin")).unwrap();
        let cli = root.join("bin/codex.exe");
        let server = root.join("bin/codex-app-server.exe");
        fs::write(&cli, b"abc").unwrap();
        fs::write(&server, b"def").unwrap();
        fs::write(root.join("other/bin/codex-app-server.exe"), b"def").unwrap();
        let target = if env::consts::ARCH == "aarch64" { "aarch64-pc-windows-msvc" } else { "x86_64-pc-windows-msvc" };
        let manifest = serde_json::to_vec(&json!({"layoutVersion":1,"entrypoint":"bin/codex.exe","target":target,"version":VERSION})).unwrap();
        fs::write(root.join("codex-package.json"), &manifest).unwrap();
        let cancelled = Arc::new(AtomicBool::new(false));
        let check = |server: &Path, kind: &str| package_evidence(&cli.clone().into_os_string(), &server.as_os_str().to_owned(),
            kind, VERSION, &cancelled, Instant::now() + Duration::from_secs(20));
        let evidence = check(&server, "standalone").unwrap();
        assert_eq!(evidence.manifest, manifest);
        assert_eq!(evidence.root, root.canonicalize().unwrap());
        assert_eq!(evidence.cli_hash, Sha256::digest(b"abc").to_vec());
        assert_eq!(evidence.server_hash, Sha256::digest(b"def").to_vec());
        assert!(check(&root.join("other/bin/codex-app-server.exe"), "standalone").is_err());
        assert!(check(&server, "cli").is_err());
        let same = check(&cli, "cli").unwrap();
        assert_eq!(same.cli_hash, same.server_hash);
        fs::write(root.join("codex-package.json"), vec![b' '; MANIFEST_LIMIT as usize + 1]).unwrap();
        assert!(check(&server, "standalone").is_err());
    }

    // D85 regressions use only pre-existing APIs so the same tests compile in
    // before-with-regression.rs. No real executable, HOME or daemon is used.
    struct D85Package(PathBuf);
    impl Drop for D85Package {
        fn drop(&mut self) { let _ = fs::remove_dir_all(&self.0); }
    }
    impl D85Package {
        fn new(entrypoint: &str) -> Self {
            let fixture = Self(env::temp_dir().join(format!("desktop-d85-{}", Uuid::new_v4())));
            for directory in ["installed/bin", "raw", "other/bin"] {
                fs::create_dir_all(fixture.0.join(directory)).unwrap();
            }
            for directory in ["installed/bin", "raw", "other/bin"] {
                for (name, bytes) in [("arcvera.exe", b"d85-cli".as_slice()), ("codex.exe", b"d85-cli".as_slice()),
                    ("codex-app-server.exe", b"d85-server".as_slice()), ("codex-code-mode-host.exe", b"d85-helper".as_slice())] {
                    fs::write(fixture.0.join(directory).join(name), bytes).unwrap();
                }
            }
            fixture.manifest(entrypoint);
            fixture
        }
        fn manifest(&self, entrypoint: &str) {
            fs::write(self.0.join("installed/codex-package.json"), serde_json::to_vec(&json!({
                "layoutVersion": 1, "entrypoint": entrypoint, "version": VERSION,
                "target": format!("{}-pc-windows-msvc", env::consts::ARCH),
            })).unwrap()).unwrap();
        }
        fn cli(&self, name: &str) -> OsString { self.0.join("installed/bin").join(name).into_os_string() }
        fn server(&self) -> OsString { self.cli("codex-app-server.exe") }
        fn manager(&self, name: &str) -> String { self.0.join("installed/bin").join(name).to_str().unwrap().into() }
        fn full(&self, name: &str, kind: &str) -> Result<PackageEvidence, String> {
            let cli = self.cli(name);
            let server = if kind == "cli" { cli.clone() } else { self.server() };
            package_evidence(&cli, &server, kind, VERSION, &Arc::new(AtomicBool::new(false)),
                Instant::now() + Duration::from_secs(20))
        }
        fn raw(&self, name: &str) -> Result<PackageEvidence, String> {
            replacement_package_evidence(&self.0.join("raw").join(name).into_os_string(),
                &self.0.join("raw/codex-app-server.exe").into_os_string(), "standalone", VERSION,
                Some(&self.manager(name)), &Arc::new(AtomicBool::new(false)), Instant::now() + Duration::from_secs(20))
        }
    }

    #[test]
    fn d85_manifest_entrypoints_bind_current_and_legacy_full_and_cli() {
        for name in ["arcvera.exe", "codex.exe"] {
            let package = D85Package::new(&format!("bin/{name}"));
            let full = package.full(name, "standalone").expect("manifest-bound full package must be accepted");
            assert_eq!(full.cli, Path::new(&package.cli(name)).canonicalize().unwrap());
            assert_eq!(full.server, Path::new(&package.server()).canonicalize().unwrap());
            assert_eq!(full.cli_hash, Sha256::digest(b"d85-cli").to_vec());
            assert_eq!(full.server_hash, Sha256::digest(b"d85-server").to_vec());
            assert_eq!(full.manifest, fs::read(package.0.join("installed/codex-package.json")).unwrap());
            let cli = package.full(name, "cli").expect("manifest-bound CLI mode must be accepted");
            assert_eq!(cli.cli, cli.server);
            assert_eq!(cli.cli_hash, cli.server_hash);
        }
    }

    #[test]
    fn d85_manifest_selection_mismatch_is_rejected_even_for_identical_cli_bytes() {
        for (manifest, selected) in [("bin/arcvera.exe", "codex.exe"), ("bin/codex.exe", "arcvera.exe")] {
            let package = D85Package::new(manifest);
            assert!(package.full(selected, "standalone").is_err());
            assert!(package.full(selected, "cli").is_err());
        }
    }

    #[test]
    fn d85_manifest_rejects_traversal_unsupported_names_and_other_bins() {
        let package = D85Package::new("bin/arcvera.exe");
        for entrypoint in ["../bin/arcvera.exe", "bin/../bin/arcvera.exe", "bin\\arcvera.exe", "/bin/arcvera.exe",
            "C:/bin/arcvera.exe", "bin/other.exe", "arcvera.exe", "bin/ARCVERA.EXE"] {
            package.manifest(entrypoint);
            assert!(package.full("arcvera.exe", "standalone").is_err(), "{entrypoint}");
        }
        package.manifest("bin/arcvera.exe");
        let cancelled = Arc::new(AtomicBool::new(false));
        let deadline = Instant::now() + Duration::from_secs(20);
        let other_server = package.0.join("other/bin/codex-app-server.exe").into_os_string();
        assert!(package_evidence(&package.cli("arcvera.exe"), &other_server, "standalone", VERSION, &cancelled, deadline).is_err());
        let other_cli = package.0.join("other/bin/arcvera.exe").into_os_string();
        assert!(package_evidence(&other_cli, &package.server(), "standalone", VERSION, &cancelled, deadline).is_err());
    }

    #[test]
    fn d85_runtime_only_reuses_the_manifest_bound_current_and_legacy_cli() {
        for name in ["arcvera.exe", "codex.exe"] {
            let package = D85Package::new(&format!("bin/{name}"));
            let evidence = package.raw(name).expect("identical runtime-only package must reuse its bound installed CLI");
            assert_eq!(evidence.cli, Path::new(&package.cli(name)).canonicalize().unwrap());
            assert_eq!(evidence.runtime_helper_hash, Some(Sha256::digest(b"d85-helper").to_vec()));
        }
    }

    #[test]
    fn d85_runtime_only_rejects_each_changed_file_and_missing_helper() {
        let package = D85Package::new("bin/arcvera.exe");
        for (name, bytes) in [("arcvera.exe", b"d85-cli".as_slice()), ("codex-app-server.exe", b"d85-server".as_slice()),
            ("codex-code-mode-host.exe", b"d85-helper".as_slice())] {
            let file = package.0.join("raw").join(name);
            fs::write(&file, b"changed same-version executable").unwrap();
            assert!(package.raw("arcvera.exe").is_err(), "{name}");
            fs::write(file, bytes).unwrap();
        }
        fs::remove_file(package.0.join("raw/codex-code-mode-host.exe")).unwrap();
        assert!(package.raw("arcvera.exe").is_err());
    }

    #[test]
    fn d85_runtime_only_cannot_fall_back_to_another_manifest_cli_name() {
        let package = D85Package::new("bin/codex.exe");
        let raw_cli = package.0.join("raw/arcvera.exe").into_os_string();
        let raw_server = package.0.join("raw/codex-app-server.exe").into_os_string();
        assert!(replacement_package_evidence(&raw_cli, &raw_server, "standalone", VERSION,
            Some(&package.manager("codex.exe")), &Arc::new(AtomicBool::new(false)),
            Instant::now() + Duration::from_secs(20)).is_err());
    }

    #[test]
    fn d85_postcheck_preserves_current_cli_name_and_all_runtime_only_file_identities() {
        let package = D85Package::new("bin/arcvera.exe");
        let expected = fixture(pair(), package.raw("arcvera.exe").unwrap(), help());
        let mut observed = pair();
        observed.state = "matched".into(); observed.managed_version = Some(VERSION.into());
        observed.running_version = Some(VERSION.into()); observed.managed_cli_executable = Some(package.manager("arcvera.exe"));
        let cancelled = Arc::new(AtomicBool::new(false));
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_ok());
        for (name, bytes) in [("arcvera.exe", b"d85-cli".as_slice()), ("codex-app-server.exe", b"d85-server".as_slice()),
            ("codex-code-mode-host.exe", b"d85-helper".as_slice())] {
            let file = package.0.join("installed/bin").join(name);
            fs::write(&file, b"changed same-version installed executable").unwrap();
            assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err(), "{name}");
            fs::write(file, bytes).unwrap();
        }
        // Both CLI names already exist with identical bytes. A manifest/name
        // switch must not authorize a different executable identity.
        package.manifest("bin/codex.exe");
        observed.managed_cli_executable = Some(package.manager("codex.exe"));
        assert!(verify_replacement_target(&expected, &observed, &cancelled).is_err());
    }

    #[test]
    fn d85_package_reads_keep_cancellation_and_expired_deadline_guards() {
        let package = D85Package::new("bin/arcvera.exe");
        for (cancelled, deadline) in [(true, Instant::now() + Duration::from_secs(20)), (false, Instant::now())] {
            let cancelled = Arc::new(AtomicBool::new(cancelled));
            assert!(package_evidence(&package.cli("arcvera.exe"), &package.server(), "standalone", VERSION,
                &cancelled, deadline).is_err());
            assert!(replacement_package_evidence(&package.0.join("raw/arcvera.exe").into_os_string(),
                &package.0.join("raw/codex-app-server.exe").into_os_string(), "standalone", VERSION,
                Some(&package.manager("arcvera.exe")), &cancelled, deadline).is_err());
        }
    }

    #[test]
    fn runtime_trace_requires_exact_explicit_opt_in_without_changing_environment() {
        use std::ffi::OsStr;
        for setting in [None, Some(""), Some("0"), Some("true"), Some(" 1"), Some("1 ")] {
            assert!(!RuntimePreviewTrace::from_setting(setting.map(OsStr::new)).enabled);
        }
        assert!(RuntimePreviewTrace::from_setting(Some(OsStr::new("1"))).enabled);
        assert!(!RuntimePreviewTrace::default().enabled);
    }

    #[test]
    fn runtime_trace_format_contains_only_static_labels_times_and_optional_byte_count() {
        let event = RuntimeTraceEvent { phase: RuntimeTracePhase::FileHash,
            elapsed: Duration::from_millis(12), outcome: RuntimeTraceOutcome::Err, bytes: Some(65536) };
        let mut bytes = Vec::new();
        event.write_to(&mut bytes).unwrap();
        assert_eq!(String::from_utf8(bytes).unwrap(),
            "runtime_preview_trace phase=file_hash elapsed_ms=12 outcome=err bytes=65536\n");
        let mut calls = 0;
        let result = RuntimePreviewTrace::default().measure_with(RuntimeTracePhase::PackageProbe,
            || Ok::<_, String>("private domain payload"), |_| calls += 1);
        assert_eq!(result.unwrap(), "private domain payload");
        assert_eq!(calls, 0, "disabled traces must not reach the sink");
    }

    #[test]
    fn runtime_trace_completes_success_and_error_without_replacing_domain_results() {
        struct FailingWriter;
        impl io::Write for FailingWriter {
            fn write(&mut self, _: &[u8]) -> io::Result<usize> { Err(io::ErrorKind::BrokenPipe.into()) }
            fn flush(&mut self) -> io::Result<()> { Ok(()) }
        }
        let trace = RuntimePreviewTrace { enabled: true };
        let mut outcomes = Vec::new();
        let success = trace.measure_with(RuntimeTracePhase::UpdateHelp, || Ok::<_, String>(17), |event| {
            outcomes.push(event.outcome);
            let _ = event.write_to(&mut FailingWriter);
        });
        let failure = trace.measure_with(RuntimeTracePhase::PairProbe,
            || Err::<(), _>("private sentinel error".to_owned()), |event| {
                outcomes.push(event.outcome);
                let mut bytes = Vec::new(); event.write_to(&mut bytes).unwrap();
                assert!(!String::from_utf8(bytes).unwrap().contains("sentinel"));
            });
        assert_eq!(success, Ok(17));
        assert_eq!(failure, Err("private sentinel error".to_owned()));
        assert_eq!(outcomes, [RuntimeTraceOutcome::Ok, RuntimeTraceOutcome::Err]);
    }

    #[test]
    fn runtime_trace_file_aggregates_keep_hash_and_failure_boundaries() {
        let package = D85Package::new("bin/arcvera.exe");
        let path = PathBuf::from(package.cli("arcvera.exe"));
        let trace = RuntimePreviewTrace { enabled: true };
        let cancelled = Arc::new(AtomicBool::new(false));
        let mut records = Vec::new();
        let hash = file_hash_observed(&path, &cancelled, Instant::now() + Duration::from_secs(20), trace,
            |events| records.extend(events)).unwrap();
        assert_eq!(hash, Sha256::digest(b"d85-cli").to_vec());
        assert_eq!(records.len(), 3);
        assert_eq!(records.iter().map(|event| event.bytes).collect::<Vec<_>>(), [Some(7); 3]);
        assert!(records.iter().all(|event| event.outcome == RuntimeTraceOutcome::Ok));
        let mut calls = 0;
        assert_eq!(file_hash_observed(&path, &cancelled, Instant::now() + Duration::from_secs(20),
            RuntimePreviewTrace::default(), |_| calls += 1).unwrap(), hash);
        assert_eq!(calls, 0, "existing disabled file fixtures must not reach the sink");
        for (path, cancelled, deadline) in [
            (path.clone(), Arc::new(AtomicBool::new(true)), Instant::now() + Duration::from_secs(20)),
            (path.clone(), Arc::new(AtomicBool::new(false)), Instant::now()),
            (path.with_file_name("missing-private-sentinel.exe"), cancelled.clone(), Instant::now() + Duration::from_secs(20)),
        ] {
            let mut records = Vec::new();
            assert!(file_hash_observed(&path, &cancelled, deadline, trace, |events| records.extend(events)).is_err());
            assert_eq!(records.len(), 3);
            assert_eq!(records[0].outcome, RuntimeTraceOutcome::Err);
            assert!(records[1..].iter().all(|event| event.outcome == RuntimeTraceOutcome::NotRun));
        }
    }

    #[test]
    fn runtime_trace_preview_classification_does_not_promote_blocked_or_error_to_success() {
        let allowed = Ok(fixture(pair(), evidence(), help()));
        let blocked = Ok(fixture(pair(), evidence(), help_without("      --no-force\n")));
        let error = Err("private preview error".to_owned());
        assert_eq!(runtime_preview_outcome(&allowed), RuntimeTraceOutcome::Ok);
        assert_eq!(runtime_preview_outcome(&blocked), RuntimeTraceOutcome::Blocked);
        assert_eq!(runtime_preview_outcome(&error), RuntimeTraceOutcome::Err);
        assert!(allowed.unwrap().can_apply);
        assert!(!blocked.unwrap().can_apply);
        assert_eq!(error.unwrap_err(), "private preview error");
    }
}
