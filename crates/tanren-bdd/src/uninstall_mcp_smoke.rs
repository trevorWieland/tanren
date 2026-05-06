use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};

use chrono::Utc;
use rmcp::RoleClient;
use rmcp::ServiceExt;
use rmcp::model::{CallToolRequestParams, CallToolResult, ClientInfo, RawContent};
use rmcp::service::RunningService;
use rmcp::transport::StreamableHttpClientTransport;
use rmcp::transport::streamable_http_client::StreamableHttpClientTransportConfig;
use secrecy::SecretString;
use sha2::{Digest, Sha256};
use tanren_app_services::Store;
use tanren_contract::{FileOwnership, InstallManifest, ManifestEntry};
use tokio::net::TcpListener;

static COUNTER: AtomicUsize = AtomicUsize::new(0);

struct TempRepo {
    path: PathBuf,
}

impl TempRepo {
    fn new(label: &str) -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("tanren-mcp-uninstall-test-{label}-{id}"));
        let _ = fs::remove_dir_all(&path);
        fs::create_dir_all(&path).expect("create temp repo");
        Self { path }
    }

    fn path(&self) -> &Path {
        &self.path
    }

    fn write_file(&self, rel: &str, content: &[u8]) {
        let full = self.path.join(rel);
        if let Some(parent) = full.parent() {
            let _ = fs::create_dir_all(parent);
        }
        fs::write(full, content).expect("write file");
    }

    fn file_exists(&self, rel: &str) -> bool {
        self.path.join(rel).exists()
    }

    fn read_file(&self, rel: &str) -> String {
        fs::read_to_string(self.path.join(rel)).expect("read file")
    }

    fn write_manifest(&self, manifest: &InstallManifest) {
        let dir = self.path.join(".tanren");
        fs::create_dir_all(&dir).expect("create .tanren dir");
        let json = serde_json::to_string_pretty(manifest).expect("serialize manifest");
        fs::write(dir.join("install-manifest.json"), json).expect("write manifest");
    }
}

impl Drop for TempRepo {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.path);
    }
}

fn sha256_hex(content: &[u8]) -> String {
    let digest = Sha256::digest(content);
    let mut hex = String::with_capacity(digest.len() * 2);
    for byte in digest.as_slice() {
        use std::fmt::Write;
        let _ = write!(hex, "{byte:02x}");
    }
    hex
}

struct McpTestServer {
    client: RunningService<RoleClient, ClientInfo>,
    server: Option<tokio::task::JoinHandle<()>>,
    db_path: PathBuf,
}

impl McpTestServer {
    async fn spawn() -> Self {
        let id = COUNTER.fetch_add(1, Ordering::Relaxed);
        let db_path = std::env::temp_dir().join(format!("tanren-mcp-uninstall-test-db-{id}"));
        let database_url = format!("sqlite://{}?mode=rwc", db_path.display());

        let store = Store::connect(&database_url).await.expect("connect store");
        store.migrate().await.expect("migrate store");
        let store = Arc::new(store);

        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind listener");
        let local_addr = listener.local_addr().expect("local addr");

        let (router, cancellation) = tanren_mcp_app::build_router_with_store(
            store,
            SecretString::from("bdd-test-key".to_owned()),
        );

        let server = tokio::spawn(async move {
            let _ = axum::serve(listener, router)
                .with_graceful_shutdown(async move { cancellation.cancelled_owned().await })
                .await;
        });

        let config =
            StreamableHttpClientTransportConfig::with_uri(format!("http://{local_addr}/mcp"))
                .auth_header("bdd-test-key".to_owned());
        let transport = StreamableHttpClientTransport::with_client(reqwest::Client::new(), config);
        let client = ClientInfo::default()
            .serve(transport)
            .await
            .expect("rmcp client handshake");

        Self {
            client,
            server: Some(server),
            db_path,
        }
    }

    async fn call_tool(&self, name: &'static str, body: serde_json::Value) -> CallToolResult {
        let args = body
            .as_object()
            .expect("tool args must be a JSON object")
            .clone();
        self.client
            .call_tool(CallToolRequestParams::new(name).with_arguments(args))
            .await
            .expect("call tool")
    }
}

impl Drop for McpTestServer {
    fn drop(&mut self) {
        if let Some(handle) = self.server.take() {
            handle.abort();
        }
        let _ = fs::remove_file(&self.db_path);
    }
}

fn text_payload(result: &CallToolResult) -> serde_json::Value {
    let text = result
        .content
        .iter()
        .find_map(|item| {
            if let RawContent::Text(text) = &item.raw {
                Some(text.text.clone())
            } else {
                None
            }
        })
        .expect("tool result must contain text content");
    serde_json::from_str(&text).expect("decode tool result")
}

#[tokio::test]
async fn mcp_uninstall_preview_returns_data_without_deleting_files() {
    let mcp = McpTestServer::spawn().await;
    let repo = TempRepo::new("mcp-preview");

    let generated_content = b"generated by tanren\n";
    let generated_hash = sha256_hex(generated_content);
    repo.write_file("generated.txt", generated_content);
    repo.write_file("user-spec.md", b"my spec content\n");

    repo.write_manifest(&InstallManifest {
        version: 1,
        entries: vec![
            ManifestEntry {
                path: "generated.txt".into(),
                ownership: FileOwnership::TanrenGenerated,
                content_hash: generated_hash,
                generated_at: Utc::now(),
            },
            ManifestEntry {
                path: "user-spec.md".into(),
                ownership: FileOwnership::UserOwned,
                content_hash: sha256_hex(b"my spec content\n"),
                generated_at: Utc::now(),
            },
        ],
        created_at: Utc::now(),
    });

    let result = mcp
        .call_tool(
            "project.uninstall_preview",
            serde_json::json!({
                "repo_path": repo.path().display().to_string()
            }),
        )
        .await;

    assert!(
        result.is_error != Some(true),
        "preview should succeed, got error: {:?}",
        text_payload(&result)
    );

    let body = text_payload(&result);

    let to_remove = body["to_remove"].as_array().expect("to_remove array");
    assert!(!to_remove.is_empty(), "preview should list files to remove");

    let preserved = body["preserved"].as_array().expect("preserved array");
    assert!(
        !preserved.is_empty(),
        "preview should list files to preserve"
    );

    assert_eq!(
        body["hosted_account_unchanged"], true,
        "hosted_account_unchanged must be true"
    );

    assert!(
        repo.file_exists("generated.txt"),
        "generated.txt must still exist after preview"
    );
    assert!(
        repo.file_exists("user-spec.md"),
        "user-spec.md must still exist after preview"
    );
    assert!(
        repo.file_exists(".tanren/install-manifest.json"),
        "manifest must still exist after preview"
    );
}

#[tokio::test]
async fn mcp_uninstall_apply_without_confirmation_fails_and_leaves_files() {
    let mcp = McpTestServer::spawn().await;
    let repo = TempRepo::new("mcp-no-confirm");

    let generated_content = b"generated by tanren\n";
    let generated_hash = sha256_hex(generated_content);
    repo.write_file("generated.txt", generated_content);

    repo.write_manifest(&InstallManifest {
        version: 1,
        entries: vec![ManifestEntry {
            path: "generated.txt".into(),
            ownership: FileOwnership::TanrenGenerated,
            content_hash: generated_hash,
            generated_at: Utc::now(),
        }],
        created_at: Utc::now(),
    });

    let result = mcp
        .call_tool(
            "project.uninstall_apply",
            serde_json::json!({
                "repo_path": repo.path().display().to_string(),
                "confirm": false
            }),
        )
        .await;

    assert_eq!(
        result.is_error,
        Some(true),
        "apply without confirm should be an error"
    );

    let body = text_payload(&result);
    assert_eq!(
        body["code"], "confirmation_required",
        "expected confirmation_required, got {}",
        body["code"]
    );

    assert!(
        repo.file_exists("generated.txt"),
        "generated.txt must still exist after rejected apply"
    );
    assert!(
        repo.file_exists(".tanren/install-manifest.json"),
        "manifest must still exist after rejected apply"
    );
}

#[tokio::test]
async fn mcp_uninstall_confirmed_apply_removes_generated_preserves_user_files() {
    let mcp = McpTestServer::spawn().await;
    let repo = TempRepo::new("mcp-confirmed-apply");

    let unchanged_content = b"generated by tanren\n";
    let unchanged_hash = sha256_hex(unchanged_content);
    repo.write_file("generated.txt", unchanged_content);

    let original_standard = b"original standard\n";
    let original_hash = sha256_hex(original_standard);
    repo.write_file("standard.md", b"edited standard content\n");

    let user_content = b"my important spec\n";
    let user_hash = sha256_hex(user_content);
    repo.write_file("user-spec.md", user_content);

    repo.write_manifest(&InstallManifest {
        version: 1,
        entries: vec![
            ManifestEntry {
                path: "generated.txt".into(),
                ownership: FileOwnership::TanrenGenerated,
                content_hash: unchanged_hash,
                generated_at: Utc::now(),
            },
            ManifestEntry {
                path: "standard.md".into(),
                ownership: FileOwnership::TanrenGenerated,
                content_hash: original_hash,
                generated_at: Utc::now(),
            },
            ManifestEntry {
                path: "user-spec.md".into(),
                ownership: FileOwnership::UserOwned,
                content_hash: user_hash,
                generated_at: Utc::now(),
            },
        ],
        created_at: Utc::now(),
    });

    let result = mcp
        .call_tool(
            "project.uninstall_apply",
            serde_json::json!({
                "repo_path": repo.path().display().to_string(),
                "confirm": true
            }),
        )
        .await;

    assert!(
        result.is_error != Some(true),
        "confirmed apply should succeed, got error: {:?}",
        text_payload(&result)
    );

    let body = text_payload(&result);

    assert_eq!(
        body["hosted_account_unchanged"], true,
        "hosted_account_unchanged must be true"
    );

    let removed = body["removed"].as_array().expect("removed array");
    assert!(
        !removed.is_empty(),
        "apply result should list removed files"
    );

    let preserved = body["preserved"].as_array().expect("preserved array");
    assert!(
        !preserved.is_empty(),
        "apply result should list preserved files"
    );

    assert!(
        !repo.file_exists("generated.txt"),
        "unchanged generated file must be removed"
    );
    assert!(
        repo.file_exists("standard.md"),
        "modified standard must be preserved"
    );
    assert_eq!(
        repo.read_file("standard.md"),
        "edited standard content\n",
        "modified standard content must be unchanged"
    );
    assert!(
        repo.file_exists("user-spec.md"),
        "user spec must be preserved"
    );
    assert_eq!(
        repo.read_file("user-spec.md"),
        "my important spec\n",
        "user spec content must be unchanged"
    );
    assert!(
        !repo.file_exists(".tanren/install-manifest.json"),
        "manifest must be removed"
    );
}
