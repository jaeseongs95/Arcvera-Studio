param([ValidatePattern('^[0-9a-f]{40}$')][string]$Baseline='1471e6adb86f612693e713491dd7f19227077391')
$ErrorActionPreference='Stop'
$PSNativeCommandUseErrorActionPreference=$false
$env:CARGO_TERM_COLOR='never'
$output='artifacts/native-adapters/d87-response-contract'
if(Test-Path -LiteralPath "$output/proof.json"){throw 'Preserve previous proof'}
[IO.Directory]::CreateDirectory($output)|Out-Null
git cat-file -e "${Baseline}^{commit}" 2>$null
if($LASTEXITCODE-ne0){git fetch --no-tags origin $Baseline;if($LASTEXITCODE-ne0){throw 'Baseline source unavailable'}}
$paths=@('src-tauri/src/session.rs','src-tauri/src/codex_conversation.rs','src-tauri/src/codex_conversation_protocol_tests.rs')
$original=@{}
foreach($file in $paths){$original[$file]=[IO.File]::ReadAllBytes($file)}
$utf8=[Text.UTF8Encoding]::new($false)
$sha={param([byte[]]$bytes)[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData($bytes)).ToLowerInvariant()}
$witness=@'

#[cfg(test)]
mod d0087_boundary_witness {
    use super::*;
    #[test]
    fn standard_mcp_tool_response_contract() {
        let request=serde_json::json!({"id":71,"method":"mcpServer/elicitation/request","params":{
            "origin":"core","threadId":"original","turnId":"tool-turn","serverName":"review-server","mode":"form",
            "message":"Review fixture tool","requestedSchema":{"type":"object","properties":{}},
            "_meta":{"codex_approval_kind":"mcp_tool_call","tool_name":"review_tool","tool_params":{},"persist":["session","always"]}}});
        for action in ["accept","decline","cancel"] {
            let response=serde_json::json!({"action":action,"content":null,"_meta":null});
            let result=validate_response(&request,&response);
            assert!(result.is_ok(),"D0087_STANDARD_APPROVAL_REJECTED: {result:?}");
        }
        for invalid in [serde_json::json!({"action":"accept","content":null,"_meta":{"persist":"always"}}),
            serde_json::json!({"action":"accept","content":null,"_meta":{"persist":"session"}}),
            serde_json::json!({"action":"accept","content":{"unreviewed":true},"_meta":null}),
            serde_json::json!({"action":"accept","content":null,"_meta":null,"remember":true})] {
            assert!(validate_response(&request,&invalid).is_err());
        }
    }
}
'@
function Append-Witness {
    $file='src-tauri/src/codex_conversation.rs'
    [IO.File]::WriteAllBytes($file,[byte[]]([IO.File]::ReadAllBytes($file)+$utf8.GetBytes("`n$witness`n")))
}
$redExit=$null;$greenExit=$null
try {
    foreach($file in $paths){
        $body=git show "${Baseline}:$file"
        if($LASTEXITCODE-ne0){throw 'Baseline file unavailable'}
        [IO.File]::WriteAllText($file,($body-join"`n")+"`n",$utf8)
    }
    Append-Witness
    cargo test --lib --locked --manifest-path src-tauri/Cargo.toml d0087_boundary_witness -- --test-threads=1 2>&1 | Tee-Object -FilePath "$output/before.log"
    $redExit=$LASTEXITCODE
} finally {
    foreach($file in $paths){[IO.File]::WriteAllBytes($file,$original[$file])}
    foreach($file in $paths){if((&$sha ([IO.File]::ReadAllBytes($file)))-ne(&$sha $original[$file])){throw 'Candidate restoration mismatch'}}
}
$red=[IO.File]::ReadAllText("$output/before.log")
$redRows=[regex]::Matches($red,'(?m)^test [\w:]+standard_mcp_tool_response_contract \.\.\. FAILED\r?$')
if($redExit-ne101-or$redRows.Count-ne1-or$red-notmatch'D0087_STANDARD_APPROVAL_REJECTED'-or$red-notmatch'현재 화면에서 지원하지 않는 서버 요청'){
    throw 'Baseline did not fail at the original unsupported-request validator boundary'
}
try {
    Append-Witness
    cargo test --lib --locked --manifest-path src-tauri/Cargo.toml d0087_boundary_witness -- --test-threads=1 2>&1 | Tee-Object -FilePath "$output/fixed.log"
    $greenExit=$LASTEXITCODE
} finally {
    foreach($file in $paths){[IO.File]::WriteAllBytes($file,$original[$file])}
    foreach($file in $paths){if((&$sha ([IO.File]::ReadAllBytes($file)))-ne(&$sha $original[$file])){throw 'Candidate restoration mismatch'}}
}
$green=[IO.File]::ReadAllText("$output/fixed.log")
$greenRows=[regex]::Matches($green,'(?m)^test [\w:]+standard_mcp_tool_response_contract \.\.\. ok\r?$')
@{baseline=$Baseline;candidate=(git rev-parse HEAD);witnessSha256=(&$sha ($utf8.GetBytes($witness)));beforeExit=$redExit;fixedExit=$greenExit;beforeCaseCount=$redRows.Count;fixedCaseCount=$greenRows.Count;candidateRestored=$true;scope='Same pure Native response validator witness; no GUI/Core/permission-owner integration proof'}|ConvertTo-Json -Depth 4|Set-Content -LiteralPath "$output/proof.json" -Encoding utf8NoBOM
if($greenExit-ne0-or$greenRows.Count-ne1){throw 'Fixed response validator witness failed'}

# A separate mutation probes the newly added original-number review ceiling.
# It does not pretend the uncommitted D87 intermediate was a released baseline.
$numericFilter='mcp_tool_approval_numeric_review_ceiling_checks_original_number_variants'
$nativePath='src-tauri/src/codex_conversation.rs'
$fixedText=$utf8.GetString($original[$nativePath])
$marker='fn mcp_arguments_reviewable(value: &Value, depth: usize) -> bool {'
$offset=$fixedText.IndexOf($marker,[StringComparison]::Ordinal)
if($offset-lt0-or$offset-ne$fixedText.LastIndexOf($marker,[StringComparison]::Ordinal)){throw 'Numeric mutation target is not unique'}
$insert=$fixedText.IndexOf('    match value {',$offset,[StringComparison]::Ordinal)
if($insert-lt$offset-or$insert-$offset-gt250){throw 'Numeric mutation boundary changed'}
$mutant=$fixedText.Insert($insert,"    if value.is_number() {return true;} // D89 test-only mutation`n")
try {
    [IO.File]::WriteAllText($nativePath,$mutant,$utf8)
    cargo test --lib --locked --manifest-path src-tauri/Cargo.toml $numericFilter -- --test-threads=1 2>&1 | Tee-Object -FilePath "$output/d89-mutant.log"
    $mutantExit=$LASTEXITCODE
} finally {
    [IO.File]::WriteAllBytes($nativePath,$original[$nativePath])
    if((&$sha ([IO.File]::ReadAllBytes($nativePath)))-ne(&$sha $original[$nativePath])){throw 'Numeric candidate restoration mismatch'}
}
$mutationLog=[IO.File]::ReadAllText("$output/d89-mutant.log")
if($mutantExit-ne101-or$mutationLog-notmatch'request classification mismatch: u64-adjacent'-or[regex]::Matches($mutationLog,"(?m)^test [\w:]+$numericFilter \.\.\. FAILED\r?$").Count-ne1){throw 'Numeric mutation did not fail at the unsafe original-integer boundary'}
cargo test --lib --locked --manifest-path src-tauri/Cargo.toml $numericFilter -- --test-threads=1 2>&1 | Tee-Object -FilePath "$output/d89-fixed.log"
$numericExit=$LASTEXITCODE
$numericLog=[IO.File]::ReadAllText("$output/d89-fixed.log")
@{candidate=(git rev-parse HEAD);mutationSourceSha256=(&$sha ($utf8.GetBytes($mutant)));fixedSourceSha256=(&$sha $original[$nativePath]);testFileSha256=(&$sha $original['src-tauri/src/codex_conversation_protocol_tests.rs']);mutationExit=$mutantExit;fixedExit=$numericExit;candidateRestored=$true;scope='Original-number classifier mutation only; transport/GUI/real Core are separate evidence'}|ConvertTo-Json|Set-Content -LiteralPath "$output/d89-proof.json" -Encoding utf8NoBOM
if($numericExit-ne0-or[regex]::Matches($numericLog,"(?m)^test [\w:]+$numericFilter \.\.\. ok\r?$").Count-ne1){throw 'Numeric fixed boundary failed'}
