---
# asseteer-klss
title: 'Nested ZIP import: improve initial scan/import throughput'
status: todo
type: task
priority: normal
created_at: 2026-03-19T16:21:56Z
updated_at: 2026-03-19T16:22:10Z
parent: asseteer-kvnt
---

The processing-stage nested-ZIP slowdown is fixed, but the initial importing stage that enumerates nested ZIP contents is still serial and slower than desired. Review scan/import nested-ZIP traversal and improve throughput without reintroducing OOM risk.
