//! Screenshot interpretation via a dedicated vision role (agent-harness
//! spec: "Vision routing"). Used when the driver role lacks vision
//! (`RoleClient.vision == false`), and directly by MCP's
//! `browser_screenshot(interpret: true)` (mcp-task-delegation).

use crate::error::Result;
use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use llm::{ChatRequest, Message, Part, RoleClient};

const DEFAULT_VISION_GUIDANCE: &str =
    "Describe the page layout, visible text, interactive elements, and anything anomalous.";

/// Sends a screenshot to `vision_role` with `guidance` (falling back to a
/// built-in default) and returns its text interpretation.
pub async fn interpret_screenshot(
    vision_role: &RoleClient,
    png_bytes: &[u8],
    guidance: Option<&str>,
) -> Result<String> {
    let b64 = STANDARD.encode(png_bytes);
    let guidance_text = guidance
        .filter(|g| !g.is_empty())
        .unwrap_or(DEFAULT_VISION_GUIDANCE);

    let req = ChatRequest {
        model: String::new(), // RoleClient::complete fills in the configured model
        messages: vec![
            Message::system(
                "You interpret browser screenshots for an agent that cannot see images itself. \
                 Be concise, factual, and specific about what's actually visible -- don't guess \
                 at content you can't read clearly.",
            ),
            Message::user_with_image(guidance_text, Part::ImagePngB64(b64)),
        ],
        tools: vec![],
    };

    let resp = vision_role.complete(req).await?;
    Ok(resp.message.text())
}
