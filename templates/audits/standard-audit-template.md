---
standard: {slug}
category: {category}
score: {0-100}
importance: {Critical|High|Medium|Low}
violations_count: {N}
date: {YYYY-MM-DD}
status: {clean|violations-found}
---

# Standards Audit: {Title}

**Standard:** `{category}/{slug}`
**Date:** {YYYY-MM-DD}
**Score:** {0-100}/100
**Importance:** {Critical|High|Medium|Low}

## Summary

Brief assessment of codebase compliance with this standard. 2-3 sentences covering the overall state: how widely is the standard followed, where are the gaps, and what's the trend.

## Violations

### Violation 1: {Short description}

- **File:** `{file_path}:{line_number}`
- **Severity:** {Critical|High|Medium|Low}
- **Evidence:**
  ```
  {relevant code snippet or command output}
  ```
- **Recommendation:** {specific fix with code example if helpful}

### Violation 2: {Short description}

(repeat pattern)

## Compliant Examples

Notable examples where the codebase follows this standard well. Useful for establishing the "right way" when fixing violations.

- `{file_path}:{line_number}` — {brief description of good pattern}

## Scoring Rationale

Explain how the 0-100 score was determined:

- **Coverage:** What percentage of relevant code follows the standard?
- **Severity:** How critical are the violations found?
- **Trend:** Are newer files more compliant than older ones?
- **Risk:** What's the practical impact of the violations?
