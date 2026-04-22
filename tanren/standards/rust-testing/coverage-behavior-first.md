---
kind: standard
name: coverage-behavior-first
category: rust-testing
importance: high
applies_to:
- '**/*.rs'
applies_to_languages:
- rust
applies_to_domains:
- testing
- coverage
---

Coverage is interpreted as a behavior-gap signal, not a vanity percentage. The coverage artifact must classify uncovered paths into two buckets: (1) missing-behavior — code whose absence of coverage corresponds to a behavior without a scenario, which requires adding/extending a `.feature` scenario; and (2) dead/support code — code that is not itself a behavior claim, which requires removal or explicit support-classification. Raw line/branch totals are reported but never gate on their own.
