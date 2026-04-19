use std::collections::BTreeMap;

use chrono::Utc;
use sea_orm::ActiveValue::Set;
use sea_orm::{ActiveModelTrait, ColumnTrait, DatabaseTransaction, EntityTrait, QueryFilter};
use tanren_domain::methodology::events::MethodologyEvent;
use tanren_domain::methodology::task::{
    RequiredGuard, TaskAbandonDisposition, TaskGuardFlags, TaskStatus,
};
use tanren_domain::{SpecId, TaskId};

use crate::Store;
use crate::entity::methodology_task_status;
use crate::errors::{StoreError, StoreResult};

const TASK_STATUS_PENDING: &str = "pending";
const TASK_STATUS_IN_PROGRESS: &str = "in_progress";
const TASK_STATUS_IMPLEMENTED: &str = "implemented";
const TASK_STATUS_COMPLETE: &str = "complete";
const TASK_STATUS_ABANDONED: &str = "abandoned";

/// Current store-backed projection for one task's lifecycle state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TaskStatusProjection {
    pub task_id: TaskId,
    pub spec_id: SpecId,
    pub status: TaskStatus,
}

impl Store {
    /// Read the current projection row for one task.
    ///
    /// # Errors
    /// Returns a store/database or conversion error on invalid projection rows.
    pub async fn load_methodology_task_status_projection(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
    ) -> StoreResult<Option<TaskStatusProjection>> {
        let Some(row) = methodology_task_status::Entity::find_by_id(task_id.into_uuid())
            .filter(methodology_task_status::Column::SpecId.eq(spec_id.into_uuid()))
            .one(self.conn())
            .await?
        else {
            return Ok(None);
        };
        Ok(Some(TaskStatusProjection::try_from_model(&row)?))
    }

    /// Upsert one task-status projection row from a pre-folded status value.
    ///
    /// This backfills projection rows for historical streams that predate
    /// `m_0015_methodology_task_status_projection`.
    ///
    /// # Errors
    /// Returns a store/database error on write failures.
    pub async fn upsert_methodology_task_status_projection(
        &self,
        spec_id: SpecId,
        task_id: TaskId,
        status: &TaskStatus,
    ) -> StoreResult<()> {
        let state = task_projection_state_from_status(status);
        let existing = methodology_task_status::Entity::find_by_id(task_id.into_uuid())
            .one(self.conn())
            .await?;
        match existing {
            Some(model) => {
                let mut active = methodology_task_status::ActiveModel::from(model);
                active.spec_id = Set(spec_id.into_uuid());
                active.status = Set(state.tag.as_str().to_owned());
                active.gate_checked = Set(state.guards.gate_checked);
                active.audited = Set(state.guards.audited);
                active.adherent = Set(state.guards.adherent);
                active.extra_guards = Set(extra_guards_json(&state.guards.extra));
                active.updated_at = Set(Utc::now());
                active.update(self.conn()).await?;
            }
            None => {
                methodology_task_status::Entity::insert(methodology_task_status::ActiveModel {
                    task_id: Set(task_id.into_uuid()),
                    spec_id: Set(spec_id.into_uuid()),
                    status: Set(state.tag.as_str().to_owned()),
                    gate_checked: Set(state.guards.gate_checked),
                    audited: Set(state.guards.audited),
                    adherent: Set(state.guards.adherent),
                    extra_guards: Set(extra_guards_json(&state.guards.extra)),
                    updated_at: Set(Utc::now()),
                })
                .exec(self.conn())
                .await?;
            }
        }
        Ok(())
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TaskStatusTag {
    Pending,
    InProgress,
    Implemented,
    Complete,
    Abandoned,
}

impl TaskStatusTag {
    const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => TASK_STATUS_PENDING,
            Self::InProgress => TASK_STATUS_IN_PROGRESS,
            Self::Implemented => TASK_STATUS_IMPLEMENTED,
            Self::Complete => TASK_STATUS_COMPLETE,
            Self::Abandoned => TASK_STATUS_ABANDONED,
        }
    }

    fn parse(raw: &str) -> Result<Self, StoreError> {
        match raw {
            TASK_STATUS_PENDING => Ok(Self::Pending),
            TASK_STATUS_IN_PROGRESS => Ok(Self::InProgress),
            TASK_STATUS_IMPLEMENTED => Ok(Self::Implemented),
            TASK_STATUS_COMPLETE => Ok(Self::Complete),
            TASK_STATUS_ABANDONED => Ok(Self::Abandoned),
            other => Err(StoreError::Conversion {
                context: "methodology_task_status",
                reason: format!("unknown status tag `{other}`"),
            }),
        }
    }

    const fn is_terminal(self) -> bool {
        matches!(self, Self::Complete | Self::Abandoned)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskProjectionState {
    tag: TaskStatusTag,
    guards: TaskGuardFlags,
}

impl Default for TaskProjectionState {
    fn default() -> Self {
        Self {
            tag: TaskStatusTag::Pending,
            guards: TaskGuardFlags::default(),
        }
    }
}

impl TaskProjectionState {
    fn apply(&mut self, op: TaskProjectionOp) {
        match op {
            TaskProjectionOp::Create => {
                self.tag = TaskStatusTag::Pending;
            }
            TaskProjectionOp::Start => {
                if !self.tag.is_terminal() {
                    self.tag = TaskStatusTag::InProgress;
                }
            }
            TaskProjectionOp::Implement => {
                if !self.tag.is_terminal() {
                    self.tag = TaskStatusTag::Implemented;
                }
            }
            TaskProjectionOp::Guard(guard) => {
                self.guards.set(&guard, true);
            }
            TaskProjectionOp::Complete => {
                if self.tag == TaskStatusTag::Implemented {
                    self.tag = TaskStatusTag::Complete;
                }
            }
            TaskProjectionOp::Abandon => {
                if self.tag != TaskStatusTag::Complete {
                    self.tag = TaskStatusTag::Abandoned;
                }
            }
            TaskProjectionOp::Revise => {}
        }
    }

    fn from_model(model: &methodology_task_status::Model) -> Result<Self, StoreError> {
        let tag = TaskStatusTag::parse(&model.status)?;
        let guards = TaskGuardFlags {
            gate_checked: model.gate_checked,
            audited: model.audited,
            adherent: model.adherent,
            extra: parse_extra_guards(&model.extra_guards)?,
        };
        Ok(Self { tag, guards })
    }

    fn to_status(&self) -> TaskStatus {
        match self.tag {
            TaskStatusTag::Pending => TaskStatus::Pending,
            TaskStatusTag::InProgress => TaskStatus::InProgress,
            TaskStatusTag::Implemented => TaskStatus::Implemented {
                guards: self.guards.clone(),
            },
            TaskStatusTag::Complete => TaskStatus::Complete,
            TaskStatusTag::Abandoned => TaskStatus::Abandoned {
                disposition: TaskAbandonDisposition::Replacement,
                replacements: Vec::new(),
                explicit_user_discard_provenance: None,
            },
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum TaskProjectionOp {
    Create,
    Start,
    Implement,
    Guard(RequiredGuard),
    Complete,
    Abandon,
    Revise,
}

#[derive(Debug, Clone, PartialEq, Eq)]
struct TaskProjectionMutation {
    task_id: TaskId,
    spec_id: SpecId,
    op: TaskProjectionOp,
}

impl TaskProjectionMutation {
    fn from_event(event: &MethodologyEvent) -> Option<Self> {
        let (task_id, spec_id, op) = match event {
            MethodologyEvent::TaskCreated(e) => {
                (e.task.id, e.task.spec_id, TaskProjectionOp::Create)
            }
            MethodologyEvent::TaskStarted(e) => (e.task_id, e.spec_id, TaskProjectionOp::Start),
            MethodologyEvent::TaskImplemented(e) => {
                (e.task_id, e.spec_id, TaskProjectionOp::Implement)
            }
            MethodologyEvent::TaskGateChecked(e) => (
                e.task_id,
                e.spec_id,
                TaskProjectionOp::Guard(RequiredGuard::GateChecked),
            ),
            MethodologyEvent::TaskAudited(e) => (
                e.task_id,
                e.spec_id,
                TaskProjectionOp::Guard(RequiredGuard::Audited),
            ),
            MethodologyEvent::TaskAdherent(e) => (
                e.task_id,
                e.spec_id,
                TaskProjectionOp::Guard(RequiredGuard::Adherent),
            ),
            MethodologyEvent::TaskXChecked(e) => (
                e.task_id,
                e.spec_id,
                TaskProjectionOp::Guard(RequiredGuard::Extra(e.guard_name.as_str().to_owned())),
            ),
            MethodologyEvent::TaskCompleted(e) => {
                (e.task_id, e.spec_id, TaskProjectionOp::Complete)
            }
            MethodologyEvent::TaskAbandoned(e) => (e.task_id, e.spec_id, TaskProjectionOp::Abandon),
            MethodologyEvent::TaskRevised(e) => (e.task_id, e.spec_id, TaskProjectionOp::Revise),
            _ => return None,
        };
        Some(Self {
            task_id,
            spec_id,
            op,
        })
    }
}

impl TaskStatusProjection {
    fn try_from_model(model: &methodology_task_status::Model) -> Result<Self, StoreError> {
        let state = TaskProjectionState::from_model(model)?;
        Ok(Self {
            task_id: TaskId::from_uuid(model.task_id),
            spec_id: SpecId::from_uuid(model.spec_id),
            status: state.to_status(),
        })
    }
}

fn parse_extra_guards(value: &serde_json::Value) -> Result<BTreeMap<String, bool>, StoreError> {
    let Some(map) = value.as_object() else {
        return Err(StoreError::Conversion {
            context: "methodology_task_status",
            reason: "extra_guards must be a JSON object".into(),
        });
    };
    let mut out = BTreeMap::new();
    for (name, flag) in map {
        let Some(flag) = flag.as_bool() else {
            return Err(StoreError::Conversion {
                context: "methodology_task_status",
                reason: format!("extra_guards[{name}] must be boolean"),
            });
        };
        out.insert(name.clone(), flag);
    }
    Ok(out)
}

fn extra_guards_json(extra: &BTreeMap<String, bool>) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    for (name, flag) in extra {
        out.insert(name.clone(), serde_json::Value::Bool(*flag));
    }
    serde_json::Value::Object(out)
}

fn task_projection_state_from_status(status: &TaskStatus) -> TaskProjectionState {
    match status {
        TaskStatus::Pending => TaskProjectionState {
            tag: TaskStatusTag::Pending,
            guards: TaskGuardFlags::default(),
        },
        TaskStatus::InProgress => TaskProjectionState {
            tag: TaskStatusTag::InProgress,
            guards: TaskGuardFlags::default(),
        },
        TaskStatus::Implemented { guards } => TaskProjectionState {
            tag: TaskStatusTag::Implemented,
            guards: guards.clone(),
        },
        TaskStatus::Complete => TaskProjectionState {
            tag: TaskStatusTag::Complete,
            guards: TaskGuardFlags::default(),
        },
        TaskStatus::Abandoned { .. } => TaskProjectionState {
            tag: TaskStatusTag::Abandoned,
            guards: TaskGuardFlags::default(),
        },
    }
}

pub(crate) async fn upsert_task_status_projection_txn(
    txn: &DatabaseTransaction,
    event: &MethodologyEvent,
) -> StoreResult<()> {
    let Some(mutation) = TaskProjectionMutation::from_event(event) else {
        return Ok(());
    };

    let existing = methodology_task_status::Entity::find_by_id(mutation.task_id.into_uuid())
        .one(txn)
        .await?;

    let mut state = if let Some(model) = existing.as_ref() {
        if SpecId::from_uuid(model.spec_id) != mutation.spec_id {
            return Err(StoreError::Conversion {
                context: "methodology_task_status",
                reason: format!(
                    "task {} projection exists under spec {}, cannot update with spec {}",
                    mutation.task_id,
                    SpecId::from_uuid(model.spec_id),
                    mutation.spec_id
                ),
            });
        }
        TaskProjectionState::from_model(model)?
    } else {
        TaskProjectionState::default()
    };

    state.apply(mutation.op);

    match existing {
        Some(model) => {
            let mut active = methodology_task_status::ActiveModel::from(model);
            active.spec_id = Set(mutation.spec_id.into_uuid());
            active.status = Set(state.tag.as_str().to_owned());
            active.gate_checked = Set(state.guards.gate_checked);
            active.audited = Set(state.guards.audited);
            active.adherent = Set(state.guards.adherent);
            active.extra_guards = Set(extra_guards_json(&state.guards.extra));
            active.updated_at = Set(Utc::now());
            active.update(txn).await?;
        }
        None => {
            methodology_task_status::Entity::insert(methodology_task_status::ActiveModel {
                task_id: Set(mutation.task_id.into_uuid()),
                spec_id: Set(mutation.spec_id.into_uuid()),
                status: Set(state.tag.as_str().to_owned()),
                gate_checked: Set(state.guards.gate_checked),
                audited: Set(state.guards.audited),
                adherent: Set(state.guards.adherent),
                extra_guards: Set(extra_guards_json(&state.guards.extra)),
                updated_at: Set(Utc::now()),
            })
            .exec(txn)
            .await?;
        }
    }

    Ok(())
}
