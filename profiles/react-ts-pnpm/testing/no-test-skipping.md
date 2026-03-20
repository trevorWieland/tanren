# No Test Skipping

Never skip tests. Tests either run and pass or run and fail.

```typescript
// ✓ Good: Test runs and validates result
test("validates user input", () => {
  const result = validateEmail("invalid");
  expect(result.success).toBe(false);
  expect(result.error).toBe("Invalid email format");
});

// ✗ Bad: Skip test instead of fixing
it.skip("validates user input", () => {
  // "will fix later" — NEVER DO THIS
  const result = validateEmail("invalid");
  expect(result.success).toBe(false);
});

// ✗ Bad: Conditional skip
test("validates user input", ({ skip }) => {
  if (!process.env.CI) skip();
  // ...
});
```

**No skipping means:**

**No `it.skip` / `describe.skip` / `xit`:**
- No "will fix later" skips
- No flaky test skips
- No platform-specific skips
- No environment-specific skips

**No `test.todo` in committed code:**
- Tests either exist and pass, or don't exist yet
- `test.todo` is for local development only — never committed

**No conditional skips:**
- No `if (!process.env.CI) skip()`
- No `if (!hasFeature) return`
- No `skipIf(platform === "linux")`

**When tests fail:**

**Fix the test or fix the code:**
- If the test is wrong: fix the test
- If the code is broken: fix the code
- If the test is flaky: make it deterministic or delete it

**Never:**
- Skip failing tests
- Comment out failing tests
- Wrap assertions in try/catch to swallow errors

**Flaky tests:**
- Identify the source of nondeterminism (timing, network, random data)
- Fix with explicit waits, MSW for network, seeded random data
- If it can't be made reliable: delete it. A flaky test is worse than no test

**CI enforcement:**
- CI must fail if any test has a skip marker
- Block PRs that introduce test skips
- Run all tiers with zero skips

**Why:** Tests either work or fail — no silent bypasses. Skipped tests rot silently and create a false sense of coverage. A test suite with skips is a test suite you can't trust.
