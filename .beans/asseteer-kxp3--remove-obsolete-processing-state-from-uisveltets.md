---
# asseteer-kxp3
title: Remove obsolete processing state from ui.svelte.ts
status: todo
type: task
priority: normal
created_at: 2026-03-20T11:44:57Z
updated_at: 2026-03-20T11:44:57Z
parent: asseteer-38rb
---

`src/lib/state/ui.svelte.ts` contains `isProcessing` and `processProgress` fields (lines 28-29) that appear to be leftover from before the category-based processing system was built in `tasks.svelte.ts`.

The real processing state lives in `tasks.svelte.ts` (ProcessingState class with per-category progress, event listeners, etc.). The ui.svelte.ts fields are unused but create confusion about which is the source of truth.

**Action:** Remove `isProcessing` and `processProgress` from UIState class, verify no references remain.
