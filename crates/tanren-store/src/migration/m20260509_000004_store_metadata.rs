//! R-0008 migration: create `store_metadata` table for installation-wide
//! key material and idempotently seed the `installation_seal_salt` row.
//!
//! The `store_metadata` table is a simple key-value store (TEXT key, BYTEA
//! value) for durable blobs that must survive restarts. The first row is the
//! installation-unique seal salt used by `tanren-configuration-secrets` for
//! credential sealing. The salt is generated via the OS CSPRNG on first
//! migration run and never overwritten on subsequent runs.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub(super) struct Migration;

impl std::fmt::Debug for Migration {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Migration").finish()
    }
}

/// The metadata key for the installation-unique seal salt.
const INSTALLATION_SEAL_SALT_KEY: &str = "installation_seal_salt";

/// Byte length of the installation seal salt. Must match
/// `tanren_configuration_secrets::INSTALLATION_SEAL_SALT_LEN`.
const SEAL_SALT_LEN: usize = 32;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(StoreMetadata::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(StoreMetadata::Key)
                            .text()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(StoreMetadata::Value).binary().not_null())
                    .to_owned(),
            )
            .await?;

        // Seed the installation seal salt row idempotently.
        let backend = manager.get_database_backend();
        let salt = generate_seal_salt();

        match backend {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        "INSERT INTO store_metadata (key, value) \
                         VALUES ('{INSTALLATION_SEAL_SALT_KEY}', '\\x{}') \
                         ON CONFLICT (key) DO NOTHING",
                        hex_encode(&salt),
                    ))
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        "INSERT OR IGNORE INTO store_metadata (key, value) \
                         VALUES ('{INSTALLATION_SEAL_SALT_KEY}', X'{}')",
                        hex_encode(&salt),
                    ))
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(&format!(
                        "INSERT IGNORE INTO store_metadata (`key`, `value`) \
                         VALUES ('{INSTALLATION_SEAL_SALT_KEY}', 0x{})",
                        hex_encode(&salt),
                    ))
                    .await?;
            }
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(StoreMetadata::Table).to_owned())
            .await
    }
}

/// Generate a fresh seal salt using the fallible OS CSPRNG. Panics only
/// if the OS entropy source is unavailable — the migration cannot proceed
/// without entropy (the alternative is writing a deterministic salt, which
/// defeats the purpose).
fn generate_seal_salt() -> Vec<u8> {
    let mut bytes = vec![0u8; SEAL_SALT_LEN];
    getrandom::fill(&mut bytes)
        .expect("OS RNG must be available to generate installation seal salt");
    bytes
}

/// Encode a byte slice as lowercase hex without external dependencies.
fn hex_encode(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut out = String::with_capacity(bytes.len() * 2);
    for &byte in bytes {
        out.push(HEX[(byte >> 4) as usize] as char);
        out.push(HEX[(byte & 0x0F) as usize] as char);
    }
    out
}

#[derive(DeriveIden)]
enum StoreMetadata {
    Table,
    Key,
    Value,
}
