---
# asseteer-wkyq
title: AssetList table virtual scrolling is broken with spacer rows
status: completed
type: bug
priority: normal
created_at: 2026-03-16T09:19:10Z
updated_at: 2026-03-16T14:42:29Z
parent: asseteer-cfrp
---

AssetList.svelte (lines 97-145) uses spacer <tr> elements before and after visible rows inside a <table> to simulate virtual scrolling. However, the table itself has style='height: totalHeight' which fights with the actual content height. The thead is sticky but the tbody spacers use height which doesn't reliably work in table layout — browsers may collapse or redistribute table row heights. For large lists, the scroll position will drift because the spacer row height isn't honored consistently across browsers. The translateY approach used in ImageGrid and VirtualList is more reliable.

## Summary of Changes

Replaced the `<table>` + spacer `<tr>` virtual scrolling approach with div-based layout using `transform: translateY()`, matching the reliable pattern used by ImageGrid. The header is a sticky CSS grid row, and visible rows are rendered inside an absolutely-positioned container with translateY offset. This eliminates browser-dependent table row height collapsing that caused scroll position drift.
