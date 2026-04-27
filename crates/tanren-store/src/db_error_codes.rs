use sea_orm::{DbErr, RuntimeErr, SqlxError};

pub(crate) fn extract_db_error_code(db_err: &DbErr) -> Option<String> {
    match db_err {
        DbErr::Conn(RuntimeErr::SqlxError(SqlxError::Database(db)))
        | DbErr::Exec(RuntimeErr::SqlxError(SqlxError::Database(db)))
        | DbErr::Query(RuntimeErr::SqlxError(SqlxError::Database(db))) => {
            db.code().map(std::borrow::Cow::into_owned)
        }
        _ => None,
    }
}

pub(crate) fn is_sqlite_contention_code(code: &str) -> bool {
    // SQLite uses integer result codes, with extended codes preserving
    // the primary class in the lower byte.
    code.parse::<i32>()
        .is_ok_and(|raw| matches!(raw & 0xFF, 5 | 6))
}

pub(crate) fn is_postgres_contention_code(code: &str) -> bool {
    matches!(
        code.to_ascii_uppercase().as_str(),
        "40P01" | "40001" | "55P03"
    )
}

pub(crate) fn is_postgres_undefined_table_code(code: &str) -> bool {
    code.eq_ignore_ascii_case("42P01")
}

pub(crate) fn is_sqlite_unique_violation_code(code: &str) -> bool {
    // SQLite reports the extended constraint code (primary class 19
    // OR'd with a subclass in the high byte). We only classify the
    // uniqueness-style subclasses as "unique violations":
    //
    //   SQLITE_CONSTRAINT_PRIMARYKEY = 1555  (SQLITE_CONSTRAINT | 6<<8)
    //   SQLITE_CONSTRAINT_UNIQUE     = 2067  (SQLITE_CONSTRAINT | 8<<8)
    //   SQLITE_CONSTRAINT_ROWID      = 2579  (SQLITE_CONSTRAINT | 10<<8)
    //
    // The bare primary class (19) is intentionally NOT classified
    // because it also covers NOT NULL, FOREIGN KEY, CHECK, TRIGGER,
    // and FUNCTION violations — those must surface as their own
    // typed errors instead of being silently treated as "the row
    // already exists" by the replay-store code path.
    matches!(code.parse::<i32>().ok(), Some(1555 | 2067 | 2579))
}

pub(crate) fn is_postgres_unique_violation_code(code: &str) -> bool {
    code.eq_ignore_ascii_case("23505")
}
