use sea_orm::entity::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum DispatchStatusModel {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum LaneModel {
    #[sea_orm(string_value = "impl")]
    Impl,
    #[sea_orm(string_value = "audit")]
    Audit,
    #[sea_orm(string_value = "gate")]
    Gate,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum OutcomeModel {
    #[sea_orm(string_value = "success")]
    Success,
    #[sea_orm(string_value = "fail")]
    Fail,
    #[sea_orm(string_value = "blocked")]
    Blocked,
    #[sea_orm(string_value = "error")]
    Error,
    #[sea_orm(string_value = "timeout")]
    Timeout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum StepTypeModel {
    #[sea_orm(string_value = "provision")]
    Provision,
    #[sea_orm(string_value = "execute")]
    Execute,
    #[sea_orm(string_value = "teardown")]
    Teardown,
    #[sea_orm(string_value = "dry_run")]
    DryRun,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum StepStatusModel {
    #[sea_orm(string_value = "pending")]
    Pending,
    #[sea_orm(string_value = "running")]
    Running,
    #[sea_orm(string_value = "completed")]
    Completed,
    #[sea_orm(string_value = "failed")]
    Failed,
    #[sea_orm(string_value = "cancelled")]
    Cancelled,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, EnumIter, DeriveActiveEnum)]
#[sea_orm(rs_type = "String", db_type = "String(StringLen::None)")]
pub enum StepReadyStateModel {
    #[sea_orm(string_value = "blocked")]
    Blocked,
    #[sea_orm(string_value = "ready")]
    Ready,
}
