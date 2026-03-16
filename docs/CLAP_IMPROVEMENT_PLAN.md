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

## Chosen Approach: `uv`-Managed Python (Primary) + Manual Fallback

**Decision**: Use `uv` (Astral's Rust-based Python package manager) to fully manage the Python environment. The app downloads `uv` on first use, then `uv` handles Python installation, dependency resolution, and environment isolation — all transparently. Manual Python setup remains as a documented fallback.

**Target user**: Any user. No Python knowledge required — just click "Enable Semantic Search" in the app.

**Rationale**:
- **Zero prerequisites**: `uv` downloads its own Python — user doesn't need Python installed
- **No ABI mismatch bugs**: `uv` pins the Python version (e.g., 3.13) regardless of system Python
- **One-click setup**: App downloads `uv` (~30MB), then `uv run` handles everything
- **Small app installer**: `uv` is downloaded on demand, not bundled
- **Keeps Python ecosystem benefits**: Still uses torch, transformers, librosa — just managed automatically
- **Graceful fallback**: Users who prefer their own Python/venv can still configure that in settings

### How It Works

```
User clicks "Enable Semantic Search"
  → App downloads uv binary (~30MB) to app data directory
  → App runs: uv run --python 3.13 clap_server.py
  → uv automatically:
      1. Downloads Python 3.13 (~25MB, cached)
      2. Creates isolated environment (cached, invisible to user)
      3. Installs dependencies from inline script metadata
      4. Runs the server
  → First inference triggers HuggingFace model download (~1-2GB)
  → All subsequent starts reuse cache — server starts in seconds
```

### Inline Script Dependencies (PEP 723)

Instead of a separate `requirements.txt`, dependencies are declared directly in `clap_server.py`:

```python
# /// script
# requires-python = ">=3.11,<3.14"
# dependencies = [
#     "torch>=2.0.0",
#     "transformers>=4.30.0",
#     "librosa>=0.10.0",
#     "soundfile>=0.13.0",
#     "numpy>=1.24.0",
#     "fastapi>=0.109.0",
#     "uvicorn[standard]>=0.27.0",
#     "python-multipart>=0.0.6",
# ]
# ///
```

Note the `<3.14` upper bound — this pins away from Python versions with known compatibility issues. When 3.14 support is confirmed for all deps, bump the bound.

### What We Ship

```
clap-server/
├── clap_server.py       # FastAPI server (with inline PEP 723 deps)
├── clap_test.py         # CLAP model wrapper (existing)
├── requirements.txt     # Kept for manual fallback users
├── setup.bat            # Manual setup (fallback)
├── setup.sh             # Manual setup (fallback)
└── README.md            # Documents both uv and manual approaches
```

### Setup Scripts

**Important: Stale venv detection.** If the user upgrades their system Python (e.g., 3.13 → 3.14), all C extension packages (numpy, cffi/soundfile, torch) will have `.pyd`/`.so` files compiled for the old Python ABI and silently fail to import. The setup scripts must detect this and rebuild the venv. This was discovered when a Python 3.13→3.14 upgrade caused `soundfile` errors that looked like MP3 format issues but were actually ABI mismatches (`_cffi_backend.cp313-win_amd64.pyd` loaded by Python 3.14).

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

:: Check if venv exists but was built with a different Python version
if exist venv (
    venv\Scripts\python -c "import sys; exit(0)" >nul 2>nul
    if %errorlevel% neq 0 (
        echo Existing venv is broken or built with a different Python version.
        echo Recreating...
        rmdir /s /q venv
    ) else (
        :: Verify C extensions work (catches Python minor version upgrades)
        venv\Scripts\python -c "import _cffi_backend" >nul 2>nul
        if %errorlevel% neq 0 (
            echo Existing venv has incompatible packages (Python version changed?).
            echo Recreating...
            rmdir /s /q venv
        )
    )
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

# Check if venv exists but was built with a different Python version
if [ -d "venv" ]; then
    if ! venv/bin/python -c "import sys; exit(0)" 2>/dev/null; then
        echo "Existing venv is broken or built with a different Python version."
        echo "Recreating..."
        rm -rf venv
    elif ! venv/bin/python -c "import _cffi_backend" 2>/dev/null; then
        echo "Existing venv has incompatible packages (Python version changed?)."
        echo "Recreating..."
        rm -rf venv
    fi
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

#### 1. `uv` Binary Management (`server.rs` or new `uv.rs`)

Download and cache the `uv` binary on first use:

```rust
fn get_or_download_uv(app: &AppHandle) -> Result<PathBuf, String> {
    let uv_dir = app.path().app_data_dir()?.join("uv");
    let uv_bin = if cfg!(windows) { uv_dir.join("uv.exe") } else { uv_dir.join("uv") };

    if uv_bin.exists() {
        return Ok(uv_bin);
    }

    // Download from https://github.com/astral-sh/uv/releases
    // ~30MB, single static binary, no dependencies
    // Use platform-appropriate URL:
    //   Windows: uv-x86_64-pc-windows-msvc.zip
    //   macOS:   uv-aarch64-apple-darwin.tar.gz
    //   Linux:   uv-x86_64-unknown-linux-gnu.tar.gz
    download_and_extract_uv(&uv_dir)?;

    Ok(uv_bin)
}
```

#### 2. Server Startup via `uv` (`server.rs`)

Replace venv-based startup with `uv run`:

```rust
// Current: find venv python, spawn uvicorn
let python_path = clap_dir.join("venv/Scripts/python.exe");
Command::new(&python_path).args(["-m", "uvicorn", "clap_server:app", ...])

// New: uv handles everything
let uv = get_or_download_uv(app)?;
Command::new(&uv)
    .args(["run", "--python", "3.13", "clap_server.py"])
    .current_dir(&clap_dir)
    .env("UV_CACHE_DIR", app.path().app_data_dir()?.join("uv-cache"))
    .spawn()
```

Key details:
- `--python 3.13` ensures uv downloads and uses exactly Python 3.13
- `UV_CACHE_DIR` keeps Python/packages in app data (clean uninstall)
- First run: uv downloads Python + deps (1-3 min). Subsequent runs: instant
- The inline PEP 723 metadata in `clap_server.py` tells uv what to install

#### 3. First-Run UX

When user enables semantic search for the first time:

```
┌─────────────────────────────────────────────────────┐
│  Setting Up Semantic Search                          │
│                                                      │
│  ✓ Downloading runtime tools...           (30MB)    │
│  ◌ Installing Python environment...       (~500MB)  │
│  ○ Downloading AI model...                (~1-2GB)  │
│                                                      │
│  This is a one-time setup. Future starts will be    │
│  instant.                                            │
│                                                      │
│  [Cancel]                                            │
└─────────────────────────────────────────────────────┘
```

No "install Python" step. No "run setup script" step. Just a progress dialog.

#### 4. Settings UI

Add to settings page:
- **Semantic Search**: Toggle to enable/disable
- **Server Status**: Shows "Not set up" / "Setting up..." / "Ready" / "Running"
- **Runtime Info**: Shows Python version, device (CPU/GPU), cache size
- **Advanced: Manual Python path**: Override for users who want their own venv (fallback)
- **Clear Cache**: Button to remove downloaded Python/packages and re-download

#### 5. Configurable Server Path (for manual fallback)

Users who prefer to manage their own Python can configure a custom path:

```rust
fn get_clap_server_path(app: &AppHandle) -> Result<PathBuf, String> {
    // 1. Check user-configured path in settings (manual override)
    if let Some(path) = get_setting("clap_server_path") {
        return Ok(PathBuf::from(path));
    }

    // 2. Default: relative to app
    let default = app.path().resource_dir()?.join("clap-server");
    if default.exists() {
        return Ok(default);
    }

    // 3. Development fallback
    // ... existing cwd/parent logic

    Err("CLAP server not found.".into())
}
```

#### 6. Better Error Messages

```rust
// uv download failed
"Failed to download runtime tools. Check your internet connection and try again."

// uv run failed (first time - dependency install)
"Failed to set up Python environment. Check the logs for details."

// Server crashed
"Semantic search server stopped unexpectedly. [Restart] [View Logs]"
```

#### 7. Model Download Progress (Nice-to-Have)

Capture Python stdout during startup to show:
- "Loading model..."
- "Downloading model: 45%..." (if we can parse HF download output)

This may be tricky since HuggingFace downloads happen inside transformers library.

### Files to Modify

| File | Change |
|------|--------|
| `src-tauri/src/clap/server.rs` | `uv`-based startup, fallback to manual venv |
| `src-tauri/src/clap/uv.rs` | NEW: `uv` binary download/management |
| `src-tauri/src/commands/settings.rs` | Add CLAP/semantic search settings |
| `src/lib/state/settings.svelte.ts` | Add semantic search state |
| `src/routes/(app)/settings/+page.svelte` | Add semantic search configuration UI |
| `src/lib/components/ClapSetupDialog.svelte` | NEW: First-run setup progress dialog |
| `clap-server/clap_server.py` | Add PEP 723 inline dependency metadata |
| `clap-server/setup.bat` | Manual fallback setup script |
| `clap-server/setup.sh` | Manual fallback setup script |
| `clap-server/README.md` | Documents both uv (automatic) and manual approaches |

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

### Docker Container

**Concept**: Ship a Dockerfile (or pre-built image) that bundles Python, dependencies, and optionally the model. Users run `docker compose up` or the app manages the container lifecycle. Could be offered as an alternative to the manual Python setup — users choose whichever they're comfortable with.

**Pros**:
- Completely isolates Python/torch/numpy version issues (no more ABI mismatch bugs)
- Reproducible environment — "works on my machine" becomes "works in the container"
- No pollution of user's system Python or PATH
- Docker Desktop is increasingly common among technical users
- App could auto-manage container lifecycle (start/stop/health check) via Docker API or CLI
- GPU passthrough possible with `--gpus` flag (nvidia-docker)
- Model can be persisted via Docker volume (download once, survives container rebuilds)

**Cons**:
- Docker Desktop is a ~500MB install, heavyweight if user doesn't already have it
- GPU passthrough is Linux-only natively; Windows requires WSL2+nvidia-docker (works but extra setup)
- Adds networking complexity (container port mapping, though localhost:5555 is straightforward)
- Image size ~3-5GB with torch+transformers (though this is a one-time pull)
- Slight overhead vs native Python, though negligible for inference workloads
- macOS Docker has no GPU passthrough at all (CPU only, which is fine for CLAP)

**Hybrid approach**: Offer both options in settings — "Python (manual)" and "Docker (managed)". The app detects which is available and guides the user. Docker path would be fully managed (app runs `docker compose up -d`, monitors health, stops on app exit). Python path stays as-is for users who prefer it or can't install Docker.

**Revisit if**: The manual Python setup keeps causing friction (version mismatches, broken venvs, platform differences). Docker would trade "Python environment hell" for "just have Docker installed."

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

This validates our choice: work with the Python ecosystem rather than fight it. The emergence of `uv` (Astral, Rust-based, ~30MB static binary) makes this much more viable — it can download and manage Python itself, eliminating the "user must have Python installed" prerequisite that was the biggest UX hurdle.

---

## Implementation Priority

### Phase 1: `uv` Integration (Foundation)
1. **`uv` binary management** - Download, cache, and version-check `uv` binary in Rust
2. **PEP 723 metadata** - Add inline dependencies to `clap_server.py`
3. **`uv run` startup** - Replace venv-based spawning in `server.rs` with `uv run --python 3.13`
4. **Better error messages** - Actionable errors for download failures, setup issues, crashes

### Phase 2: One-Click Setup UX
5. **Setup progress dialog** - Show download/install progress on first enable
6. **Settings UI** - Toggle semantic search, show status, manual path override
7. **StatusBar integration** - Show server status (offline/setting up/starting/ready)

### Phase 3: Polish
8. **Detailed health check** - Return and display device info (CPU/GPU)
9. **Periodic health monitoring** - Detect server crashes, auto-restart
10. **Port fallback** - Handle port conflicts gracefully
11. **Log capture** - Redirect Python output to log file
12. **Manual fallback scripts** - `setup.bat`/`setup.sh` for users who prefer their own Python

---

## Open Questions

1. Should CLAP features be hidden entirely until set up, or shown with "Enable" state?
2. Do we rename `clap-python-prototype` to just `clap-server` for release?
3. Should first-time setup also pre-download the HuggingFace model, or let it happen on first inference?
4. Should we offer Docker as a third option alongside `uv` (automatic) and manual Python?
5. What `uv` version to target? Pin a specific release, or always download latest?
6. Where to store `uv` cache? App data dir keeps it contained, but torch packages are large (~2GB) — should we allow users to choose the location?
