//! Candidate Core 1.1 metadata adapter. Real support requires the paired runtime test.
use super::*;
const INVALID: &str = "컨텍스트 구성의 대상·형식·상한을 확인하지 못했습니다.";
const LIMIT: usize = 25;
const SAFE: u64 = 9_007_199_254_740_991;

pub(super) fn validate(snapshot: Option<&str>, cursor: u64) -> Result<(), String> {
    if cursor > SAFE || (cursor > 0 && snapshot.is_none()) { return Err(INVALID.into()); }
    if let Some(id) = snapshot { text(&json!(id), 256)?; }
    Ok(())
}

fn text(value: &Value, max: usize) -> Result<Value, String> {
    let s = value.as_str().ok_or(INVALID)?;
    if s.is_empty() || s.len() > max || s.chars().any(|c| c.is_control() || matches!(c, '\u{202a}'..='\u{202e}' | '\u{2066}'..='\u{2069}')) { return Err(INVALID.into()); }
    Ok(json!(s))
}
fn field<'a>(v: &'a Value, name: &str) -> Result<&'a Value, String> { v.get(name).ok_or_else(|| INVALID.into()) }
fn number(v: &Value, identity: bool, positive: bool) -> Result<Value, String> {
    let n = v.as_u64().ok_or(INVALID)?;
    if positive && n == 0 { return Err(INVALID.into()); }
    if identity { Ok(json!(n.to_string())) } else if n <= SAFE { Ok(json!(n)) } else { Err(INVALID.into()) }
}
fn numbers(v: &Value, out: &mut Value, keys: &[&str], nullable: bool, identity: bool, positive: bool) -> Result<(), String> {
    for key in keys { let value = field(v, key)?; out[*key] = if value.is_null() && nullable { Value::Null } else { number(value, identity, positive)? }; }
    Ok(())
}
fn texts(v: &Value, out: &mut Value, keys: &[&str], nullable: bool, max: usize) -> Result<(), String> {
    for key in keys { let value = field(v, key)?; out[*key] = if value.is_null() && nullable { Value::Null } else { text(value, max)? }; }
    Ok(())
}
pub(super) fn exchange<S: Read + Write>(rpc: &mut Rpc<S>, thread: &str, snapshot: Option<&str>, cursor: u64) -> Result<Value, String> {
    validate(snapshot, cursor)?;
    let mut params = json!({"threadId":thread,"cursor":cursor,"limit":LIMIT});
    if let Some(id) = snapshot { params["snapshotId"] = json!(id); }
    let reply = match rpc.call_envelope("thread/context/read", params) {
        Ok(value) => value,
        Err(_) => return Ok(json!({"supported":false,"reason":"unavailable"})),
    };
    if let Some(error) = reply.get("error") {
        let reason = if error["code"].as_i64() == Some(-32601) { "unsupported" }
            else if error["code"].as_i64() == Some(-32600) && error["data"]["reason"] == "staleContextSnapshot" { "stale" }
            else { "unavailable" };
        return Ok(json!({"supported":false,"reason":reason}));
    }
    let value = field(&reply, "result")?;
    Ok(json!({"supported":true,"snapshot":project(value, thread, snapshot, cursor)?}))
}
fn project(v: &Value, thread: &str, expected_snapshot: Option<&str>, cursor: u64) -> Result<Value, String> {
    // Existing Rpc frame bound remains 8 MiB. This metadata protocol additionally caps 64 KiB.
    if serde_json::to_vec(v).map_err(|_| INVALID)?.len() > 64 * 1024 || v["threadId"].as_str() != Some(thread) { return Err(INVALID.into()); }
    if expected_snapshot.is_some_and(|id| v["snapshotId"].as_str() != Some(id)) { return Err(INVALID.into()); }
    let mut out = json!({});
    texts(v, &mut out, &["threadId","snapshotId"], false, 256)?;
    texts(v, &mut out, &["asOf"], false, 64)?;
    numbers(v, &mut out, &["contextIndexRevision","historyVersion"], false, true, false)?;
    let sizes = field(v, "sizes")?; let mut s = json!({});
    numbers(sizes, &mut s, &["residentSerializedBytes","selectedResidentSerializedBytes","excludedResidentSerializedBytes","selectedBodyEstimatedTokens","navigationEstimatedTokens"], false, false, false)?;
    numbers(sizes, &mut s, &["archiveSerializedBytes","baseEstimatedTokens","toolEstimatedTokens","totalEstimatedTokens"], true, false, false)?;
    texts(sizes, &mut s, &["estimateProvenance"], false, 256)?; out["sizes"] = s;
    let limits = field(v, "limits")?; let mut l = json!({});
    numbers(limits, &mut l, &["requestedContextWindowOverride","effectiveContextWindow"], true, false, true)?;
    texts(limits, &mut l, &["provenance"], false, 256)?; out["limits"] = l;
    let fragments = field(v, "fragments")?.as_array().ok_or(INVALID)?;
    if fragments.len() > LIMIT { return Err(INVALID.into()); }
    let mut rows = Vec::with_capacity(fragments.len());
    for row in fragments {
        let mut r = json!({});
        texts(row, &mut r, &["sourceId","taskId","kind","selectionReason"], true, 256)?;
        texts(row, &mut r, &["residency"], false, 256)?;
        numbers(row, &mut r, &["sourceOrdinal","residentOccurrence"], true, true, false)?;
        numbers(row, &mut r, &["residentSerializedBytes","estimatedTokens"], true, false, false)?;
        r["selected"] = json!(field(row,"selected")?.as_bool().ok_or(INVALID)?);
        let required = field(row,"required")?;
        r["required"] = if required.is_null() { Value::Null } else { json!(required.as_bool().ok_or(INVALID)?) };
        rows.push(r);
    }
    out["fragments"] = json!(rows);
    let cache = field(v, "cache")?; let mut c = json!({});
    texts(cache, &mut c, &["scope","provenance","status","revisionStatus","ttlSupport","ttlApplication","minimumValidUntilProvenance","minimumValidUntilScope"], false, 256)?;
    texts(cache, &mut c, &["observationId"], true, 256)?;
    texts(cache, &mut c, &["observedAt","requestStartedAt","minimumValidUntil","estimatedExpiry","serverConfirmedExpiry"], true, 64)?;
    numbers(cache, &mut c, &["cachedInputTokens","writeInputTokens","uncachedInputTokens","totalInputTokens","requestedTtlSeconds","effectiveTtlSeconds"], true, false, false)?;
    numbers(cache, &mut c, &["requestContextIndexRevision"], true, true, false)?;
    out["cache"] = c;
    let next = field(v, "nextCursor")?;
    if !next.is_null() {
        let n = next.as_u64().filter(|n| *n <= SAFE && *n > cursor && *n == cursor + fragments.len() as u64).ok_or(INVALID)?;
        out["nextCursor"] = json!(n);
    } else { out["nextCursor"] = Value::Null; }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    struct Memory { input: Cursor<Vec<u8>>, output: Vec<u8> }
    impl Read for Memory { fn read(&mut self, b: &mut [u8]) -> io::Result<usize> { self.input.read(b) } }
    impl Write for Memory {
        fn write(&mut self, b: &[u8]) -> io::Result<usize> { self.output.extend_from_slice(b); Ok(b.len()) }
        fn flush(&mut self) -> io::Result<()> { Ok(()) }
    }
    fn rpc(reply: Value) -> Rpc<Memory> {
        let data = reply.to_string().into_bytes(); let mut frame = vec![0x81];
        if data.len() <= 125 { frame.push(data.len() as u8); }
        else if data.len() <= 65535 { frame.push(126); frame.extend_from_slice(&(data.len() as u16).to_be_bytes()); }
        else { frame.push(127); frame.extend_from_slice(&(data.len() as u64).to_be_bytes()); }
        frame.extend(data);
        Rpc { ws: WebSocket::from_raw_socket(Memory { input: Cursor::new(frame), output: vec![] }, tungstenite::protocol::Role::Client, None), core_version: None, next_id: 0, cancelled: None, deadline: None }
    }
    fn snapshot() -> Value {
        json!({"threadId":"thread-a","snapshotId":"snapshot-a","contextIndexRevision":u64::MAX,"historyVersion":0,"asOf":"2026-11-13T23:00:00Z",
            "sizes":{"residentSerializedBytes":0,"selectedResidentSerializedBytes":0,"excludedResidentSerializedBytes":0,"archiveSerializedBytes":null,"selectedBodyEstimatedTokens":0,"estimateProvenance":"heuristic","navigationEstimatedTokens":0,"baseEstimatedTokens":null,"toolEstimatedTokens":null,"totalEstimatedTokens":null},
            "limits":{"requestedContextWindowOverride":null,"effectiveContextWindow":1000,"provenance":"session"},
            "fragments":[{"sourceId":"source-a","sourceOrdinal":u64::MAX,"residentOccurrence":0,"taskId":null,"kind":"tool_result","selected":true,"required":null,"selectionReason":"selected","residency":"resident","residentSerializedBytes":0,"estimatedTokens":0}],
            "cache":{"scope":"aggregate","provenance":"not_reported","status":"unknown","cachedInputTokens":null,"writeInputTokens":0,"uncachedInputTokens":null,"totalInputTokens":null,"observedAt":null,"observationId":null,"requestContextIndexRevision":null,"revisionStatus":"unknown","requestedTtlSeconds":null,"effectiveTtlSeconds":null,"ttlSupport":"unknown","ttlApplication":"unknown","requestStartedAt":null,"minimumValidUntil":null,"minimumValidUntilProvenance":"unknown","minimumValidUntilScope":"reported_write_tokens_only_not_individual_fragments","estimatedExpiry":null,"serverConfirmedExpiry":null},"nextCursor":null})
    }
    #[test]
    fn composition_wire_is_single_read_and_projects_only_metadata() {
        let mut v = snapshot(); v["rawBody"] = json!("SECRET_SENTINEL"); v["fragments"][0]["rawBody"] = json!("SECRET_SENTINEL");
        let mut r = rpc(json!({"id":1,"result":v}));
        let out = exchange(&mut r,"thread-a",None,0).unwrap();
        assert_eq!(r.next_id,1); assert_eq!(out["supported"],true);
        assert_eq!(out["snapshot"]["contextIndexRevision"],u64::MAX.to_string());
        assert_eq!(out["snapshot"]["fragments"][0]["sourceOrdinal"],u64::MAX.to_string());
        assert_eq!(out["snapshot"]["cache"]["writeInputTokens"],0);
        assert!(out["snapshot"]["cache"]["cachedInputTokens"].is_null());
        assert!(!out.to_string().contains("SECRET_SENTINEL"));
        let mut server = WebSocket::from_raw_socket(Memory { input: Cursor::new(r.ws.get_ref().output.clone()), output: vec![] },tungstenite::protocol::Role::Server,None);
        let request: Value = serde_json::from_str(server.read().unwrap().to_text().unwrap()).unwrap();
        assert_eq!(request["method"],"thread/context/read");
        assert_eq!(request["params"],json!({"threadId":"thread-a","cursor":0,"limit":25}));
    }
    #[test]
    fn composition_structured_errors_do_not_parse_or_expose_prose() {
        for (code,data,expected) in [(-32601,json!({}),"unsupported"),(-32600,json!({"reason":"staleContextSnapshot"}),"stale"),(-32600,json!({}),"unavailable"),(-32602,json!({"reason":"staleContextSnapshot"}),"unavailable")] {
            let mut r = rpc(json!({"id":1,"error":{"code":code,"data":data,"message":"SECRET staleContextSnapshot"}}));
            let result = exchange(&mut r,"thread-a",None,0).unwrap();
            assert_eq!(result,json!({"supported":false,"reason":expected})); assert_eq!(r.next_id,1);
        }
    }
    #[test]
    fn composition_paging_rejects_missing_snapshot_before_sending() {
        let mut r = rpc(json!({"id":1,"result":snapshot()}));
        assert!(exchange(&mut r,"thread-a",None,1).is_err()); assert_eq!(r.next_id,0); assert!(r.ws.get_ref().output.is_empty());
        let mut v = snapshot(); v["nextCursor"] = json!(2);
        assert!(project(&v,"thread-a",None,0).is_err());
        v["nextCursor"] = json!(1); assert!(project(&v,"thread-a",Some("snapshot-a"),0).is_ok());
    }
    #[test]
    fn composition_target_and_snapshot_mismatch_never_retry() {
        for (thread,id) in [("thread-b",None),("thread-a",Some("snapshot-b"))] {
            let mut r = rpc(json!({"id":1,"result":snapshot()}));
            assert!(exchange(&mut r,thread,id,0).is_err()); assert_eq!(r.next_id,1);
        }
    }
    #[test]
    fn composition_preserves_unknown_strings_and_unknown_quantities() {
        let mut v = snapshot(); v["cache"]["status"] = json!("future-status");
        let out = project(&v,"thread-a",None,0).unwrap();
        assert_eq!(out["cache"]["status"],"future-status"); assert!(out["sizes"]["archiveSerializedBytes"].is_null());
        assert!(out["fragments"][0]["required"].is_null()); assert_eq!(out["fragments"][0]["residentSerializedBytes"],0);
    }
    #[test]
    fn composition_used_field_types_and_bounds_are_not_coerced() {
        for (section,key,value) in [("sizes","residentSerializedBytes",json!(SAFE+1)),("sizes","selectedBodyEstimatedTokens",json!(-1)),("limits","effectiveContextWindow",json!(0)),("cache","writeInputTokens",json!(1.5)),("cache","status",json!("x".repeat(257)))] {
            let mut v = snapshot(); v[section][key] = value; assert!(project(&v,"thread-a",None,0).is_err(),"{section}.{key}");
        }
        let mut v = snapshot(); v["fragments"] = json!(vec![v["fragments"][0].clone();26]); assert!(project(&v,"thread-a",None,0).is_err());
        let mut v = snapshot(); v["unused"] = json!("x".repeat(64*1024)); assert!(project(&v,"thread-a",None,0).is_err());
        let mut v = snapshot(); v["cache"].as_object_mut().unwrap().remove("cachedInputTokens"); assert!(project(&v,"thread-a",None,0).is_err());
    }
}
