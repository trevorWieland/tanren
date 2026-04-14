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
