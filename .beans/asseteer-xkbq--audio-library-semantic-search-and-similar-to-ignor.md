---
# asseteer-xkbq
title: 'Audio library: semantic search and ''similar to'' ignore selected folder filter'
status: todo
type: bug
created_at: 2026-03-19T11:40:57Z
updated_at: 2026-03-19T11:40:57Z
parent: asseteer-kvnt
---

In the audio Library view, when semantic search is active AND a specific folder is selected in the sidebar, the folder filter is ignored — search runs across the entire library. Same issue with 'similar to' search. Non-semantic (text) search respects the folder filter correctly. The folder condition is likely not being passed through to the semantic/embedding query path.
