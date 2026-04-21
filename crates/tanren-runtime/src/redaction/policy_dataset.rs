pub(crate) const DEFAULT_REDACTION_POLICY_DATASET_VERSION: &str = "2026-04-21.v1";

pub(crate) struct RedactionPolicyDatasetV1 {
    pub(crate) min_token_len: usize,
    pub(crate) min_secret_fragment_len: usize,
    pub(crate) max_persistable_channel_bytes: usize,
    pub(crate) sensitive_key_names: &'static [&'static str],
    pub(crate) token_prefixes: &'static [&'static str],
}

pub(crate) fn default_policy_dataset_v1() -> RedactionPolicyDatasetV1 {
    RedactionPolicyDatasetV1 {
        min_token_len: 10,
        min_secret_fragment_len: 4,
        max_persistable_channel_bytes: 512 * 1024,
        sensitive_key_names: &[
            "api_key",
            "api-token",
            "api_token",
            "auth_token",
            "access_token",
            "refresh_token",
            "session_token",
            "authorization",
            "bearer",
            "cookie",
            "set-cookie",
            "password",
            "secret",
            "secret_key",
            "private_key",
            "client_secret",
            "id_token",
            "personal_access_token",
            "aws_access_key_id",
            "aws_secret_access_key",
            "x-api-key",
        ],
        token_prefixes: &[
            "sk-",
            "sk-proj-",
            "sk-ant-",
            "ghp_",
            "gho_",
            "ghu_",
            "ghs_",
            "github_pat_",
            "xoxb-",
            "xoxp-",
            "xoxa-",
            "xoxr-",
            "AKIA",
            "ASIA",
            "AIza",
            "ya29.",
        ],
    }
}
