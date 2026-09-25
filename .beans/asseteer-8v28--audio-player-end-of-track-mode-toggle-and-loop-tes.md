---
# asseteer-8v28
title: 'Audio player: end-of-track mode toggle and loop test button'
status: in-progress
type: feature
priority: normal
created_at: 2026-04-02T13:09:46Z
updated_at: 2026-09-25T09:22:51Z
---

Two additions to the audio player controls:

1. **End-of-track mode toggle** — a button cycling through modes for what happens when a track ends:
   - Stop
   - Play next
   - Repeat

2. **Test loop button** — sets repeat mode and seeks to ~5 seconds before the end of the track, so the loop point can be quickly auditioned.

## Progress
UI done (behaviour intentionally not wired yet):
- `settings.audioEndMode: 'stop' | 'next' | 'repeat'`, persisted. It's a 3-icon segmented radiogroup in the transport strip rather than a cycling button: the current state is visible at a glance, and any mode is one click away.
- "Test loop" button next to it. Currently it only switches the mode to Repeat.

Remaining:
- [ ] onEnded honours `audioEndMode` (next → navigate + play; repeat → loop)
- [ ] Test loop: repeat + seek to max(0, duration − 5s) + play
