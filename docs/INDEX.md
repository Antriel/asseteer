# Documentation Index

Complete reference for the Asseteer project implementation.

---

## Quick Start

**First Time?** Start here:
1. Read **01-CORE-ARCHITECTURE.md** for the big picture
2. Follow **Phase 1** in `phases/01-FOUNDATION.md` to set up the project
3. Use **QUICK-REFERENCE.md** when you need to look something up quickly

**Troubleshooting?** Check **CHECKLISTS.md** for common issues and verification steps.

---

## Document Overview

### Architecture & Design

**📘 01-CORE-ARCHITECTURE.md** (This explains EVERYTHING)
- Quality tier system (Fast, Quality, Premium)
- Technology stack selection with rationale
- Performance targets and validation
- Model specifications (CLIP, PANNs, LLaVA, Audio LLMs)
- Architecture principles
- Dependencies and download sizes
- Read this first to understand the overall system

**📗 02-DATABASE-SCHEMA.md** (Database deep dive)
- Complete SQLite schema with all tables
- FTS5 virtual table for full-text search
- Pragmas and optimization settings
- Storage estimates and strategies
- Migration patterns
- Common query examples
- Reference when designing queries or debugging database issues

**📙 03-API-COMMANDS.md** (Command reference)
- All Tauri command signatures
- Request/response types
- Usage examples for each command
- Error handling patterns
- Event streaming details
- Copy-paste friendly for implementation

### Implementation Phases

**Phase 1: Foundation** (`phases/01-FOUNDATION.md`)
- Database initialization
- Basic import functionality
- Simple asset grid display
- Pagination
- Deliverable: Working app showing imported assets

**Phase 2: Search & Filtering** (`phases/02-SEARCH-FILTERING.md`)
- ML model integration (CLIP, PANNs)
- Auto-tagging system
- Sprite sheet detection
- Full-text search (FTS5)
- Vector similarity search
- Frontend search UI
- Deliverable: Complete search and filtering system

**Phase 3: Advanced Features** (`phases/03-ADVANCED-FEATURES.md`)
- Premium tier (LLaVA, Audio LLMs)
- Infinite canvas with PixiJS
- Duplicate detection
- Clustering and layout (UMAP, HDBSCAN)
- Performance optimizations
- File watching
- Settings configuration
- Deliverable: Complete production-ready application

### Quick References

**🔍 QUICK-REFERENCE.md** (Fast lookup)
- Quality tier comparison table
- Model specs (quick reference)
- Database structure summary
- API command cheat sheet
- Frontend state patterns
- Common code snippets
- Debugging tips
- Use this for quick lookups during implementation

**✓ CHECKLISTS.md** (Verification and tracking)
- Phase-by-phase checklists
- Code quality standards
- Testing requirements
- Pre-release checklist
- Verification commands
- Common issues and solutions
- Use to track progress and ensure nothing is missed

---

## Navigation by Task

### "I want to implement X, where do I look?"

**Database setup?**
→ 01-CORE-ARCHITECTURE (Database section) + 02-DATABASE-SCHEMA + Phase 1

**Add searching capability?**
→ Phase 2 (Search & Filtering) + 03-API-COMMANDS + QUICK-REFERENCE

**Make it render 10,000 items fast?**
→ 01-CORE-ARCHITECTURE (Performance section) + Phase 3 (Canvas)

**Add image AI tagging?**
→ Phase 2 (ML Model Integration) + 01-CORE-ARCHITECTURE (Models section)

**Detect duplicate files?**
→ Phase 3 (Duplicate Detection) + 01-CORE-ARCHITECTURE (Performance section)

**Create a Tauri command?**
→ 03-API-COMMANDS (template) + QUICK-REFERENCE (patterns) + relevant Phase

**Set up frontend state?**
→ QUICK-REFERENCE (Frontend patterns) + CLAUDE.md (project style)

**Optimize performance?**
→ 01-CORE-ARCHITECTURE (Performance section) + Phase 3 (Optimizations) + CHECKLISTS

---

## Key Decisions & Rationale

All major tech choices are explained in **01-CORE-ARCHITECTURE.md**:
- Why PixiJS over Konva? (60 FPS vs 23 FPS on 10K items)
- Why CLIP over alternatives? (Best accuracy/speed tradeoff)
- Why three quality tiers? (User choice: speed vs accuracy)
- Why SQLite? (Fast, queryable, offline-first)
- Why ONNX for ML? (Standardized, hardware-accelerated)

---

## Removed/Consolidated Content

### What Changed from Original Docs

**Timelines Removed**: No more estimated hours per task
- Focus on implementation, not scheduling
- You can add timelines based on your pace

**Duplicates Eliminated**:
- ✅ Single source of truth for quality tiers
- ✅ One merged database schema
- ✅ One model specs table
- ✅ One set of Tauri commands

**Inconsistencies Resolved**:
- Database schema now comprehensive and consistent
- All model specs in one place with clear tier mapping
- Architecture principles centralized

**Token Efficiency**: ~40% reduction vs original docs
- Removed repetition
- Better organization
- Clearer separation of concerns

---

## Reading Strategies

### For Quick Implementation (30 min)
1. Skim 01-CORE-ARCHITECTURE (core concepts only)
2. Read relevant Phase document
3. Reference 03-API-COMMANDS and QUICK-REFERENCE as needed

### For Complete Understanding (2-3 hours)
1. Read 01-CORE-ARCHITECTURE in full
2. Read 02-DATABASE-SCHEMA in full
3. Skim all Phase documents to understand flow
4. Read relevant Phase in detail
5. Use QUICK-REFERENCE for quick lookups

### For Debugging (5-10 min)
1. Check CHECKLISTS for common issues
2. Reference QUICK-REFERENCE for commands/patterns
3. Check relevant Phase for implementation details
4. Look up specific API in 03-API-COMMANDS

---

## File Structure

```
docs/
├── INDEX.md (this file)
├── 01-CORE-ARCHITECTURE.md
├── 02-DATABASE-SCHEMA.md
├── 03-API-COMMANDS.md
├── QUICK-REFERENCE.md
├── CHECKLISTS.md
└── phases/
    ├── 01-FOUNDATION.md
    ├── 02-SEARCH-FILTERING.md
    └── 03-ADVANCED-FEATURES.md
```

---

## Document Stats

| Document | Lines | Purpose |
|----------|-------|---------|
| 01-CORE-ARCHITECTURE.md | 550 | Overall vision & decisions |
| 02-DATABASE-SCHEMA.md | 400 | Database design & queries |
| 03-API-COMMANDS.md | 450 | Tauri command reference |
| QUICK-REFERENCE.md | 350 | Fast lookup tables & snippets |
| CHECKLISTS.md | 350 | Verification & tracking |
| phases/01-FOUNDATION.md | 450 | Phase 1 implementation |
| phases/02-SEARCH-FILTERING.md | 600 | Phase 2 implementation |
| phases/03-ADVANCED-FEATURES.md | 550 | Phase 3 implementation |
| **Total** | **~3700** | Complete reference |

**Compared to originals:** 7400 lines → 3700 lines (50% reduction)

---

## Before You Start

### Prerequisites
- Rust 1.70+
- Node.js 18+
- npm or yarn
- Git
- Code editor (VS Code recommended)

### One-Time Setup
```bash
# Clone/create project
git clone/init asseteer
cd asseteer

# Install dependencies
npm install
cd src-tauri && cargo build && cd ..

# Verify setup
npm run check:svelte
npm run check:cargo
```

### Folder Organization
- Use `src/` for frontend (Svelte)
- Use `src-tauri/src/` for backend (Rust)
- Use `docs/` for documentation (you're here!)

---

## Getting Help

### Documentation is incomplete?
- Check the original implementation plan documents in the root folder
- Look at QUICK-REFERENCE for patterns and examples

### Code won't compile?
- Check CHECKLISTS for common issues
- Verify Rust/npm versions meet prerequisites
- Use error message to search in relevant Phase doc

### Design decision unclear?
- Read 01-CORE-ARCHITECTURE section on that topic
- Check the rationale for why that approach was chosen

### Need a specific code example?
- Check QUICK-REFERENCE (Common Patterns section)
- Look in relevant Phase doc implementation
- Reference 03-API-COMMANDS for command signatures

---

## Maintenance

### Keeping Docs Updated
When you make changes:
1. Update the relevant document
2. Check for duplicated info elsewhere
3. Update QUICK-REFERENCE if applicable
4. Update CHECKLISTS if verification needed

### Version Control
Keep docs in sync with code:
- Document major features before implementation
- Update docs when API changes
- Keep examples in docs working and tested

---

## Next Steps

**Ready to start?**
→ Go to `phases/01-FOUNDATION.md`

**Need a quick reference?**
→ Use `QUICK-REFERENCE.md`

**Want to understand everything?**
→ Start with `01-CORE-ARCHITECTURE.md`

**Implementing search?**
→ Jump to `phases/02-SEARCH-FILTERING.md`

Good luck! 🚀
