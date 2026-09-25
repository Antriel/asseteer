---
# asseteer-8v28
title: 'Audio player: end-of-track mode toggle and loop test button'
status: completed
type: feature
priority: normal
created_at: 2026-04-02T13:09:46Z
updated_at: 2026-09-25T10:18:09Z
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
- [x] Test loop: repeat + seek to max(0, duration − 5s) + play

## Summary of Changes
- `AudioPlayer` takes a `loop` prop, bound to the native `<audio loop>`, so Repeat is gapless wherever the codec allows and `ended` never fires. Seeking past the end with → wraps to the start while looping.
- `AudioList`: in Play next mode, `onEnded` moves to the next row and autoplays it (on the last row it just stops). Test loop sets Repeat and calls the new `AudioPlayer.playFromEnd(5)`.
- Bug fixed along the way: short sounds finished with the progress bar visibly short of 100%. While playing, only the rAF loop updates `currentTime`, and the final `timeupdate` arrived while still "playing", so the bar froze at the last frame sample (e.g. 98–99% on a 0.5 s sound). `handleEnded` now snaps to `duration`.
- e2e specs in `tests/e2e/audio.spec.mjs`: bar full after a short sound ends (fails 3/3 without the fix), Play next, Repeat, Test loop.
