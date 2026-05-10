use regex::Regex;
use tokio::io::AsyncWriteExt;

pub(super) async fn write_sign_in_flow(
    stdin: &mut tokio::process::ChildStdin,
    email: &str,
    password: &str,
) -> std::io::Result<()> {
    stdin.write_all(b"\x1b[B\r").await?; // Menu -> Sign in
    stdin.write_all(email.as_bytes()).await?;
    stdin.write_all(b"\t").await?;
    stdin.write_all(password.as_bytes()).await?;
    stdin.write_all(b"\r").await?; // Submit sign-in form
    Ok(())
}

pub(super) async fn write_outcome_continue(
    stdin: &mut tokio::process::ChildStdin,
) -> std::io::Result<()> {
    stdin.write_all(b"\r").await?; // Outcome -> back to menu
    Ok(())
}

pub(super) async fn write_posture_set_flow(
    stdin: &mut tokio::process::ChildStdin,
    posture_raw: &str,
) -> std::io::Result<()> {
    stdin.write_all(b"\x1b[B\x1b[B\x1b[B\r").await?; // Menu -> Deployment posture
    for _ in 0..24 {
        stdin.write_all(b"\x08").await?; // clear default "hosted"
    }
    stdin.write_all(posture_raw.as_bytes()).await?;
    stdin.write_all(b"\r").await?; // Submit posture form
    Ok(())
}

pub(super) fn extract_posture_failure(transcript: &str) -> Option<(String, String)> {
    let code_regex = Regex::new(
        r"(unsupported_posture|permission_denied|scope_not_found|validation_failed|internal_error):\s*([^\r\n]+)",
    )
    .expect("constant regex should compile");
    let mut last: Option<(String, String)> = None;
    for captures in code_regex.captures_iter(transcript) {
        let code = captures.get(1).map_or("", |m| m.as_str()).to_owned();
        let summary = captures.get(2).map_or("", |m| m.as_str()).trim().to_owned();
        last = Some((code, summary));
    }
    last
}

pub(super) fn normalize_transcript(transcript: &str) -> String {
    let ansi_regex = Regex::new(r"\x1B\[[0-?]*[ -/]*[@-~]").expect("constant regex should compile");
    ansi_regex.replace_all(transcript, "").replace('\r', "\n")
}
