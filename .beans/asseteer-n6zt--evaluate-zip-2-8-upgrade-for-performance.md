---
# asseteer-n6zt
title: Evaluate zip 2 → 8 upgrade for performance
status: todo
type: task
priority: low
created_at: 2026-09-25T08:59:13Z
updated_at: 2026-09-25T08:59:13Z
---

Held back from asseteer-oodo. `zip` crate is at 2.4, latest 8.x. All archive reading (incl. nested zips, staged ZIP dispatch in the work queue) goes through it.

Only worth doing if it brings real performance gains. Check the changelog for decompression/central-directory/read-path speedups, then benchmark scan + processing on a large zip-heavy library before and after.

- [ ] review zip changelog 2.4 → 8.x for perf + API breaks
- [ ] benchmark scan/processing of a zip-heavy library on 2.4
- [ ] upgrade, fix API breaks, re-benchmark
- [ ] e2e (fixture has a nested zip) + cargo test
