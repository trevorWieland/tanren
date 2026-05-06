use std::collections::HashMap;
use std::hash::Hash;
use std::path::PathBuf;

use async_trait::async_trait;
use chrono::Utc;
use tanren_app_services::ActorContext;
use tanren_app_services::project::{ProjectDependencyView, ProjectSpecView};
use tanren_contract::{
    ConnectProjectRequest, ConnectProjectResponse, DisconnectProjectRequest,
    DisconnectProjectResponse, ListProjectsResponse, ProjectFailureReason, ProjectView,
    ReconnectProjectResponse,
};
use tanren_identity_policy::{AccountId, OrgId, ProjectId, SpecId};
use tanren_store::{EventEnvelope, ProjectRecord, ProjectStatus};

use super::in_process::InProcessHarness;
use super::{AccountHarness as _, HarnessError, HarnessKind, HarnessResult};

#[async_trait]
pub trait ProjectHarness: Send + std::fmt::Debug {
    fn kind(&self) -> HarnessKind;
    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse>;
    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse>;
    async fn list_projects(&mut self, account_id: AccountId)
    -> HarnessResult<ListProjectsResponse>;
    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse>;
    async fn project_specs(&mut self, project_id: ProjectId)
    -> HarnessResult<Vec<ProjectSpecView>>;
    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>>;
    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)>;
    async fn seed_spec(&mut self, project_id: ProjectId, title: String) -> HarnessResult<SpecId>;
    async fn seed_dependency(
        &mut self,
        source: ProjectId,
        source_spec: SpecId,
        target: ProjectId,
    ) -> HarnessResult<()>;
    async fn seed_active_loop(&mut self, project_id: ProjectId) -> HarnessResult<()>;
    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>>;
}

pub struct ProjectInProcessHarness {
    inner: InProcessHarness,
    account_id: Option<AccountId>,
}

impl std::fmt::Debug for ProjectInProcessHarness {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ProjectInProcessHarness")
            .finish_non_exhaustive()
    }
}

impl ProjectInProcessHarness {
    pub async fn new(kind: HarnessKind) -> HarnessResult<Self> {
        Ok(Self {
            inner: InProcessHarness::new(kind).await?,
            account_id: None,
        })
    }
}

fn make_actor(account_id: AccountId) -> ActorContext {
    ActorContext::from_account_id(account_id)
}

#[async_trait]
impl ProjectHarness for ProjectInProcessHarness {
    fn kind(&self) -> HarnessKind {
        self.inner.kind()
    }

    async fn connect_project(
        &mut self,
        req: ConnectProjectRequest,
    ) -> HarnessResult<ConnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .connect_project(self.inner.store(), &actor, req)
            .await
            .map_err(translate_project_error)
    }

    async fn disconnect_project(
        &mut self,
        req: DisconnectProjectRequest,
    ) -> HarnessResult<DisconnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .disconnect_project(self.inner.store(), &actor, req)
            .await
            .map_err(translate_project_error)
    }

    async fn list_projects(
        &mut self,
        account_id: AccountId,
    ) -> HarnessResult<ListProjectsResponse> {
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .list_projects(self.inner.store(), &actor)
            .await
            .map_err(translate_project_error)
    }

    async fn reconnect_project(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<ReconnectProjectResponse> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .reconnect_project(self.inner.store(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn project_specs(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectSpecView>> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .project_specs(self.inner.store(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn project_dependencies(
        &mut self,
        project_id: ProjectId,
    ) -> HarnessResult<Vec<ProjectDependencyView>> {
        let account_id = self.account_id.expect("seed_account must be called first");
        let actor = make_actor(account_id);
        self.inner
            .handlers()
            .project_dependencies(self.inner.store(), &actor, project_id)
            .await
            .map_err(translate_project_error)
    }

    async fn seed_account(&mut self) -> HarnessResult<(AccountId, OrgId)> {
        let aid = AccountId::fresh();
        let oid = OrgId::fresh();
        self.inner
            .store_mut()
            .seed_account_with_org(aid, oid, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_account: {e}")))?;
        self.account_id = Some(aid);
        Ok((aid, oid))
    }

    async fn seed_spec(&mut self, project_id: ProjectId, title: String) -> HarnessResult<SpecId> {
        let sid = SpecId::fresh();
        self.inner
            .store_mut()
            .seed_project_spec(sid, project_id, title, String::new(), Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
        Ok(sid)
    }

    async fn seed_dependency(
        &mut self,
        s: ProjectId,
        ss: SpecId,
        t: ProjectId,
    ) -> HarnessResult<()> {
        self.inner
            .store_mut()
            .seed_project_dependency(s, ss, t, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_dependency: {e}")))?;
        Ok(())
    }

    async fn seed_active_loop(&mut self, project_id: ProjectId) -> HarnessResult<()> {
        self.inner
            .store_mut()
            .seed_loop_fixture(project_id, true, Utc::now())
            .await
            .map_err(|e| HarnessError::Transport(format!("seed_active_loop: {e}")))?;
        Ok(())
    }

    async fn recent_events(&self, limit: u64) -> HarnessResult<Vec<EventEnvelope>> {
        use tanren_store::AccountStore as _;
        self.inner
            .store()
            .recent_events(limit)
            .await
            .map_err(|e| HarnessError::Transport(format!("recent_events: {e}")))
    }
}

#[derive(Debug, Clone)]
pub enum ProjectOutcome {
    Connected(ProjectView),
    Disconnected(DisconnectProjectResponse),
    Listed(ListProjectsResponse),
    Reconnected(ProjectView),
    Failure(ProjectFailureReason),
    Other(String),
}

impl ProjectOutcome {
    #[must_use]
    pub fn failure_code(&self) -> Option<String> {
        match self {
            Self::Failure(reason) => Some(reason.code().to_owned()),
            Self::Connected(_)
            | Self::Disconnected(_)
            | Self::Listed(_)
            | Self::Reconnected(_)
            | Self::Other(_) => None,
        }
    }
}

pub fn record_project_failure(
    err: HarnessError,
    last_failure: &mut Option<ProjectFailureReason>,
) -> ProjectOutcome {
    match err {
        HarnessError::Project(reason, _) => {
            *last_failure = Some(reason);
            ProjectOutcome::Failure(reason)
        }
        HarnessError::Account(reason, _) => {
            ProjectOutcome::Other(format!("account: {}", reason.code()))
        }
        HarnessError::Transport(message) => ProjectOutcome::Other(format!("transport: {message}")),
    }
}

#[derive(Debug, Default)]
pub struct ProjectWorldState {
    pub account_id: Option<AccountId>,
    pub org_id: Option<OrgId>,
    pub projects: HashMap<String, ProjectId>,
    pub spec_titles: HashMap<String, Vec<String>>,
    pub checksums: HashMap<String, String>,
    pub last_outcome: Option<ProjectOutcome>,
    pub last_failure: Option<ProjectFailureReason>,
}

#[derive(Debug)]
pub struct RepositoryFixture {
    pub path: PathBuf,
}

impl RepositoryFixture {
    pub fn create(name: &str) -> HarnessResult<Self> {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "tanren-repo-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::create_dir_all(&path)
            .map_err(|e| HarnessError::Transport(format!("create repo: {e}")))?;
        std::fs::write(path.join("README.md"), format!("# {name}\n"))
            .map_err(|e| HarnessError::Transport(format!("write README: {e}")))?;
        Ok(Self { path })
    }

    pub fn url(&self) -> String {
        format!("file://{}", self.path.display())
    }

    pub fn checksum(&self) -> HarnessResult<String> {
        use std::hash::Hasher;
        use std::io::Read;
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        let mut entries: Vec<_> = walkdir_files(&self.path);
        entries.sort();
        for entry in &entries {
            entry.hash(&mut hasher);
            let mut buf = Vec::new();
            std::fs::File::open(entry)
                .and_then(|mut f| f.read_to_end(&mut buf))
                .map_err(|e| HarnessError::Transport(format!("read: {e}")))?;
            buf.hash(&mut hasher);
        }
        Ok(format!("{:016x}", hasher.finish()))
    }
}

impl Drop for RepositoryFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.path);
    }
}

fn walkdir_files(path: &std::path::Path) -> Vec<PathBuf> {
    let mut r = Vec::new();
    if let Ok(entries) = std::fs::read_dir(path) {
        for e in entries.flatten() {
            let p = e.path();
            if p.is_dir() {
                r.extend(walkdir_files(&p));
            } else {
                r.push(p);
            }
        }
    }
    r
}

pub(crate) fn record_to_view(record: &ProjectRecord) -> ProjectView {
    ProjectView {
        id: record.id,
        name: record.name.clone(),
        org_id: record.org_id,
        repository_url: record.repository_url.clone(),
        connected_at: record.connected_at,
        disconnected_at: match record.status {
            ProjectStatus::Disconnected(at) => Some(at),
            ProjectStatus::Connected => None,
        },
    }
}

pub(crate) fn translate_project_error(err: tanren_app_services::AppServiceError) -> HarnessError {
    use tanren_app_services::AppServiceError;
    match err {
        AppServiceError::Project(reason) => HarnessError::Project(reason, reason.code().to_owned()),
        AppServiceError::Account(reason) => HarnessError::Account(reason, reason.code().to_owned()),
        AppServiceError::InvalidInput(msg) => {
            HarnessError::Transport(format!("invalid_input: {msg}"))
        }
        AppServiceError::Store(e) => HarnessError::Transport(format!("store: {e}")),
        _ => HarnessError::Transport("unknown app-service failure".to_owned()),
    }
}

pub(crate) async fn seed_account_via_store(
    store: &tanren_app_services::Store,
) -> HarnessResult<(AccountId, OrgId)> {
    let aid = AccountId::fresh();
    let oid = OrgId::fresh();
    store
        .seed_account_with_org(aid, oid, Utc::now())
        .await
        .map_err(|e| HarnessError::Transport(format!("seed_account: {e}")))?;
    Ok((aid, oid))
}

pub(crate) async fn seed_spec_via_store(
    store: &tanren_app_services::Store,
    pid: ProjectId,
    title: String,
) -> HarnessResult<SpecId> {
    let sid = SpecId::fresh();
    store
        .seed_project_spec(sid, pid, title, String::new(), Utc::now())
        .await
        .map_err(|e| HarnessError::Transport(format!("seed_spec: {e}")))?;
    Ok(sid)
}

pub(crate) async fn seed_dependency_via_store(
    store: &tanren_app_services::Store,
    s: ProjectId,
    ss: SpecId,
    t: ProjectId,
) -> HarnessResult<()> {
    store
        .seed_project_dependency(s, ss, t, Utc::now())
        .await
        .map_err(|e| HarnessError::Transport(format!("seed_dependency: {e}")))?;
    Ok(())
}

pub(crate) async fn seed_active_loop_via_store(
    store: &tanren_app_services::Store,
    pid: ProjectId,
) -> HarnessResult<()> {
    store
        .seed_loop_fixture(pid, true, Utc::now())
        .await
        .map_err(|e| HarnessError::Transport(format!("seed_active_loop: {e}")))?;
    Ok(())
}
