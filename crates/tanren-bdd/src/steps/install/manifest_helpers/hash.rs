use tanren_testkit::sha256_hex_string as contract_sha256_hex_string;

pub(crate) fn sha256_hex_string(bytes: &[u8]) -> String {
    contract_sha256_hex_string(bytes)
}
