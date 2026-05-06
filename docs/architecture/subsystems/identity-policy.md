---
schema: tanren.subsystem_architecture.v0
subsystem: identity-policy
status: accepted
owner_command: architect-system
updated_at: 2026-04-29
---

# Identity And Policy Architecture

## Purpose

This document defines Tanren's identity, authorization, approval, and policy
architecture. It is the authority for who an actor is, what scope they are
acting in, what they are allowed to do, which actions require approval, and how
identity and policy decisions become auditable state.

Identity and policy exist in every Tanren install. Solo use, team use, local
compose, and larger self-hosted deployments share the same model.

## Subsystem Boundary

The identity and policy subsystem owns:

- users, accounts, organizations, memberships, invitations, and active scope;
- project ownership and project access grants;
- service accounts, API keys, and non-human actor identity;
- worker-scoped temporary access;
- permission definitions and scoped permission grants;
- roles as permission grant templates;
- administrative permission protections;
- policy evaluation and policy-denial explanations;
- approval requirements, approval state, and approval authority;
- capability discovery inputs for interfaces and MCP tools;
- identity and policy events consumed by audit, observation, runtime, and
  interfaces.

The subsystem does not own secret storage, provider credential values, runtime
placement execution, event append mechanics, interface presentation, or
subsystem-specific policy vocabulary. Those records belong to their owning
subsystems, but their commands must call this subsystem for actor and policy
decisions.

## Core Invariants

1. **Authorization checks permissions, not personas.** Personas describe user
   needs. They do not grant access.
2. **Roles are grant-time templates.** A role bundles permissions for easier
   assignment. Applying a role creates permission grants. Later role changes do
   not retroactively alter existing grants.
3. **Allow grants only.** Tanren grants permissions explicitly and otherwise
   denies by default. General deny grants are not part of the v1 policy model.
4. **Scope is always explicit.** Every policy decision resolves installation,
   account, organization, project, resource, and assignment context where
   applicable.
5. **Solo and team use share one model.** A solo builder still acts through a
   user, account, project, permissions, service accounts, and audit history.
6. **Service accounts are first-class actors.** They are not personas and are
   not merely API keys owned by the creating user.
7. **Worker access is temporary and bounded.** Internal workers receive only
   the access required for their assignment and lose it when the assignment no
   longer needs it.
8. **Approvals are policy state.** Approval gates, approval authority, approval
   decisions, and approval satisfaction are typed, auditable state.
9. **Policy decisions are explainable.** Denials and approval requirements
   return stable reasons that interfaces can show without leaking hidden state.
10. **Identity and policy changes are events.** Accounts, memberships, grants,
    role changes, service accounts, approvals, denials, and policy changes are
    recorded through the canonical event log.

## Identity Model

Tanren separates login identity from action scope.

- **User**: a human login identity. A user may control or access multiple
  accounts.
- **Account**: the security and workspace container a user acts through.
  Accounts own or can access projects and can belong to organizations.
- **Organization**: a governance entity that owns projects, members, policy,
  shared configuration, shared service accounts, and shared access rules.
- **Project**: exactly one source-control repository registered in Tanren.
  A project is either account-owned or organization-owned.
- **Service account**: a non-human actor with its own name, purpose, scope,
  permissions, status, credentials, and audit identity.
- **Worker actor**: a temporary non-human actor created or scoped for one
  runtime assignment.
- **Provider actor**: an external provider, webhook sender, or integration
  callback actor authenticated through integration-specific credentials.
- **System actor**: Tanren itself when performing internal maintenance,
  projection repair, scheduled analysis, recovery, or bootstrap work.

A builder who creates a service account is not the service account. Creation,
ownership, and management rights are distinct from the service account's own
actor identity and permission grants.

## Account And Organization Model

A user can move between accounts and organizations they are allowed to access.
This supports a single builder who works across personal projects and multiple
organizations.

Account and organization rules:

- an account may belong to zero, one, or more organizations;
- a personal account belongs to no organization by default;
- an organization may own many projects;
- an account may own personal projects directly;
- organization membership alone does not grant access to every organization
  project;
- project access is resolved through project-level grants plus inherited or
  constraining organization policy;
- account-owned and organization-owned projects use the same project permission
  model.

First-run bootstrap creates an initial account with the administrative
permissions required to configure the installation. It does not create a
separate no-auth or local-only identity model.

## Scope Model

Policy decisions evaluate a request in a resolved scope.

Scopes include:

- **Installation**: self-hosted Tanren deployment administration and bootstrap.
- **Account**: projects, settings, service accounts, credentials, and activity
  available to an account.
- **Organization**: organization-owned projects, memberships, shared policy,
  shared configuration, and shared service accounts.
- **Project**: one registered repository and its planning, orchestration,
  runtime, behavior-proof, integrations, and observation state.
- **Resource**: a specific behavior, roadmap item, spec, task, finding,
  approval, worker assignment, credential metadata record, proof/source item, or
  provider connection.
- **Assignment**: temporary runtime scope for worker and harness execution.

The active account and, when relevant, active organization and project are
part of actor context. Interfaces may help users switch active scope, but they
must not bypass policy by switching interfaces.

## Permissions

Permissions are stable, named capabilities evaluated at a scope.

Permission grants record:

- grantee actor;
- permission name;
- grant scope;
- granting actor;
- grant reason or source;
- optional expiration;
- optional conditions supported by policy;
- event position and audit metadata.

If a permission is absent, access is denied by default. Tanren does not need a
general explicit deny grant because policy constraints, resource lifecycle
state, approvals, maintenance mode, credential status, and runtime boundaries
can block actions even when a user holds broad permissions.

Permission namespaces should follow subsystem boundaries, for example:

- organization and membership administration;
- project access and project administration;
- planning and architecture mutation;
- roadmap, spec, task, and review actions;
- runtime placement and worker operations;
- proof and audit access;
- configuration, credential, and secret management;
- integration and webhook management;
- observation, export, and recovery operations.

Subsystem records own their specific permission names. This document owns the
grant, evaluation, and audit model.

## Roles

Roles are named permission templates.

Applying a role creates direct permission grants for the selected grantee and
scope. After the grants are created, policy evaluation checks the resulting
permissions, not the role identity.

Role rules:

- roles may be installation, account, organization, or project scoped;
- role application emits grant events for the permissions it applies;
- editing a role changes future applications only;
- deleting or retiring a role does not remove permissions already granted from
  that role;
- audit views may preserve the role used as the grant source.

This keeps authorization transparent and prevents hidden retroactive access
changes when a role template is edited.

## Administrative Protection

Administrative permissions govern access, policy, configuration, organization
membership, project ownership, service accounts, secrets, and recovery.

Tanren must prevent actions that would leave a governed scope without any
remaining holder of a required administrative permission. For example, the last
organization member with organization-access administration rights cannot
remove themselves or be removed until another eligible holder exists.

Administrative protection is enforced by policy checks and recorded through
policy-denial or accepted-change events.

## Service Accounts

Service accounts are scoped non-human identities for automation, external API
clients, builder-owned agents, webhooks, and integration workflows.

Service accounts may be created at account, organization, or project scope.
They have:

- stable identity;
- name and description;
- owner or responsible maintainer;
- purpose metadata;
- scope;
- permission grants;
- credential metadata;
- status such as active, suspended, revoked, or retired;
- creation, change, suspension, revocation, and deletion history.

Service accounts receive no unrestricted access by default. Their permissions
are explicit grants, and their credentials are managed through the
configuration and secrets subsystem.

Service account actions are attributed to the service account actor in audit
views, with links to owner and purpose metadata where visible.

## API Keys And Sessions

API keys, browser sessions, CLI credentials, TUI credentials, MCP credentials,
worker tokens, and provider callback credentials authenticate actors. They do
not define authorization by themselves.

Authentication artifacts resolve to an actor context. Policy evaluation then
checks permissions, scope, capability claims, resource state, and applicable
policy constraints.

Credential values and secrets are owned by the configuration and secrets
subsystem. Identity and policy own the actor relationship, status, permission
binding, and audit semantics.

## Worker-Scoped Access

Internal Tanren workers operate through worker-scoped actor context tied to a
specific assignment.

Worker-scoped access rules:

- access is created for one assignment or bounded workflow;
- access includes only required project, resource, harness, credential-use,
  and proof permissions;
- access is time-limited where practical;
- access is revoked or expires when the assignment completes, fails, is
  cancelled, is superseded, or violates policy;
- worker actions are attributable to the worker actor and correlated to the
  assignment, service account, user, or system action that caused the work.

Worker-scoped access lets internal automation use MCP and API paths without
giving workers broad standing credentials.

## Policy Evaluation

Every protected command and read resolves a policy decision before returning
protected data or performing side effects.

Policy inputs include:

- actor context;
- authentication artifact status;
- active account, organization, project, resource, and assignment scope;
- permission grants;
- service account or worker capability claims;
- resource lifecycle state;
- organization and project policy;
- configuration and credential-use policy;
- runtime placement policy;
- budget, quota, maintenance, or incident mode policy;
- required approval state;
- visibility and redaction rules.

Policy outcomes are:

- **allow**: the action may proceed;
- **deny**: the action must fail with a stable explanation;
- **approval_required**: the action is blocked until configured approval
  conditions are satisfied.

Policy evaluation should be implemented as typed Tanren application logic over
event-sourced read models. The architecture does not require a separate
external policy engine.

## Approvals

Approvals are policy-controlled state transitions.

Approval rules:

- approval requirements are derived from policy and resource state;
- approval authority is granted by permission;
- approval requests identify the gated action, scope, resource, policy
  condition, requester, and required approval condition;
- approval decisions are attributed and event-recorded;
- an approval satisfies only the action, resource, scope, and policy condition
  it was created for;
- approval state is visible without exposing hidden approver details or hidden
  resources;
- approval cannot bypass audit, credential policy, or runtime isolation.

Approvals are not comments, reactions, or informal acknowledgements. They are
first-class policy records.

## Capability Discovery

Interfaces and MCP tool discovery may ask what an actor can do in a scope.

Capability discovery is derived from:

- permissions;
- service account or worker scope;
- assignment context;
- current resource lifecycle state;
- active policy constraints;
- required approvals;
- redaction and visibility rules.

Capability discovery is advisory and may be stale. Command and query execution
must still perform authoritative policy checks.

Capability claims are generally derived rather than stored as independent
durable grants. External credentials or worker tokens may carry claims, but
those claims are interpreted against Tanren state and policy before use.

## Visibility And Redaction

Identity and policy participate in visibility decisions for events, read
models, proof/source records, runtime output, credentials metadata, and
operational views.

Visibility rules:

- hidden resources should not leak through search, counts, autocomplete, error
  details, or event-stream access;
- users may see that an action is blocked by policy without seeing protected
  details they lack permission to view;
- service accounts and workers see only state needed for their assigned scope;
- redaction decisions must be consistent across web, API, MCP, CLI, and TUI;
- raw event access is filtered through the same permission and visibility
  model as read models.

## Audit And Events

Identity and policy state is event-sourced. Events include:

- user and account creation;
- account switching and active-scope changes where durable;
- organization creation, membership, invitation, removal, and deletion;
- project ownership and project access changes;
- permission grants, changes, expirations, and revocations;
- role creation, update, retirement, deletion, and application;
- service account creation, update, suspension, revocation, and retirement;
- API key and authentication artifact metadata changes;
- worker-scoped access creation and revocation;
- policy configuration changes;
- policy denials where audit policy requires recording;
- approval requests, approvals, rejections, expiry, and satisfaction;
- administrative-protection denials.

Audit views distinguish human users, accounts, service accounts, worker actors,
provider actors, and system actors. Audit records link decisions to
correlation IDs, causation IDs, event positions, and visible proof/source links where
applicable.

## Accepted Identity And Policy Decisions

- Users and accounts are distinct: a user is a login identity; an account is a
  security and workspace container the user acts through.
- A user may access multiple accounts and organizations.
- A project is either account-owned or organization-owned and always uses the
  same project permission model.
- Organization membership alone does not grant every project permission.
- Authorization checks permissions, not roles or personas.
- Roles are grant-time permission templates.
- Role edits do not retroactively mutate existing grants.
- Tanren uses allow grants and implicit deny, not general explicit deny grants.
- Service accounts may exist at account, organization, or project scope.
- Service accounts are first-class actors distinct from the users who create
  them.
- Bootstrap creates an initial account with installation administrative
  permissions, not a separate no-auth identity model.
- Approval authority is permission-based.
- Capability claims are normally derived from permissions, scope, assignment,
  and policy rather than stored as independent durable grants.
- Policy evaluation is typed Tanren application logic over event-sourced state;
  no external policy engine is required by the architecture.
- Last-holder protection applies to installation administration, account owner,
  organization owner, project administrator, policy administrator, credential
  administrator, and provider administrator permissions.
- Durable policy-denial audit events are emitted for sensitive actions,
  credential access, provider actions, raw export, role/grant changes,
  approvals, runtime provisioning, and merge operations. Routine read denials
  may remain request-local.
- Bootstrap role templates are owner, administrator, builder, reviewer,
  observer, operator, integration-manager, and billing/budget-manager where the
  scope supports them.
- Service-account credentials support API keys, MCP bearer tokens, webhook
  signing keys, provider OAuth/client credentials, and short-lived worker
  assignment tokens.
- Built-in approval policies cover sensitive credentials, provider actions,
  merge readiness, baseline-control disablement, raw export, destructive
  restore, runtime budget override, and production-affecting operations.

## Canonical Credential And Session Decisions

These decisions land with R-0001 (B-0043 "Create an account") and apply to
every subsequent feature that handles credentials, secrets, or session
tokens. They cross-reference
[`profiles/rust-cargo/architecture/secrets-handling.md`](../../../profiles/rust-cargo/architecture/secrets-handling.md)
and
[`profiles/rust-cargo/architecture/id-formats.md`](../../../profiles/rust-cargo/architecture/id-formats.md).

### Password hashing

The canonical password verifier impl is `Argon2idVerifier` in
`tanren-identity-policy::argon2`, an implementation of the
`CredentialVerifier` trait. It is the only verifier impl Tanren ships;
inline SHA-256 paths are removed and the workspace `clippy.toml` denies
`sha2::Sha256::new` outside an explicit allowlist.

Production parameters track the **OWASP 2025 floor**:
`m = 19 MiB, t = 2, p = 1`. Passwords are stored as a single PHC string
(`$argon2id$v=19$m=19456,t=2,p=1$<salt>$<hash>`) in one TEXT column; the
salt is embedded in the PHC string and there is no separate salt column.

`Argon2idVerifier::fast_for_tests()` is gated on
`cfg(any(test, feature = "test-hooks"))` and uses cheap params
(`m = 8 KiB, t = 1, p = 1`) so BDD scenarios stay fast. The production
verifier is the only one a release build can construct.

### Secrets handling and the `xtask check-secrets` allowlist

Every credential-shaped field in workspace types uses
`secrecy::SecretString` (or a workspace newtype that wraps one). This
applies to:

- request types in `tanren-contract` (`SignUpRequest`, `SignInRequest`,
  `AcceptInvitationRequest`, …);
- store record types whose fields hold raw credential material;
- env-loaded credentials such as `TANREN_MCP_API_KEY` in
  `bin/tanren-mcp`.

`xtask check-secrets` (a `syn`-based AST walker) rejects struct fields
whose name matches the regex
`(?i)password|secret|api_key|credential|session_token|bearer|private_key|csrf|auth_token`
unless the field type is in the allowlist:

- `secrecy::SecretString` and `secrecy::SecretBox<_>`;
- workspace newtype wrappers listed in `xtask/secret-newtypes.toml`.

Adding a new credential-shaped newtype requires registering it in
`xtask/secret-newtypes.toml`; that registration is the audit trail for
the wrapper's existence. OpenAPI examples MUST never include real
secret values; `utoipa` annotations on credential request fields use
placeholder values only.

### Session tokens

Session tokens are 32 random bytes from a CSPRNG (`rand::random::<[u8;
32]>()`), encoded URL-safe base64 with no padding via the `base64` crate,
and wrapped in `SessionToken(SecretString)`. They are explicitly NOT
UUIDs — UUIDs are identifiers, not secrets, and using them as session
tokens conflates the two.

`SessionToken` exposes the inner value only via `expose_secret()`. Its
`Debug` impl prints `SessionToken(<redacted>)`. It implements neither
`Display` nor `Serialize` for the inner bytes. The `clippy.toml`
workspace lint denies `uuid::Uuid::new_v4` in `tanren-app-services` and
the `tanren-{api,cli,mcp,tui}-app` lib crates so a future contributor
cannot regress to a UUID session token.

## Canonical Existing-Account Join Decisions

These decisions land with R-0006 ("Join an organization with an existing
account") and apply to every feature that handles existing-account
invitation acceptance.

### Existing-account join flow

An authenticated Tanren account can accept an invitation to join an
organization without creating a new account. The flow is:

1. The caller presents a valid session (resolving to an `AccountId`) and
   an invitation token.
2. The handler looks up the account and resolves its identifier.
3. The handler delegates to the store's atomic
   `accept_existing_invitation_atomic` path, which inside one transaction:
   verifies the invitation is pending, unexpired, not revoked, and
   addressed to the accepting account's identifier; inserts a membership
   carrying the invitation's org-level permissions; consumes the
   invitation; and appends an `organization_joined` event.
4. The account's other organization memberships are unaffected.
5. Project access is NOT granted automatically — project access is
   governed by M-0031.

### Join failure taxonomy

`AccountFailureReason` includes two stable codes specific to the join
path:

- `wrong_account` (HTTP 403): the invitation is addressed to a different
  account's identifier.
- `unauthenticated` (HTTP 401): the caller attempted a join without a
  valid session.

Other join-path failures reuse existing invitation codes
(`invitation_not_found`, `invitation_expired`,
`invitation_already_consumed`).

### Response shape

`JoinOrganizationResponse` carries the joined org, the membership's
org-level permissions, the full list of selectable organization
memberships (so the caller can offer org switching via R-0004), and an
explicitly empty `project_access_grants` list. The empty list makes
project-level access visibly absent without introducing grant types owned
by the project-access subsystem (M-0031).

## Rejected Alternatives

- **Personas as authorization subjects.** Rejected because personas describe
  product needs and should not grant access.
- **Roles as runtime authorization.** Rejected because permission grants are
  more transparent, auditable, and easier to reason about.
- **General explicit deny grants.** Rejected because allow grants plus implicit
  deny and typed policy constraints keep policy reasoning simpler.
- **User-owned API keys as service accounts.** Rejected because service
  accounts need independent actor identity, ownership, scope, status, and
  audit history.
- **No-auth local mode.** Rejected because solo use must not create a separate
  architecture that breaks audit, service accounts, MCP access, or team
  migration.
- **External policy engine as a baseline dependency.** Rejected because Tanren
  can express v1 policy needs through typed application policy over
  event-sourced state.
