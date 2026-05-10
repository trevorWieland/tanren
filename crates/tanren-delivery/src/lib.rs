//! Shared delivery service for Tanren install, upgrade, and repository
//! asset management.
//!
//! This crate owns the delivery subsystem's install planning, manifest
//! management, catalog, upgrade preview/apply, and BDD fixture support.
//! CLI argument parsing and stdout formatting live in `tanren-cli-app`;
//! API routing and HTTP fixtures live in `tanren-api-app`. Both call
//! the typed service functions exposed here.

pub mod install;
