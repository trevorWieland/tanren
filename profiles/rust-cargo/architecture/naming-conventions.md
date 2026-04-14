# Cross-Layer Naming Conventions

Consistent naming across Rust modules, types, database schema, and API surfaces. Each layer follows its natural convention.

```rust
// ✓ Good: Consistent naming across layers

// Module: snake_case
mod task_dispatch;

// Type: PascalCase
pub struct TaskDispatch { /* ... */ }

// Error: {Domain}Error
pub enum DispatchError {
    AgentUnavailable { agent_id: AgentId },
    BudgetExhausted { budget_id: BudgetId },
}

// Function: snake_case, verb-first
pub fn create_dispatch(cmd: CreateDispatchCommand) -> Result<TaskDispatch, DispatchError>;
pub fn validate_budget(budget_id: BudgetId) -> Result<Budget, PolicyError>;
pub fn is_active(&self) -> bool;  // Predicate: is_/has_/can_ prefix
```

**Database naming:**

```sql
-- ✓ Good: snake_case tables and columns (matches Rust convention)
CREATE TABLE task_dispatches (
    id          TEXT PRIMARY KEY,
    agent_id    TEXT NOT NULL,
    status      TEXT NOT NULL,
    created_at  TEXT NOT NULL
);
```

**API naming:**

```
# ✓ Good: kebab-case paths, snake_case JSON fields
POST /api/v1/task-dispatches
GET  /api/v1/task-dispatches/{id}

{
    "agent_id": "01234567-...",
    "task_status": "pending",
    "created_at": "2025-01-15T09:00:00Z"
}
```

**Serde configuration:**

```rust
// ✓ Good: Explicit serde rename for API consistency
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Pending,
    Running,
    Completed,
    Failed,
}
```

**Summary:**

| Layer | Convention | Example |
|-------|-----------|---------|
| Crate names | `kebab-case` | `myapp-domain` |
| Modules/files | `snake_case` | `task_dispatch.rs` |
| Types/traits | `PascalCase` | `TaskDispatch` |
| Functions | `snake_case` | `create_dispatch()` |
| Constants | `SCREAMING_SNAKE_CASE` | `MAX_RETRY_COUNT` |
| Error types | `{Domain}Error` | `DispatchError` |
| Feature flags | `kebab-case` | `postgres-integration` |
| DB tables/columns | `snake_case` | `task_dispatches` |
| API paths | `kebab-case` | `/task-dispatches` |
| JSON fields | `snake_case` | `agent_id` |

**Why:** Predictable naming reduces cognitive load. Developers can infer the name of a table from its Rust type, or an API path from a module name, without looking it up.
