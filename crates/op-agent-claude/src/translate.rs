use op_agent::{
    ApprovalDecision, ApprovalId, ApprovalRequest, ItemId, RateLimit, RateLimitWindow, SessionId,
    SessionInfo, ToolOutcome, Usage,
};
use op_claude::{ContentBlock, Extra, KnownContentBlock, KnownStreamInput, Message, StreamInput};
use serde_json::{Value, json};

pub fn session_info(session_id: &str, extra: &Extra) -> SessionInfo {
    SessionInfo {
        session: SessionId(session_id.to_owned()),
        cwd: extra
            .get("cwd")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .into(),
        model: extra
            .get("model")
            .and_then(Value::as_str)
            .map(str::to_owned),
        tools: extra
            .get("tools")
            .and_then(Value::as_array)
            .map(|tools| {
                tools
                    .iter()
                    .filter_map(Value::as_str)
                    .map(str::to_owned)
                    .collect()
            })
            .unwrap_or_default(),
    }
}

pub fn block_id(message: &str, index: usize) -> ItemId {
    ItemId(format!("{message}#{index}"))
}

pub fn text_at(delta: Option<&Value>, field: &str) -> String {
    delta
        .and_then(|delta| delta.get(field))
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_owned()
}

pub fn tool_results(message: &Message) -> Vec<op_agent::AgentEvent> {
    message
        .content
        .blocks()
        .iter()
        .filter_map(|block| {
            let ContentBlock::Known(KnownContentBlock::ToolResult {
                tool_use_id,
                content,
                is_error,
                ..
            }) = block
            else {
                return None;
            };
            Some(op_agent::AgentEvent::ToolEnded(ToolOutcome {
                item: ItemId(tool_use_id.clone()),
                output: flatten_content(content.clone().flatten().as_ref()),
                failed: is_error.flatten().unwrap_or(false),
            }))
        })
        .collect()
}

// A tool result is a bare string on one call and a block list on the next.
fn flatten_content(content: Option<&Value>) -> String {
    match content {
        None | Some(Value::Null) => String::new(),
        Some(Value::String(text)) => text.clone(),
        Some(Value::Array(blocks)) => blocks
            .iter()
            .filter_map(|block| block.get("text").and_then(Value::as_str))
            .collect::<Vec<_>>()
            .join("\n"),
        Some(other) => other.to_string(),
    }
}

pub fn approval(request_id: &str, request: &Value) -> Option<ApprovalRequest> {
    if request.get("subtype").and_then(Value::as_str) != Some("can_use_tool") {
        return None;
    }
    Some(ApprovalRequest {
        id: ApprovalId(request_id.to_owned()),
        item: request
            .get("tool_use_id")
            .and_then(Value::as_str)
            .map(|id| ItemId(id.to_owned())),
        tool: request
            .get("tool_name")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned(),
        input: request.get("input").cloned().unwrap_or(Value::Null),
        reason: request
            .get("reason")
            .or_else(|| request.get("blocked_path"))
            .and_then(Value::as_str)
            .map(str::to_owned),
    })
}

pub fn interrupt() -> Value {
    json!({ "subtype": "interrupt" })
}

pub fn control_response(
    id: &ApprovalId,
    decision: &ApprovalDecision,
    request: Option<&Value>,
) -> StreamInput {
    let input = request
        .and_then(|request| request.get("input"))
        .cloned()
        .unwrap_or(Value::Null);
    let outcome = match decision {
        ApprovalDecision::Allow => json!({ "behavior": "allow", "updatedInput": input }),
        ApprovalDecision::AllowForSession => {
            let mut allow = json!({ "behavior": "allow", "updatedInput": input });
            // The CLI writes the rules that would stop it asking again; sending them back is the
            // only way to widen the session's permissions from here.
            if let Some(rules) = request.and_then(|request| request.get("permission_suggestions")) {
                allow["updatedPermissions"] = rules.clone();
            }
            allow
        }
        ApprovalDecision::Deny { reason } => json!({
            "behavior": "deny",
            "message": reason.clone().unwrap_or_else(|| "the user refused".to_owned()),
        }),
        ApprovalDecision::Abort => json!({
            "behavior": "deny",
            "message": "the user stopped the turn",
        }),
    };
    StreamInput::Known(Box::new(KnownStreamInput::ControlResponse {
        response: json!({
            "subtype": "success",
            "request_id": id.0,
            "response": outcome,
        }),
        extra: Extra::new(),
    }))
}

pub fn rate_limit(info: &Value) -> RateLimit {
    let windows = info
        .get("unifiedWindows")
        .and_then(Value::as_object)
        .map(|windows| {
            windows
                .iter()
                .map(|(name, window)| RateLimitWindow {
                    name: name.clone(),
                    used_percent: window
                        .get("utilization")
                        .and_then(Value::as_f64)
                        .unwrap_or_default()
                        * 100.0,
                    resets_at: window.get("resetsAt").and_then(Value::as_i64),
                })
                .collect()
        })
        .unwrap_or_default();
    RateLimit { windows }
}

pub fn turn_usage(usage: &Value, cost_usd: Option<f64>) -> Usage {
    let count = |field: &str| usage.get(field).and_then(Value::as_u64).unwrap_or_default();
    let cached_input_tokens = count("cache_read_input_tokens");
    Usage {
        input_tokens: count("input_tokens") + cached_input_tokens,
        cached_input_tokens,
        cache_write_tokens: count("cache_creation_input_tokens"),
        output_tokens: count("output_tokens"),
        reasoning_tokens: usage
            .pointer("/output_tokens_details/thinking_tokens")
            .and_then(Value::as_u64)
            .unwrap_or_default(),
        cost_usd,
    }
}
