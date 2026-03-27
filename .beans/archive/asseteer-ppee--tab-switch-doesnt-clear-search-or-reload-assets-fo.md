---
# asseteer-ppee
title: Tab switch doesn't clear search or reload assets for new tab
status: scrapped
type: bug
priority: normal
created_at: 2026-03-16T09:19:14Z
updated_at: 2026-03-16T14:57:48Z
parent: asseteer-cfrp
---

In library/+page.svelte, switching tabs via TabBar calls viewState.setActiveTab() which only updates the tab and layout mode. It does NOT trigger a new asset search/load for the new tab type. The displayed assets remain from the previous tab's search. The user must manually re-type or clear search to see results for the new tab. Expected: switching tabs should re-execute the current search query with the new asset type filter.

## Reasons for Scrapping

The described bug does not exist in the current codebase. `TabBar.switchTab()` already calls `assetsState.loadAssets()` with the correct asset type when switching tabs, re-querying the database with the new type filter. The search text is preserved across tab switches, which is correct behavior.
