use tanren_cli_app::install::manifest::sha256_hex;

pub(crate) fn sha256_hex_string(bytes: &[u8]) -> String {
    sha256_hex(bytes)
}
