//! Constrain `request_fingerprint` to `CHAR(43)` so the column enforces
//! the fixed-length `SHA-256` base64url-no-pad digest invariant at the
//! database level. `SHA-256` produces 32 bytes; base64url-no-pad
//! encoding yields exactly 43 characters.

use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrganizationCreateIdempotency::Table)
                    .modify_column(
                        ColumnDef::new(OrganizationCreateIdempotency::RequestFingerprint)
                            .char_len(43)
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(OrganizationCreateIdempotency::Table)
                    .modify_column(
                        ColumnDef::new(OrganizationCreateIdempotency::RequestFingerprint)
                            .string()
                            .not_null(),
                    )
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum OrganizationCreateIdempotency {
    Table,
    RequestFingerprint,
}
