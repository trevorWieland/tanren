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

#[cfg(test)]
mod tests {
    use super::{
        is_postgres_contention_code, is_postgres_undefined_table_code,
        is_postgres_unique_violation_code, is_sqlite_contention_code,
        is_sqlite_unique_violation_code,
    };

    #[test]
    fn sqlite_contention_codes_detect_busy_and_locked_classes() {
        assert!(
            is_sqlite_contention_code("5"),
            "SQLITE_BUSY should classify"
        );
        assert!(
            is_sqlite_contention_code("6"),
            "SQLITE_LOCKED should classify"
        );
        assert!(
            is_sqlite_contention_code("261"),
            "extended SQLITE_BUSY code should classify via primary code"
        );
        assert!(
            !is_sqlite_contention_code("2067"),
            "unique violation is not contention"
        );
    }

    #[test]
    fn postgres_contention_codes_detect_deadlock_and_serialization() {
        assert!(is_postgres_contention_code("40P01"));
        assert!(is_postgres_contention_code("40001"));
        assert!(is_postgres_contention_code("55P03"));
        assert!(!is_postgres_contention_code("23505"));
    }

    #[test]
    fn postgres_undefined_table_code_detects_42p01() {
        assert!(is_postgres_undefined_table_code("42P01"));
        assert!(is_postgres_undefined_table_code("42p01"));
        assert!(!is_postgres_undefined_table_code("42703"));
    }

    #[test]
    fn sqlite_unique_code_only_matches_uniqueness_subclasses() {
        // Uniqueness-style extended codes must classify as unique.
        assert!(
            is_sqlite_unique_violation_code("1555"),
            "PRIMARYKEY extended code"
        );
        assert!(
            is_sqlite_unique_violation_code("2067"),
            "UNIQUE extended code"
        );
        assert!(
            is_sqlite_unique_violation_code("2579"),
            "ROWID extended code"
        );

        // Bare primary class 19 covers many constraint kinds. It
        // must NOT be treated as a unique violation — that was the
        // bug the lane-0.4 review flagged.
        assert!(!is_sqlite_unique_violation_code("19"), "bare CONSTRAINT");

        // Non-uniqueness constraint subclasses must reject too.
        assert!(!is_sqlite_unique_violation_code("275"), "CHECK = 275");
        assert!(!is_sqlite_unique_violation_code("787"), "FOREIGNKEY = 787");
        assert!(!is_sqlite_unique_violation_code("1043"), "FUNCTION = 1043");
        assert!(!is_sqlite_unique_violation_code("1299"), "NOTNULL = 1299");
        assert!(!is_sqlite_unique_violation_code("1811"), "TRIGGER = 1811");

        // Non-constraint codes must reject.
        assert!(!is_sqlite_unique_violation_code("5"), "BUSY");
        assert!(!is_sqlite_unique_violation_code("6"), "LOCKED");
    }

    #[test]
    fn postgres_unique_code_detects_23505() {
        assert!(is_postgres_unique_violation_code("23505"));
        assert!(is_postgres_unique_violation_code("23505"));
        assert!(!is_postgres_unique_violation_code("40P01"));
    }
}
