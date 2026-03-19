"""Shared fixtures and CLI options for tests."""

from __future__ import annotations


def pytest_addoption(parser):
    """Add SSH test CLI options."""
    parser.addoption("--ssh-host", action="store", default=None)
    parser.addoption("--ssh-key", action="store", default=None)
    parser.addoption("--ssh-user", action="store", default="root")
    parser.addoption("--postgres-url", action="store", default=None)
