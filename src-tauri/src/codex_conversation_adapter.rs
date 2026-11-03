//! 공통 app-server 계약과 명시적으로 선언된 선택 확장의 wire 경계.
use serde_json::{json, Value};

pub(super) const PASSIVE_SUBSCRIBE: &str = "thread/subscribePassive";
pub(super) const START_USER_IF_IDLE: &str = "turn/startUserIfIdle";
pub(super) const COMPACT_IF_IDLE: &str = "thread/compact/startIfIdle";

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ExtensionProfile {
    pub passive_subscription: bool,
    pub atomic_user_start: bool,
    pub idle_compaction: bool,
    pub expected_turn_interrupt: bool,
    pub subagents: bool,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct RuntimeProtocol { pub extensions: ExtensionProfile }

fn declared(value: &Value, field: &str) -> Result<bool, String> {
    match value.get(field) {
        None => Ok(false),
        Some(Value::Bool(supported)) => Ok(*supported),
        Some(_) => Err("서버의 기능 지원 정보 형식이 올바르지 않습니다.".into()),
    }
}

pub(super) fn from_initialize(value: &Value) -> Result<RuntimeProtocol, String> {
    if !value.is_object() { return Err("서버 초기화 응답이 올바른 객체가 아닙니다.".into()); }
    let capabilities=match value.get("opendiyCapabilities") {
        None | Some(Value::Null)=>None,
        Some(value) if value.is_object()=>Some(value),
        Some(_)=>return Err("선택 확장 지원 정보는 객체 또는 null이어야 합니다.".into()),
    };
    let extension=|field|match capabilities.and_then(|value|value.get(field)) {
        None=>Ok(false),
        Some(Value::Bool(value))=>Ok(*value),
        Some(_)=>Err(format!("선택 확장 지원 정보의 {field} 값은 bool이어야 합니다.")),
    };
    // historySummary는 선택 최적화의 선언일 뿐, 공통 페이지 이력의 지원
    // 조건이 아니다. 형식은 검증하지만 공통 이력 경로를 차단하지 않는다.
    let _=extension("historySummary")?;
    Ok(RuntimeProtocol { extensions: ExtensionProfile {
        // 현재 연결의 명시 선언만 사용한다. 공식·R34의 객체 부재와
        // 다른 기능의 bool 또는 제품 버전에서 확장 지원을 추론하지 않는다.
        passive_subscription: extension("passiveSubscription")?,
        atomic_user_start: extension("userTurnStartIfIdle")?,
        idle_compaction: extension("compactStartIfIdle")?,
        expected_turn_interrupt: declared(value, "supportsExpectedTurnInterrupt")?,
        subagents: declared(value, "supportsSubagentManagement")?,
    } })
}

impl RuntimeProtocol {
    pub(super) fn attach(self, thread_id: &str) -> (&'static str, Value) {
        if self.extensions.passive_subscription { (PASSIVE_SUBSCRIBE, json!({"threadId":thread_id})) }
        else { ("thread/resume", json!({"threadId":thread_id,"excludeTurns":true})) }
    }
    pub(super) fn start_method(self) -> &'static str {
        if self.extensions.atomic_user_start { START_USER_IF_IDLE } else { "turn/start" }
    }
    pub(super) fn compact_method(self) -> &'static str {
        if self.extensions.idle_compaction { COMPACT_IF_IDLE } else { "thread/compact/start" }
    }
    pub(super) fn normalize_start(self, value: Value) -> Value {
        if self.extensions.atomic_user_start { value }
        else if value["turn"]["id"].as_str().is_some_and(|id| !id.is_empty()) {
            json!({"status":"started","turn":value["turn"]})
        } else { value }
    }
}

pub(super) fn is_mutation(method: &str) -> bool {
    matches!(method, START_USER_IF_IDLE | COMPACT_IF_IDLE | "turn/start" | "thread/compact/start" | "turn/interrupt" | "thread/resume")
}

pub(super) fn accepted(method: &str, message: &Value) -> bool {
    if message.get("error").is_some() { return false; }
    match method {
        START_USER_IF_IDLE | COMPACT_IF_IDLE => message["result"]["status"] == "started"
            && message["result"]["turn"]["id"].as_str().is_some_and(|id| !id.is_empty()),
        "turn/start" => message["result"]["turn"]["id"].as_str().is_some_and(|id| !id.is_empty()),
        "thread/resume" => message["result"]["thread"]["id"].is_string(),
        "thread/compact/start" => message["result"].is_object(),
        _ => message.get("result").is_some(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn standard_initialize_uses_only_standard_methods_and_metadata_resume() {
        let protocol = from_initialize(&json!({"userAgent":"any/0.166.0"})).unwrap();
        assert_eq!(protocol.attach("thread"), ("thread/resume",json!({"threadId":"thread","excludeTurns":true})));
        assert_eq!(protocol.start_method(), "turn/start");
        assert_eq!(protocol.compact_method(), "thread/compact/start");
        assert_eq!(protocol.normalize_start(json!({"turn":{"id":"turn"}})),json!({"status":"started","turn":{"id":"turn"}}));
        assert!(accepted("thread/compact/start", &json!({"result":{}})));
        assert!(!accepted("turn/start", &json!({"error":{"code":-32603}})));
    }

    #[test]
    fn extension_declarations_are_independent_and_strict() {
        let protocol = from_initialize(&json!({"supportsSubagentManagement":true,"supportsExpectedTurnInterrupt":true})).unwrap();
        assert_eq!(protocol.start_method(), "turn/start");
        assert_eq!(protocol.attach("thread").0,"thread/resume");
        assert_eq!(protocol.compact_method(),"thread/compact/start");
        for field in ["supportsExpectedTurnInterrupt","supportsSubagentManagement"] {
            for invalid in [Value::Null,json!("true"),json!(1)] {
                let mut value=json!({});value[field]=invalid;
                assert!(from_initialize(&value).is_err(),"{field}");
            }
        }
    }

    #[test]
    fn r35_explicit_extensions_select_each_method_independently_without_history_gating() {
        for flags in 0..16 {
            let passive=flags&1!=0;let start=flags&2!=0;let compact=flags&4!=0;let history=flags&8!=0;
            let protocol=from_initialize(&json!({"supportsExpectedTurnInterrupt":true,"opendiyCapabilities":{
                "passiveSubscription":passive,"userTurnStartIfIdle":start,"compactStartIfIdle":compact,"historySummary":history}})).unwrap();
            assert_eq!(protocol.attach("thread").0,if passive{PASSIVE_SUBSCRIBE}else{"thread/resume"});
            assert_eq!(protocol.start_method(),if start{START_USER_IF_IDLE}else{"turn/start"});
            assert_eq!(protocol.compact_method(),if compact{COMPACT_IF_IDLE}else{"thread/compact/start"});
            assert!(protocol.extensions.expected_turn_interrupt,"기대 turn 보호 선언은 다른 확장과 독립이다");
        }
    }

    #[test]
    fn missing_null_or_false_extensions_never_infer_support_from_version_or_other_flags() {
        for capabilities in [Value::Null,json!({}),json!({"passiveSubscription":false,"userTurnStartIfIdle":false,"compactStartIfIdle":false})] {
            let protocol=from_initialize(&json!({"userAgent":"opendiy/0.166.0+native-r35","supportsSubagentManagement":true,
                "supportsExpectedTurnInterrupt":true,"supportsCompactIfIdle":true,"opendiyCapabilities":capabilities})).unwrap();
            assert_eq!(protocol.attach("thread").0,"thread/resume");assert_eq!(protocol.start_method(),"turn/start");assert_eq!(protocol.compact_method(),"thread/compact/start");
        }
        let protocol=from_initialize(&json!({"userAgent":"opendiy/0.166.0+native-r35","supportsSubagentManagement":true,"supportsExpectedTurnInterrupt":true})).unwrap();
        assert!(!protocol.extensions.passive_subscription && !protocol.extensions.atomic_user_start && !protocol.extensions.idle_compaction);
    }

    #[test]
    fn malformed_extension_objects_and_fields_reject_the_entire_profile() {
        for invalid in [json!(true),json!(1),json!("true"),json!([])] {
            let error=from_initialize(&json!({"opendiyCapabilities":invalid})).unwrap_err();assert!(error.contains("객체 또는 null"));
        }
        for field in ["passiveSubscription","userTurnStartIfIdle","compactStartIfIdle","historySummary"] {
            for invalid in [Value::Null,json!("true"),json!(1),json!([]),json!({})] {
                let mut capabilities=json!({"passiveSubscription":true,"userTurnStartIfIdle":true,"compactStartIfIdle":true,"historySummary":true});
                capabilities[field]=invalid;
                let error=from_initialize(&json!({"opendiyCapabilities":capabilities})).unwrap_err();
                assert!(error.contains(field) && error.contains("bool"));
            }
        }
    }
}
