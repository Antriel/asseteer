# Asseteer

A desktop app for searching and browsing large asset pack collections (images and audio) even directly from ZIPs (Humble Bundles anyone?).

<img width="1280" height="640" alt="image" src="https://github.com/user-attachments/assets/8aca3e41-26ff-43c5-be60-16be2d8e79bf" />

## What it does

Asset packs often come as giant zip files with hundreds or thousands of files inside. Asseteer scans your folders (including nested zips), indexes everything into a local database, and lets you search and preview assets instantly.

**Key features:**

- Browse images and audio across multiple source folders.
- Search by filename (fuzzy + full-text).
- **Semantic search for audio** — finds sounds by meaning, not just filename. Describe what you're looking for ("deep bass hit", "birds chirping in rain") and it returns the closest matches using [CLAP](https://github.com/LAION-AI/CLAP) audio embeddings. No internet required (once set up); runs a local Python server.
- Find similar sounds. Using the semantic search, you can also filter for sounds that are similar to one you pick.
- Supports zipped asset packs — assets inside `.zip` files are indexed and playable without extraction, including zips-within-zips.
- Image thumbnails generated and cached locally.
- Audio playback.

## Platform

Windows desktop app built with [Tauri 2](https://tauri.app/) (Rust backend, SvelteKit frontend).

## Dev setup

```bash
npm install
npm run tauri dev
```

Requires Rust, Node.js, and the [Tauri prerequisites](https://tauri.app/start/prerequisites/).

For semantic audio search, a Python environment with CLAP dependencies is needed (configured in Settings).
