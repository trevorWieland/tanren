---
id: B-0062
title: Configure notification preferences and routing
personas: [solo-dev, team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev`, `team-dev`, or `observer` can configure which events notify
them, through which channels, and at what level of urgency, so that B-0004
notifications can be tuned rather than being a single on/off switch.

## Preconditions

- The user is signed into an account.

## Observable outcomes

- The user can choose, per event type (loop completed, loop errored, loop
  paused on a blocker, walk requested, attention-worthy spec state
  changes, and so on), whether they want to be notified.
- The user can choose which channels each kind of notification reaches
  them through — for example visual in the active interface, auditory,
  push to a phone — subject to what their device and account support.
- The user can set per-project or per-organization overrides, so they can
  stay quiet on one project while being alerted on another.
- Changes take effect for subsequent notifications; notifications already
  delivered or pending are not revisited.
- Muting a single loop via B-0004 continues to work alongside these
  broader preferences.

## Out of scope

- Routing someone else's notifications to the user (see B-0010 for
  assisting and B-0004 for subscribing to a specific loop).
- Digest emails, daily summaries, or scheduled reports.

## Related

- B-0004
- B-0010
