---
kind: standard
name: mock-boundaries
category: testing
importance: high
applies_to:
  - "**/*test*"
  - "**/*spec*"
  - "tests/**"
applies_to_languages:
  - typescript
applies_to_domains:
  - testing
---

# Mock Boundaries in BDD Scenarios

Use MSW/network-boundary mocking in scenario step execution. Do not mock internal hooks/components to force behavior outcomes.

```gherkin
@behavior(BEH-API-002) @tier(integration)
Scenario: Profile screen renders API error state
  Given the profile API returns status 500
  When the user opens the profile page
  Then an error banner is displayed
```

```typescript
// Step setup uses MSW boundary mocks.
server.use(
  http.get('/api/profile', () => HttpResponse.json({ message: 'error' }, { status: 500 })),
);
```

**Rules:**
- Mock at network boundary with MSW
- Avoid `vi.mock()` for hooks, router internals, and query internals
- Scenario assertions must target observable UI behavior

**Why:** Boundary-level mocking preserves realistic behavior paths.
