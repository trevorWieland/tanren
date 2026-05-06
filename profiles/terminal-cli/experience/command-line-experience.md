# Command-Line Experience Contracts

Use this profile when a behavior is primarily experienced through a shell
command.

## Standards

- Every behavior-surface contract names the command path, required arguments,
  optional flags, stdin behavior, stdout contract, stderr contract, and exit
  code contract.
- Human-readable output must be stable enough for review, but machine
  automation must use explicit structured output such as JSON when available.
- Help text is part of the experience contract. New commands and flags must
  include examples that match the accepted product vocabulary.
- Failure states must distinguish validation failure, permission denial,
  unavailable dependency, conflict, and unsupported action with stable exit
  codes or structured error fields.
- Long-running commands must expose progress or a bounded quiet mode. Silent
  indefinite waits are not acceptable.

## Proof Adapter

- Execute the compiled command as a real process.
- Assert stdout, stderr, exit code, and structured output.
- Store golden transcripts only for stable user-facing examples.
- Include at least one falsification case for malformed input or denied action
  when the behavior has meaningful negative coverage.
