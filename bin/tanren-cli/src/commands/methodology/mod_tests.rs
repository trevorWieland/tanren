use super::*;
use tanren_app_services::methodology::ToolCapability;

#[test]
fn exit_code_validation_is_four() {
    let e = MethodologyError::FieldValidation {
        field_path: "/title".into(),
        expected: "non-empty".into(),
        actual: "\"\"".into(),
        remediation: "supply a title".into(),
    };
    assert_eq!(exit_code_for(&e), 4);
}

#[test]
fn exit_code_io_is_two() {
    let e = MethodologyError::Io {
        path: PathBuf::from("/tmp/x"),
        source: std::io::Error::other("x"),
    };
    assert_eq!(exit_code_for(&e), 2);
}

#[test]
fn resolve_scope_unknown_phase_defaults_deny_without_override() {
    let phase = PhaseId::try_new("cli-admin").expect("phase");
    let scope = resolve_scope_from_inputs(&phase, None, false).expect("scope");
    assert!(!scope.allows(ToolCapability::TaskCreate));
    assert!(!scope.allows(ToolCapability::PhaseEscalate));
}

#[test]
fn resolve_scope_rejects_removed_admin_override_env() {
    let phase = PhaseId::try_new("cli-admin").expect("phase");
    let err =
        resolve_scope_from_inputs(&phase, None, true).expect_err("override env must be rejected");
    assert!(matches!(
        err,
        MethodologyError::FieldValidation { ref field_path, .. }
            if field_path == "/env/TANREN_CAPABILITY_OVERRIDE"
    ));
}

#[test]
fn phase_capabilities_export_matches_domain_bindings() {
    let response =
        render_phase_capabilities(PhaseCapabilitiesArgs { phase: None }).expect("phase export");
    let expected = default_phase_capability_bindings();
    assert_eq!(response.phases.len(), expected.len());
    for (row, binding) in response.phases.iter().zip(expected.iter()) {
        assert_eq!(row.phase, binding.phase.tag());
        let tags = binding
            .capabilities
            .iter()
            .map(|cap| cap.tag().to_owned())
            .collect::<Vec<_>>();
        assert_eq!(row.capabilities, tags);
        assert_eq!(row.capabilities_csv, tags.join(","));
    }
}
