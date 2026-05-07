# Terminal UI Experience Contracts

Use this profile when a behavior is primarily experienced through a full-screen
terminal UI.

## Standards

- Every behavior-surface contract names the screen, entry path, focus model,
  keyboard map, resize behavior, success state, and recovery path.
- Keyboard-only operation is mandatory. Mouse support may be additive, but it
  cannot be the only path to complete behavior.
- Focus, selection, disabled state, loading state, stale state, and permission
  denial must be visually distinguishable in monochrome terminals.
- Terminal resize must preserve the user's current context or provide a clear
  recovery state.
- Destructive actions require an explicit confirmation path that can be
  cancelled from the keyboard.

## Proof Adapter

- Drive the real TUI through a PTY.
- Assert visible screen text, focus movement, key handling, and exit behavior.
- Capture screen snapshots or golden terminal transcripts for human walk
  evidence.
- Include resize and keyboard navigation cases for complex screens.
