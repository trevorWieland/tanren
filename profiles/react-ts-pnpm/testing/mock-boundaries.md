# Mock Boundaries

Mock at the network boundary only. Never mock React hooks, component internals, or browser APIs.

```typescript
// ✓ Good: MSW handler mocks the network boundary
import { http, HttpResponse } from "msw";
import { setupServer } from "msw/node";

const handlers = [
  http.get("/api/users/:id", ({ params }) => {
    return HttpResponse.json({
      id: params.id,
      name: "Jane Doe",
      email: "jane@example.com",
    });
  }),
];

const server = setupServer(...handlers);

beforeAll(() => server.listen());
afterEach(() => server.resetHandlers());
afterAll(() => server.close());
```

```typescript
// ✗ Bad: Mocking hooks or internal modules
vi.mock("../hooks/use-auth", () => ({
  useAuth: () => ({ user: mockUser, isLoading: false }),
}));

vi.mock("@tanstack/react-query", () => ({
  useQuery: () => ({ data: mockData, isLoading: false }),
}));
```

**Rules:**
- Use **MSW** (Mock Service Worker) for all API mocking — intercept at the network level, not the import level
- Never `vi.mock()` React hooks, components, or internal modules
- Never mock browser APIs (`localStorage`, `fetch`, `IntersectionObserver`) unless absolutely unavoidable — prefer MSW for network and real APIs for the rest
- Never mock TanStack Query, TanStack Router, or other framework internals

**Test data factories:**
Use factory functions for test data — never inline object literals:

```typescript
// ✓ Good: Factory function for consistent test data
function createUser(overrides?: Partial<User>): User {
  return {
    id: crypto.randomUUID(),
    name: "Jane Doe",
    email: "jane@example.com",
    role: "member",
    ...overrides,
  };
}

const adminUser = createUser({ role: "admin" });

// ✗ Bad: Inline object literal in every test
const user = { id: "1", name: "Jane", email: "jane@example.com", role: "member" };
```

**Per-tier mock rules:**
- **Unit tests:** MSW for network. Direct dependency injection for services. Mocks are encouraged for isolation.
- **Component tests (Storybook):** MSW for network. Storybook decorators for providers (theme, i18n, router). Real component rendering.
- **Integration tests:** MSW for network. Real browser, real DOM, real routing. Minimal mocks.

**Mock verification:**
Always verify mocks were actually invoked:

```typescript
// ✓ Good: Verify the handler was called
const handler = http.get("/api/users", () => HttpResponse.json([]));
server.use(handler);

// ... run test ...
// MSW will warn if handlers aren't matched — treat warnings as errors
```

**Why:** Mocking internals creates a parallel reality where tests pass but the real code breaks. Network-level mocks (MSW) exercise the full code path — fetch calls, response parsing, error handling, state updates — while only replacing the external dependency.
