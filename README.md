# tanren

Development lifecycle framework for AI-assisted software projects.

tanren provides a structured set of commands, standards, and conventions that AI agents use to shape specs, manage roadmaps, enforce coding standards, and orchestrate development tasks.

## Quick Start

```bash
cd your-project
~/github/tanren/scripts/install.sh --profile python-uv
```

This installs:
- **Commands** into `.claude/commands/tanren/` and `.opencode/commands/tanren/`
- **Standards** from the selected profile into `tanren/standards/`
- **Product templates** into `tanren/product/`
- **Scripts** (orchestrator, audit tools) into `tanren/scripts/`

## Structure

```
tanren/
├── commands/        # 14 command files for AI agent workflows
├── scripts/         # Orchestration and audit scripts
├── templates/       # Starter templates for product docs, audits, Makefile
├── profiles/        # Standards profiles (default, python-uv)
├── worker-manager/  # Autonomous orchestration (planned)
├── protocol/        # Agent communication protocol (planned)
└── config.yml       # Framework configuration
```

## Profiles

- **default** — Minimal language-agnostic standards
- **python-uv** — Python + uv standards (typing, testing, architecture patterns)

## Commands

| Command | Purpose |
|---------|---------|
| shape-spec | Shape a spec from issue to implementation-ready branch |
| walk-spec | Walk through spec steps interactively |
| do-task | Execute a single task autonomously |
| audit-task | Audit completed task against standards |
| audit-spec | Audit a spec for completeness |
| run-demo | Run a demo of implemented features |
| sync-roadmap | Synchronize roadmap with current state |
| plan-product | Plan product features and priorities |
| handle-feedback | Process and route feedback |
| resolve-blockers | Resolve blocking issues |
| discover-standards | Discover and propose new standards |
| inject-standards | Inject standards into the project |
| index-standards | Rebuild the standards index |
| triage-audits | Triage audit findings |

## Install Modes

- **Fresh install**: Creates `tanren/` directory with templates, standards, scripts, and commands
- **Update**: Refreshes only scripts and commands (preserves project-specific content)
