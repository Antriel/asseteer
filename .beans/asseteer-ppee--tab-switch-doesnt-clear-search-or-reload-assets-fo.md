---
# asseteer-ppee
title: Tab switch doesn't clear search or reload assets for new tab
status: todo
type: bug
created_at: 2026-03-16T09:19:14Z
updated_at: 2026-03-16T09:19:14Z
parent: asseteer-cfrp
---

In library/+page.svelte, switching tabs via TabBar calls viewState.setActiveTab() which only updates the tab and layout mode. It does NOT trigger a new asset search/load for the new tab type. The displayed assets remain from the previous tab's search. The user must manually re-type or clear search to see results for the new tab. Expected: switching tabs should re-execute the current search query with the new asset type filter.
