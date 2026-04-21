pub(crate) const DEFAULT_REDACTION_POLICY_DATASET_VERSION: &str = "2026-04-21.v2";
#[cfg(test)]
pub(crate) const DEFAULT_REDACTION_POLICY_DATASET_PROVENANCE: &str =
    "owner=tanren-runtime;source=curated_phase1_minimum;change_control=version_bump_required";
#[cfg(test)]
pub(crate) const DEFAULT_REDACTION_POLICY_DATASET_CHECKSUM_FNV64: u64 = 5_592_519_109_534_153_128;

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

#[cfg(test)]
pub(crate) fn canonical_policy_dataset_snapshot_v1() -> String {
    let dataset = default_policy_dataset_v1();
    let mut lines = Vec::new();
    lines.push(format!(
        "version={DEFAULT_REDACTION_POLICY_DATASET_VERSION}"
    ));
    lines.push(format!(
        "provenance={DEFAULT_REDACTION_POLICY_DATASET_PROVENANCE}"
    ));
    lines.push(format!("min_token_len={}", dataset.min_token_len));
    lines.push(format!(
        "min_secret_fragment_len={}",
        dataset.min_secret_fragment_len
    ));
    lines.push(format!(
        "max_persistable_channel_bytes={}",
        dataset.max_persistable_channel_bytes
    ));
    for key in dataset.sensitive_key_names {
        lines.push(format!("sensitive_key={key}"));
    }
    for prefix in dataset.token_prefixes {
        lines.push(format!("token_prefix={prefix}"));
    }
    lines.join("\n")
}

#[cfg(test)]
pub(crate) fn policy_dataset_checksum_fnv64(snapshot: &str) -> u64 {
    const FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
    const FNV_PRIME: u64 = 0x0100_0000_01b3;

    let mut hash = FNV_OFFSET;
    for byte in snapshot.as_bytes() {
        hash ^= u64::from(*byte);
        hash = hash.wrapping_mul(FNV_PRIME);
    }
    hash
}
