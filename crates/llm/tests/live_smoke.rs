//! llm-providers spec: a real request against a real provider, exactly the
//! way an end user's config would drive it. Skipped (not failed) unless
//! `DEEPSEEK_API_KEY` is set -- same skip-if-unavailable convention this
//! project already uses for browser tests with no installed browser.

use llm::{ChatRequest, CompatClient, CredentialSource, Message};
use std::collections::BTreeMap;

#[tokio::test]
async fn deepseek_answers_a_trivial_prompt() {
    let Ok(api_key) = std::env::var("DEEPSEEK_API_KEY") else {
        eprintln!("skipping deepseek_answers_a_trivial_prompt: DEEPSEEK_API_KEY not set");
        return;
    };

    let client = CompatClient::new(
        "https://api.deepseek.com/v1".to_string(),
        CredentialSource::Static(api_key),
        BTreeMap::new(),
    );

    let req = ChatRequest {
        model: "deepseek-chat".to_string(),
        messages: vec![
            Message::system("You are a terse connectivity test. Reply with exactly one word."),
            Message::user("Reply with exactly: pong"),
        ],
        tools: vec![],
    };

    let resp = client
        .complete(&req)
        .await
        .expect("live request to DeepSeek succeeds");
    let text = resp.message.text();
    assert!(
        text.to_lowercase().contains("pong"),
        "expected a reply containing 'pong', got: {text:?}"
    );
}
