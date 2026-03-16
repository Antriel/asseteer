---
# asseteer-e8eq
title: Duplicate processing card logic increases drift risk
status: scrapped
type: task
priority: normal
created_at: 2026-02-14T07:31:59Z
updated_at: 2026-03-16T14:36:42Z
parent: asseteer-bh0n
---

src/lib/components/ProcessingCategoryCard.svelte and src/lib/components/ClapProcessingCard.svelte duplicate status mapping, control visibility, action handlers, and progress UI (e.g. statusConfig/canStart/canPause/canResume/canStop blocks around lines ~25-76 in both files). This invites inconsistent UX/behavior as features evolve. Extract shared processing-card primitives and keep CLAP-specific server controls as an extension layer.


## Reasons for Scrapping

Only 2 consumers with meaningful behavioral differences (CLAP has server lifecycle management, different error handling, extra status states). Abstraction would be leaky or add indirection for minimal benefit. Drift risk is low with just 2 cards.
