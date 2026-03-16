---
# asseteer-rvo8
title: AudioPlayer volume not synced on asset change
status: todo
type: bug
priority: low
created_at: 2026-03-16T09:19:22Z
updated_at: 2026-03-16T09:19:22Z
parent: asseteer-cfrp
---

AudioPlayer.svelte: the volume state (line 58) is initialized to 1 and bound to the range input. When a new asset loads, the audio element is recreated but audioElement.volume is never set to match the volume state. The HTML audio element defaults to 1.0, so this works by coincidence — but if the user has adjusted volume, the new audio element won't inherit it. The volume binding only writes TO the state on input, and the oninput handler (line 328) only fires on user interaction. Need to set audioElement.volume = volume when the audio element is ready (e.g., in handleCanPlay or handleLoadedMetadata).
