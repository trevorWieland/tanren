# Security and Secrets

## Secret Scopes

Never collapse these scopes:

- **Developer-scoped**: personal tokens/keys (`~/.config/tanren/secrets.env`)
- **Project-scoped**: repo-declared env requirements from `tanren.yml`
- **Infrastructure-scoped**: provider/git credentials for provisioning and clone operations

Each scope has different ownership and lifecycle constraints.

## Configuration Scopes

- **Developer**: local auth and optional role preferences (never committed)
- **Project**: `tanren.yml`, standards, product docs (committed)
- **Organization/installation**: remote/runtime policy (`remote.yml`, org role map)

## Security Controls

- Secrets are redacted in logs.
- Secret files are written with restrictive permissions.
- Teardown includes unconditional secret cleanup.
- Config is resolved/frozen locally before remote execution.
- SSH control path is independent from agent workspace contents.
- Startup recovery and VM state persistence reduce orphan/resource risk.

## Operational Reference

For environment validation and secret CLI commands, see
`worker-manager/README.md`.
