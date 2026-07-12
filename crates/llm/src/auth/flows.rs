//! OAuth flow descriptors -- data, not code, so a new provider is a new
//! struct literal (oauth-subscription-auth spec: "Flow registry"). Values
//! for `"chatgpt"` are verified against OpenAI's own `codex-rs/login`
//! source (not third-party writeups) as of this change's implementation;
//! they're deliberately not hardcoded as the *only* option -- see
//! `Config`'s `[oauth.<flow>]` override table -- since this is unofficial
//! surface OpenAI could change at any time.

pub struct OAuthFlowSpec {
    pub id: &'static str,
    pub authorize_url: &'static str,
    pub token_url: &'static str,
    pub client_id: &'static str,
    pub scope: &'static str,
    /// Extra, non-standard query params this flow's authorize request
    /// needs (name, value) -- e.g. Codex's `codex_cli_simplified_flow`.
    pub extra_authorize_params: &'static [(&'static str, &'static str)],
    /// Preferred local callback port, with one fallback if it's taken
    /// (matches Codex CLI's own 1455 -> 1457 fallback, since this flow's
    /// `redirect_uri` allow-list on OpenAI's side is presumably scoped to
    /// exactly those two ports).
    pub redirect_port: u16,
    pub redirect_port_fallback: u16,
    pub redirect_path: &'static str,
    /// `Content-Type` the *initial* code exchange uses. Codex's own
    /// refresh call uses JSON while the initial exchange uses form
    /// encoding -- a real, easy-to-miss asymmetry, not a copy-paste typo,
    /// confirmed by reading both call sites in Codex's own source.
    pub token_exchange_is_form_encoded: bool,
}

pub const CHATGPT: OAuthFlowSpec = OAuthFlowSpec {
    id: "chatgpt",
    authorize_url: "https://auth.openai.com/oauth/authorize",
    token_url: "https://auth.openai.com/oauth/token",
    client_id: "app_EMoamEEZ73f0CkXaXp7hrann",
    scope: "openid profile email offline_access api.connectors.read api.connectors.invoke",
    extra_authorize_params: &[
        ("id_token_add_organizations", "true"),
        ("codex_cli_simplified_flow", "true"),
    ],
    redirect_port: 1455,
    redirect_port_fallback: 1457,
    redirect_path: "/auth/callback",
    token_exchange_is_form_encoded: true,
};

pub fn flow(id: &str) -> Option<&'static OAuthFlowSpec> {
    match id {
        "chatgpt" => Some(&CHATGPT),
        _ => None,
    }
}
