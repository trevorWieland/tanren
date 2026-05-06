use std::io::Write;

fn main() -> anyhow::Result<()> {
    let spec = tanren_api_app::openapi_spec();
    let json = serde_json::to_string_pretty(&spec)?;
    let stdout = std::io::stdout();
    let mut handle = stdout.lock();
    writeln!(handle, "{json}")?;
    Ok(())
}
