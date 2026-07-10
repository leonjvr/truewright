use crate::session::Command;
use serde::Deserialize;

pub struct GetVersion;
impl Command for GetVersion {
    const METHOD: &'static str = "Browser.getVersion";
    type Params = super::EmptyParams;
    type Response = GetVersionResponse;
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GetVersionResponse {
    pub protocol_version: String,
    pub product: String,
    pub revision: String,
    pub user_agent: String,
    pub js_version: String,
}
