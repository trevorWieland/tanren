"""Ownership contract tests — verify endpoints thread auth for resource access.

These tests introspect FastAPI router function signatures to ensure that
every endpoint accepting a resource ID passes user_id to the service layer,
preventing cross-user data leakage.
"""

from __future__ import annotations

import ast
import inspect

import pytest

from tanren_api.routers import dispatch as dispatch_router
from tanren_api.routers import run as run_router
from tanren_api.routers import vm as vm_router

# Endpoints that accept a resource ID and must enforce ownership.
# Format: (module, function_name, expected_service_kwarg)
OWNERSHIP_ENDPOINTS = [
    (dispatch_router, "get_dispatch", "user_id"),
    (dispatch_router, "cancel_dispatch", "user_id"),
    (vm_router, "get_provision_status", "user_id"),
    (vm_router, "release_vm", "user_id"),
    (run_router, "run_execute", "user_id"),
    (run_router, "run_teardown", "user_id"),
    (run_router, "run_status", "user_id"),
]


@pytest.mark.parametrize(
    ("module", "func_name", "expected_kwarg"),
    OWNERSHIP_ENDPOINTS,
    ids=[f"{m.__name__.split('.')[-1]}.{f}" for m, f, _ in OWNERSHIP_ENDPOINTS],
)
def test_endpoint_uses_auth_not_underscore(module, func_name, expected_kwarg):
    """Verify the endpoint has 'auth' parameter (not '_auth' — meaning it's used)."""
    func = getattr(module, func_name)
    sig = inspect.signature(func)
    param_names = list(sig.parameters.keys())
    assert "auth" in param_names, (
        f"{func_name} uses '_auth' (unused). Rename to 'auth' and pass user_id to the service call."
    )
    assert "_auth" not in param_names, f"{func_name} still has '_auth' parameter"


@pytest.mark.parametrize(
    ("module", "func_name", "expected_kwarg"),
    OWNERSHIP_ENDPOINTS,
    ids=[f"{m.__name__.split('.')[-1]}.{f}" for m, f, _ in OWNERSHIP_ENDPOINTS],
)
def test_endpoint_passes_user_id_to_service(module, func_name, expected_kwarg):
    """Verify the endpoint function body includes user_id= in a service call."""
    func = getattr(module, func_name)
    source = inspect.getsource(func)
    tree = ast.parse(source)

    # Look for keyword argument user_id= in any call within the function
    found_user_id = False
    for node in ast.walk(tree):
        if isinstance(node, ast.keyword) and node.arg == expected_kwarg:
            found_user_id = True
            break

    assert found_user_id, (
        f"{func_name} does not pass '{expected_kwarg}=' to its service call. "
        "All resource-ID endpoints must thread ownership."
    )
