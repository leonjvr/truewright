pub mod llm_stub;

/// Builds a scripted chat-completion response carrying one or more tool
/// calls, in the OpenAI wire shape `LlmStub` returns verbatim.
pub fn tool_call_response(calls: &[(&str, &str, serde_json::Value)]) -> serde_json::Value {
    let tool_calls: Vec<_> = calls
        .iter()
        .map(|(id, name, args)| {
            serde_json::json!({
                "id": id,
                "type": "function",
                "function": { "name": name, "arguments": args.to_string() }
            })
        })
        .collect();
    serde_json::json!({
        "choices": [{
            "message": { "role": "assistant", "content": null, "tool_calls": tool_calls },
            "finish_reason": "tool_calls"
        }]
    })
}

/// A scripted response with plain text content and no tool calls -- used
/// as `LlmStub`'s canned vision-interpretation reply.
pub fn text_only_response(text: &str) -> serde_json::Value {
    serde_json::json!({
        "choices": [{
            "message": { "role": "assistant", "content": text },
            "finish_reason": "stop"
        }]
    })
}
