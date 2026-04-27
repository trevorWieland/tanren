---
id: B-0004
title: Be notified when a loop completes or needs attention
personas: [solo-dev, team-dev, observer]
interfaces: [cli, api, mcp, tui]
contexts: [personal, organizational]
status: draft
supersedes: []
---

## Intent

A `solo-dev`, `team-dev`, or `observer` can be notified when an implementation
loop they care about completes or needs attention, so that they do not have to
watch it to know when action is required.

## Preconditions

- The user owns the spec, is subscribed to the loop, or has a standing
  subscription (e.g. an `observer` following a team's loops) that includes it.

## Observable outcomes

- The user receives a visual notification when the loop completes, errors, or
  pauses on a blocker.
- An auditory notification accompanies the visual one where the user's device
  supports it.
- Notifications reach the user on whichever supported device they are using,
  including a phone.
- The user can mute or unsubscribe from notifications for a loop without
  affecting the loop itself.

## Out of scope

- The transport mechanism (push, email, chat integrations).
- Routing notifications to teammates other than the owner.

## Related

- B-0003
- B-0005
