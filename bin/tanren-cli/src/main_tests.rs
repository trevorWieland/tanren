use std::io::{Error as IoError, ErrorKind as IoErrorKind, Read};

use clap::{CommandFactory, Parser};
use tanren_app_services::auth::DEFAULT_ACTOR_TOKEN_MAX_BYTES;
use tanren_contract::{CliParseReasonCode, ErrorCode, ErrorDetails};

use super::actor_token::read_actor_token_from_reader;
use super::clap_error::{ALLOWED_ARG_FIELDS, clap_error_to_response};
use super::{Cli, into_error_response};

struct FailingReader;

impl Read for FailingReader {
    fn read(&mut self, _buf: &mut [u8]) -> std::io::Result<usize> {
        Err(IoError::new(
            IoErrorKind::PermissionDenied,
            "redacted-test-io-detail",
        ))
    }
}

#[test]
fn actor_token_stdin_failure_is_generic_and_redacted() {
    let mut reader = FailingReader;
    let err = read_actor_token_from_reader(&mut reader).expect_err("read should fail");
    let response = into_error_response(err);
    assert_eq!(response.code, ErrorCode::InvalidInput);
    assert!(response.message.contains("invalid actor token source"));
    assert!(!response.message.contains("stdin"));
    assert!(!response.message.contains("PermissionDenied"));
    assert!(!response.message.contains("redacted-test-io-detail"));
}

#[test]
fn actor_token_stdin_overflow_is_generic_and_invalid_input() {
    let oversized = "x".repeat(DEFAULT_ACTOR_TOKEN_MAX_BYTES + 1);
    let mut reader = std::io::Cursor::new(oversized);
    let err = read_actor_token_from_reader(&mut reader).expect_err("oversized input");
    let response = into_error_response(err);
    assert_eq!(response.code, ErrorCode::InvalidInput);
    assert!(response.message.contains("invalid actor token source"));
}

#[test]
fn allowed_arg_fields_covers_every_declared_long_flag() {
    use std::collections::BTreeSet;

    fn collect_longs(cmd: &clap::Command, acc: &mut BTreeSet<String>) {
        for arg in cmd.get_arguments() {
            if let Some(long) = arg.get_long() {
                acc.insert(long.replace('-', "_"));
            }
        }
        for sub in cmd.get_subcommands() {
            collect_longs(sub, acc);
        }
    }

    let mut declared = BTreeSet::new();
    collect_longs(&Cli::command(), &mut declared);

    let allowlist: BTreeSet<String> = ALLOWED_ARG_FIELDS.iter().map(|s| (*s).to_owned()).collect();

    for long in &declared {
        assert!(
            allowlist.contains(long),
            "declared flag --{long} is not listed in ALLOWED_ARG_FIELDS"
        );
    }
}

#[test]
fn missing_required_argument_maps_to_safe_wire_response() {
    let err = Cli::try_parse_from(["tanren", "dispatch", "create"])
        .expect_err("missing --project must fail");
    let response = clap_error_to_response(&err);
    assert_eq!(response.code, ErrorCode::InvalidInput);
    assert_eq!(response.message, "invalid cli args");
    assert!(
        matches!(
            &response.details,
            Some(ErrorDetails::InvalidArgs { reason_code, .. })
                if *reason_code == CliParseReasonCode::MissingRequiredArgument
        ),
        "expected missing_required_argument details, got {:?}",
        response.details
    );
}

#[test]
fn invalid_value_does_not_echo_user_input_on_wire() {
    let err = Cli::try_parse_from([
        "tanren",
        "dispatch",
        "create",
        "--project",
        "p",
        "--phase",
        "sk-super-secret-value",
        "--cli",
        "claude",
        "--branch",
        "b",
        "--spec-folder",
        "s",
        "--workflow-id",
        "w",
    ])
    .expect_err("invalid --phase value must fail");
    let response = clap_error_to_response(&err);
    let json = serde_json::to_string(&response).expect("serialize");
    assert!(
        !json.contains("sk-super-secret-value"),
        "raw user value leaked into wire: {json}"
    );
    assert!(
        !json.contains("super-secret"),
        "raw user value leaked into wire: {json}"
    );
    assert_eq!(response.code, ErrorCode::InvalidInput);
    assert_eq!(response.message, "invalid cli args");
}

#[test]
fn unknown_argument_with_secret_value_is_not_echoed() {
    let err = Cli::try_parse_from(["tanren", "dispatch", "list", "--secret-value=sk-1234"])
        .expect_err("unknown --secret-value must fail");
    let response = clap_error_to_response(&err);
    let json = serde_json::to_string(&response).expect("serialize");
    assert!(!json.contains("sk-1234"), "raw secret leaked: {json}");
    assert!(
        !json.contains("secret-value"),
        "unknown flag name not allowlisted must not reach wire: {json}"
    );
    assert_eq!(response.code, ErrorCode::InvalidInput);
    assert_eq!(response.message, "invalid cli args");
}
