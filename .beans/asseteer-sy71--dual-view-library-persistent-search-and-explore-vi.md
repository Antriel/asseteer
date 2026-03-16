---
# asseteer-sy71
title: 'Dual-view library: persistent Search and Explore views'
status: draft
type: feature
created_at: 2026-03-16T10:26:50Z
updated_at: 2026-03-16T10:26:50Z
---

Add two persistent, switchable views to the asset library:

**Search view** — the current search/filter experience. User searches for assets (e.g. SFX), sees results, can play/preview them.

**Explore view** — a directory tree browser. Shows the filesystem hierarchy of scanned folders, lets users navigate into directories and see what files are around a given asset. Useful when a search result looks promising and the user wants to explore nearby files in context.

The two views should be switchable (e.g. tabs), and both should be persistent — switching from Search to Explore and back doesn't lose your search query or your position in the tree.

A key workflow this enables: user searches for SFX, finds a good file, clicks to navigate to it in Explore view to see what other files are in that folder.
