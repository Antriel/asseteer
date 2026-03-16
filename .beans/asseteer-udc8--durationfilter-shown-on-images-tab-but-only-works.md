---
# asseteer-udc8
title: DurationFilter shown on images tab but only works for audio
status: todo
type: bug
created_at: 2026-03-16T09:19:28Z
updated_at: 2026-03-16T09:19:28Z
parent: asseteer-cfrp
---

In Toolbar.svelte:127, the DurationFilter component is conditionally rendered with 'if isAudioTab'. However, the DurationFilter.reloadWithFilter() at line 110 checks viewState.activeTab to decide the asset type, and passes the filter to loadAssets. If a user switches from audio (with filter set) to images, the filter state persists in assetsState.durationFilter but the DurationFilter UI disappears. The stale filter is then ignored for image searches (queries.ts line 46 only applies it when durationFilter is passed), so this is cosmetic — but the filter summary in assetsState is misleading when switching back.
