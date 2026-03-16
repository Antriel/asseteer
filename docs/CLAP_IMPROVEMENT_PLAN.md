# CLAP Integration Improvement Plan

This document outlines current pain points with the CLAP server integration and the chosen approach for improving the user experience.

---

## Current Architecture

```
┌─────────────┐    HTTP      ┌──────────────────┐
│ Tauri App   │ ────────────→│ Python FastAPI   │
│ (Rust)      │  localhost   │ (CLAP model)     │
└─────────────┘    :5555     └──────────────────┘
```

- **Server**: `clap-python-prototype/clap_server.py` (FastAPI + Uvicorn)
- **Model**: `laion/clap-htsat-fused` (~1-2GB, auto-downloaded from Hugging Face)
- **Client**: `src-tauri/src/clap/client.rs` (reqwest HTTP)
- **Lifecycle**: Rust spawns Python process on-demand, kills on app exit

---

## Current Pain Points

### 1. External Python Dependency
- Requires Python 3.9+ installed on user's system
- Requires manual venv creation and `pip install`
- Different activation commands per platform (Scripts vs bin)
- Users must run setup steps before using CLAP features

### 2. Model Download on First Use
- ~1-2GB download from Hugging Face on first startup
- No progress indication in UI (just "Starting..." for 30+ seconds)
- May timeout (60s limit) on slow connections
- Fails silently on restricted networks

### 3. Fragile Path Resolution
- Server directory found by checking `cwd` and `cwd.parent()` for `clap-python-prototype`
- Works in development but breaks if app is installed elsewhere
- No configuration option to specify custom path

### 4. Hard-Coded Server Configuration
- Port 5555 hard-coded (conflicts if already in use)
- Localhost only (no remote server option)
- 30-second HTTP timeout, 60-second startup timeout (may be insufficient)

### 5. Limited Error Feedback
- Generic "failed to start" errors don't help users debug
- Python process output goes to terminal (invisible to GUI users)
- No log file for troubleshooting

### 6. No Embedding Versioning
- `model_version` stored in DB but never checked
- Model updates would make old embeddings incompatible
- No migration path for re-processing

---

## Chosen Approach: User-Managed Python Setup

**Decision**: Keep Python as an external dependency but make setup much easier with clear documentation, setup scripts, and better error handling in the app.

**Target user**: Technical users comfortable with running a setup script. CLAP features are optional/advanced.

**Rationale**:
- Simplest to implement and maintain
- No complex bundling or porting work
- Python ML ecosystem updates easily (just `pip install --upgrade`)
- Keeps main app installer small
- CLAP is an optional power-user feature, not core functionality

### What We Ship

```
clap-server/
├── clap_server.py       # FastAPI server (existing)
├── clap_test.py         # CLAP model wrapper (existing)
├── requirements.txt     # Python dependencies (existing)
├── setup.bat            # NEW: Windows one-click setup
├── setup.sh             # NEW: Unix one-click setup
└── README.md            # NEW: Brief setup instructions
```

### Setup Scripts

**Windows (`setup.bat`)**:
```batch
@echo off
echo Setting up CLAP server...

where python >nul 2>nul
if %errorlevel% neq 0 (
    echo Python not found. Please install Python 3.9+ from python.org
    pause
    exit /b 1
)

if not exist venv (
    echo Creating virtual environment...
    python -m venv venv
)

echo Installing dependencies (this may take a few minutes)...
venv\Scripts\pip install -r requirements.txt

echo.
echo Setup complete! The CLAP server will start automatically when needed.
echo First run will download the AI model (~1-2GB).
pause
```

**Unix (`setup.sh`)**:
```bash
#!/bin/bash
echo "Setting up CLAP server..."

if ! command -v python3 &> /dev/null; then
    echo "Python not found. Please install Python 3.9+"
    exit 1
fi

if [ ! -d "venv" ]; then
    echo "Creating virtual environment..."
    python3 -m venv venv
fi

echo "Installing dependencies (this may take a few minutes)..."
venv/bin/pip install -r requirements.txt

echo ""
echo "Setup complete! The CLAP server will start automatically when needed."
echo "First run will download the AI model (~1-2GB)."
```

### Code Changes Required

#### 1. Configurable Server Path (`server.rs`)

Replace hardcoded path detection with configurable setting:

```rust
// Current: fragile path detection
let clap_dir = find_clap_directory()?; // checks cwd, parent

// New: read from config, with fallback
let clap_dir = get_clap_server_path(&app_handle)?;

fn get_clap_server_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 1. Check user-configured path in settings
    if let Some(path) = get_setting("clap_server_path") {
        return Ok(PathBuf::from(path));
    }

    // 2. Check default location relative to app
    let default = app.path().resource_dir()?.join("clap-server");
    if default.exists() {
        return Ok(default);
    }

    // 3. Legacy: check development locations
    // ... existing cwd/parent logic as fallback

    Err("CLAP server not found. Please configure the path in settings.".into())
}
```

#### 2. Better Error Messages

Replace generic errors with actionable guidance:

```rust
// Current
Err("Failed to start CLAP server".into())

// New
Err(format!(
    "CLAP server not set up. To enable semantic audio search:\n\
     1. Open the clap-server folder: {}\n\
     2. Run setup.bat (Windows) or ./setup.sh (Mac/Linux)\n\
     3. Restart the app",
    clap_dir.display()
))
```

#### 3. Settings UI Addition

Add to settings page:
- **CLAP Server Path**: Text field with folder picker
- **Open CLAP Folder**: Button to open in file explorer
- **Server Status**: Shows "Not configured" / "Ready" / "Running"

#### 4. First-Run Detection

When user tries to use semantic search without setup:

```
┌─────────────────────────────────────────────────────┐
│  Semantic Search Setup Required                      │
│                                                      │
│  To enable AI-powered audio search:                  │
│                                                      │
│  1. Install Python 3.9+ (python.org)                │
│  2. Run the setup script in: [Open Folder]          │
│  3. Restart Asseteer                                │
│                                                      │
│  First run will download the AI model (~1-2GB).     │
│                                                      │
│  [Open Setup Folder]  [Dismiss]                     │
└─────────────────────────────────────────────────────┘
```

#### 5. Model Download Progress (Nice-to-Have)

If possible, capture Python stdout during startup to show:
- "Loading model..."
- "Downloading model: 45%..." (if we can parse HF download output)

This may be tricky since HuggingFace downloads happen inside transformers library.

### Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/clap/server.rs` | Configurable path, better errors |
| `src-tauri/src/commands/settings.rs` | Add CLAP path setting |
| `src/lib/state/settings.svelte.ts` | Add CLAP path to settings state |
| `src/routes/(app)/settings/+page.svelte` | Add CLAP configuration UI |
| `src/lib/components/ClapProcessingCard.svelte` | Show setup instructions when not configured |
| `clap-server/setup.bat` | New file |
| `clap-server/setup.sh` | New file |
| `clap-server/README.md` | New file |

---

## Current Server Lifecycle & Status Display

### How Server Starts

The server uses **lazy initialization** - it only starts when needed:

1. **Semantic search** (`clap.svelte.ts` → `search()` → `ensureServer()`)
2. **CLAP processing** (`processor.rs` → `process_clap_embedding()` → `ensure_server_running()`)

**Yes, search auto-starts the server.** When a user types in semantic search mode, `ensureServer()` is called which triggers `ensure_server_running()` in Rust. First search may have 10-60s delay while server starts and loads model.

**Startup flow in `server.rs`:**
```
1. Quick health check (2s timeout) → if OK, return
2. Acquire mutex lock (double-check pattern)
3. Find clap-python-prototype directory (cwd or parent)
4. Spawn: python -m uvicorn clap_server:app --host 127.0.0.1 --port 5555
5. Poll /health every 500ms, up to 120 times (60s max)
6. Return success or timeout error
```

### Health Check Details

**Endpoint**: `GET /health`

**Response** (currently ignored except for success/fail):
```json
{
  "status": "ok",
  "model": "laion/clap-htsat-fused",
  "device": "cpu",           // or "cuda" - useful info!
  "embedding_dim": 512
}
```

**Current client behavior** (`client.rs`):
- 2-second timeout
- Only checks if request succeeds, discards response body
- No periodic monitoring after startup

### Frontend State (`clap.svelte.ts`)

| Property | Type | Usage |
|----------|------|-------|
| `serverAvailable` | boolean | Set after successful health check |
| `serverChecking` | boolean | True during `checkServer()` |
| `serverStarting` | boolean | True during `ensureServer()` |
| `isSearching` | boolean | True during search |
| `lastSearchQuery` | string | For display purposes |

**Problem**: State only updates when user triggers an action. No background monitoring.

### Current StatusBar (`StatusBar.svelte`)

Shows:
- Scan progress (if scanning)
- Processing status: "Processing X/Y" or "Paused" or "Idle"
- Category progress bars: IMG, AUD, CLAP (processing only)

**Does NOT show**:
- CLAP server status (online/offline/starting)
- Semantic search status
- Device info (CPU vs CUDA)

### Proposed StatusBar Improvements

**Add CLAP server status indicator:**

```
[Processing: Idle] ─── [CLAP: Ready (CPU)] ─── [IMG 100%] [AUD 75%] [CLAP 0%]
                            ↑
                   New: server status
```

**States to display:**
| State | Display | Color |
|-------|---------|-------|
| Not configured | `CLAP: Setup required` | Gray |
| Offline | `CLAP: Offline` | Gray |
| Starting | `CLAP: Starting...` | Yellow pulse |
| Ready (CPU) | `CLAP: Ready (CPU)` | Green |
| Ready (CUDA) | `CLAP: Ready (GPU)` | Green |
| Searching | `CLAP: Searching...` | Blue pulse |

**Optional: Show last search info:**
```
CLAP: Ready (CPU) • "footsteps" → 47 results
```

### Implementation Changes

**1. Enhance health check to return device info:**

```rust
// client.rs
#[derive(Deserialize)]
pub struct HealthInfo {
    pub status: String,
    pub model: String,
    pub device: String,
    pub embedding_dim: i32,
}

pub async fn health_check_detailed(&self) -> Result<HealthInfo, String> {
    // ... fetch and parse full response
}
```

**2. Add periodic health monitoring:**

```typescript
// clap.svelte.ts
class ClapState {
    device = $state<'cpu' | 'cuda' | null>(null);

    startHealthMonitor() {
        setInterval(async () => {
            if (this.serverAvailable) {
                const healthy = await checkClapServer();
                if (!healthy) {
                    this.serverAvailable = false;
                    this.device = null;
                }
            }
        }, 30_000); // Check every 30s
    }
}
```

**3. Update StatusBar to show CLAP status:**

```svelte
<!-- StatusBar.svelte -->
{#if clapState.serverStarting}
    <span class="text-warning">CLAP: Starting...</span>
{:else if clapState.serverAvailable}
    <span class="text-success">CLAP: Ready ({clapState.device})</span>
{:else if clapConfigured}
    <span class="text-tertiary">CLAP: Offline</span>
{:else}
    <span class="text-tertiary">CLAP: Setup required</span>
{/if}
```

### Files to Modify for Status Display

| File | Change |
|------|--------|
| `src-tauri/src/clap/client.rs` | Add `health_check_detailed()` returning device info |
| `src-tauri/src/commands/search.rs` | Expose detailed health check as Tauri command |
| `src/lib/database/queries.ts` | Add `getClapServerInfo()` function |
| `src/lib/state/clap.svelte.ts` | Add `device` state, periodic health monitor |
| `src/lib/components/layout/StatusBar.svelte` | Add CLAP status section |

---

## Quick Wins (Implement Alongside)

These improvements are low-effort and complement the main approach:

### 1. Graceful Port Handling
- Try port 5555, then 5556, 5557 if in use
- Or use port 0 (OS-assigned) and communicate back to Rust

### 2. Server Health Monitoring
- Periodic health checks while running
- Auto-restart if server crashes mid-session

### 3. Log Capture
- Redirect Python stdout/stderr to log file
- Show "View Logs" button in UI for troubleshooting

### 4. Longer Timeouts
- Increase startup timeout from 60s to 120s (model downloads can be slow)
- Make timeouts configurable in settings

---

## Future Possibilities (Decided Against for Now)

These approaches were considered but decided against due to complexity vs. benefit tradeoff. Kept here for future reference.

### PyInstaller Executable

**Concept**: Compile Python server into standalone executable (~150-250MB with CPU-only torch).

**Why not now**:
- Still requires model download (~1-2GB) separately
- PyInstaller + torch + transformers has known compatibility issues
- Adds build complexity for each platform
- Not much better UX than "run setup script"

**Revisit if**: We want to distribute to non-technical users who can't run setup scripts.

**Note on CUDA**: GPU acceleration would require separate CUDA build (~500-600MB). CUDA support is compiled into PyTorch, not loadable at runtime. For now, CPU-only is fine - CLAP inference is fast enough (50-150ms/file).

### ONNX Runtime in Rust

**Concept**: Convert CLAP to ONNX format, run inference in Rust via `ort` crate.

**Why not now**:
- CLAP export to ONNX is non-trivial (we tried, gave up)
- Audio preprocessing (librosa) would need Rust reimplementation
- HTSAT encoder has custom ops that may not export cleanly
- Multi-week effort with uncertain success

**Revisit if**: Someone publishes a working CLAP ONNX export, or we need to eliminate Python entirely.

### Pure Rust ML (Candle/Burn)

**Concept**: Port CLAP architecture to pure Rust.

**Why not now**:
- Weeks to months of development effort
- Must reimplement audio preprocessing (spectrograms, resampling)
- Model updates would require code changes
- Candle/Burn ecosystem still maturing

**Revisit if**: Rust ML ecosystem matures significantly, or CLAP becomes core differentiator worth the investment.

### Embedded Python (PyO3)

**Concept**: Embed Python interpreter in Rust, call CLAP directly without HTTP.

**Why not now**:
- Still requires Python runtime (just hidden)
- Complex build configuration
- GIL limits concurrency benefits
- HTTP overhead is negligible for our use case

**Revisit if**: HTTP latency becomes a measurable problem.

### Cloud Service

**Concept**: Host CLAP server remotely, offer as optional alternative.

**Why not now**:
- Privacy concerns (users' audio files sent to server)
- Ongoing hosting costs
- Network dependency
- Against the "local-first" philosophy

**Revisit if**: We build a SaaS version of the product.

---

## Industry Context

There's no standardized way to bundle local AI models with desktop apps yet:

- **Ollama, llama.cpp, GGML** - LLM-focused only, no audio model support
- **whisper.cpp** - Speech-to-text only, different architecture than CLAP
- **ONNX Runtime** - Generic but conversion is model-dependent
- **CoreML/DirectML** - Platform-locked

No "clap.cpp" or packaged CLAP runtime exists. The Python + HuggingFace implementation is effectively the only maintained option.

This validates our choice: work with the Python ecosystem rather than fight it.

---

## Implementation Priority

### Phase 1: Foundation
1. **Setup scripts** - `setup.bat`, `setup.sh`, `README.md`
2. **Configurable path** - Settings storage and `server.rs` changes
3. **Better error messages** - Replace generic errors with setup instructions

### Phase 2: User Experience
4. **Settings UI** - Path configuration in settings page
5. **First-run dialog** - Friendly prompt when CLAP not configured
6. **StatusBar integration** - Show server status (offline/starting/ready)

### Phase 3: Polish
7. **Detailed health check** - Return and display device info (CPU/GPU)
8. **Periodic health monitoring** - Detect server crashes
9. **Port fallback** - Handle port conflicts gracefully
10. **Log capture** - Redirect Python output to log file

---

## Open Questions

1. Should CLAP features be hidden entirely until configured, or shown with "setup required" state?
2. Do we rename `clap-python-prototype` to just `clap-server` for release?
3. Should the setup script also do a test run to pre-download the model?
