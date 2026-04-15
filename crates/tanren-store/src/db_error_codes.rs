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
    // SQLITE_CONSTRAINT primary class (19), including extended codes
    // like 2067 for UNIQUE.
    code.parse::<i32>().is_ok_and(|raw| (raw & 0xFF) == 19)
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
    fn sqlite_unique_code_detects_constraint_class() {
        assert!(is_sqlite_unique_violation_code("19"));
        assert!(is_sqlite_unique_violation_code("2067"));
        assert!(!is_sqlite_unique_violation_code("5"));
    }

    #[test]
    fn postgres_unique_code_detects_23505() {
        assert!(is_postgres_unique_violation_code("23505"));
        assert!(is_postgres_unique_violation_code("23505"));
        assert!(!is_postgres_unique_violation_code("40P01"));
    }
}
