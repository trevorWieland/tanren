//! Remote VM and cloud execution runtime.
//!
//! Depends on: `tanren-runtime`, `tanren-domain`, `tanren-policy`
//!
//! Implements the `ExecutionRuntime` trait for remote VM-backed execution
//! via provider adapters (Hetzner, GCP, `DigitalOcean`, future) and SSH transport.
//! Handles VM provisioning, bootstrapping, workspace setup, and lease lifecycle.
