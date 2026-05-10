use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use tanren_identity_policy::{AccountId, SessionToken};

const SESSION_FILE_VERSION: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub(super) struct TuiSessionFile {
    #[serde(default = "session_file_version")]
    pub(super) version: u32,
    #[serde(default)]
    pub(super) signed_in: Vec<SignedInSession>,
    #[serde(default)]
    pub(super) active_account_by_window: BTreeMap<String, AccountId>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub(super) struct SignedInSession {
    pub(super) account_id: AccountId,
    pub(super) token: SessionToken,
}

#[derive(Debug, Clone)]
pub(super) struct ActiveSession {
    pub(super) account_id: AccountId,
    pub(super) has_token: bool,
}

fn session_file_version() -> u32 {
    SESSION_FILE_VERSION
}
