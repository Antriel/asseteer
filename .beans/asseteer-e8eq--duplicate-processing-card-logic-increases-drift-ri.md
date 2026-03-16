---
# asseteer-e8eq
title: Duplicate processing card logic increases drift risk
status: todo
type: task
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-02-14T07:32:05Z
parent: asseteer-bh0n
---

src/lib/components/ProcessingCategoryCard.svelte and src/lib/components/ClapProcessingCard.svelte duplicate status mapping, control visibility, action handlers, and progress UI (e.g. statusConfig/canStart/canPause/canResume/canStop blocks around lines ~25-76 in both files). This invites inconsistent UX/behavior as features evolve. Extract shared processing-card primitives and keep CLAP-specific server controls as an extension layer.
