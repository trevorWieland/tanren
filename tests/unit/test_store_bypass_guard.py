from __future__ import annotations

import importlib.util
from pathlib import Path


def load_check_store_bypass_module():
    module_path = Path("scripts/check_store_bypass.py").resolve()
    spec = importlib.util.spec_from_file_location("check_store_bypass", module_path)
    assert spec is not None
    assert spec.loader is not None
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module


def test_app_services_glob_is_workspace_wide() -> None:
    module = load_check_store_bypass_module()
    assert "crates/tanren-app-services/src/**/*.rs" in module.INTERFACE_GLOBS


def test_flags_store_param_construction_in_app_services_file(tmp_path: Path) -> None:
    module = load_check_store_bypass_module()

    sample = tmp_path / "crates" / "tanren-app-services" / "src" / "fake_service.rs"
    sample.parent.mkdir(parents=True)
    sample.write_text(
        "fn bad() { let _params = CancelDispatchParams { dispatch_id: todo!() }; }\n",
        encoding="utf-8",
    )

    violations = module.check_file(sample)
    assert any("constructs `CancelDispatchParams`" in line for line in violations)


def test_flags_multiline_store_method_calls_in_app_services_file(tmp_path: Path) -> None:
    module = load_check_store_bypass_module()

    sample = tmp_path / "crates" / "tanren-app-services" / "src" / "fake_service.rs"
    sample.parent.mkdir(parents=True, exist_ok=True)
    sample.write_text(
        (
            "async fn bad(store: &tanren_store::Store) {\n"
            "    store\n"
            "        .append_batch(&[])\n"
            "        .await\n"
            "        .expect(\"append batch\");\n"
            "}\n"
        ),
        encoding="utf-8",
    )

    violations = module.check_file(sample)
    assert any("direct `append_batch(...)` call in app-services" in line for line in violations)


def test_flags_direct_store_path_cancel_dispatch_in_app_services_file(tmp_path: Path) -> None:
    module = load_check_store_bypass_module()

    sample = tmp_path / "crates" / "tanren-app-services" / "src" / "fake_service.rs"
    sample.parent.mkdir(parents=True, exist_ok=True)
    sample.write_text(
        (
            "async fn bad(store: &dyn tanren_store::StateStore, params: tanren_store::CancelDispatchParams) {\n"
            "    store.cancel_dispatch(params).await.expect(\"cancel\");\n"
            "}\n"
        ),
        encoding="utf-8",
    )

    violations = module.check_file(sample)
    assert any(
        "direct `cancel_dispatch(...)` store-path call in app-services" in line
        for line in violations
    )


def test_allows_orchestrator_cancel_dispatch_in_app_services_file(tmp_path: Path) -> None:
    module = load_check_store_bypass_module()

    sample = tmp_path / "crates" / "tanren-app-services" / "src" / "fake_service.rs"
    sample.parent.mkdir(parents=True, exist_ok=True)
    sample.write_text(
        (
            "async fn ok(self_ref: &Service, cmd: tanren_domain::CancelDispatch) {\n"
            "    self_ref\n"
            "        .orchestrator\n"
            "        .cancel_dispatch(cmd)\n"
            "        .await\n"
            "        .expect(\"cancel\");\n"
            "}\n"
        ),
        encoding="utf-8",
    )

    violations = module.check_file(sample)
    assert not any("cancel_dispatch" in line for line in violations)


def test_flags_multiline_transport_binary_method_call(tmp_path: Path) -> None:
    module = load_check_store_bypass_module()

    sample = tmp_path / "bin" / "tanren-cli" / "src" / "fake.rs"
    sample.parent.mkdir(parents=True, exist_ok=True)
    sample.write_text(
        (
            "async fn bad(service: &tanren_store::Store) {\n"
            "    service\n"
            "        .append(&todo!())\n"
            "        .await\n"
            "        .expect(\"append\");\n"
            "}\n"
        ),
        encoding="utf-8",
    )

    violations = module.check_file(sample)
    assert any("direct `append(...)` call in transport binary" in line for line in violations)
