#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "requests>=2.28.0",
# ]
# ///
"""
Nested ZIP Benchmark for CLAP Processing

Measures performance of processing audio files inside nested ZIPs
(ZIP containing ZIPs), comparing different access strategies.

Usage:
    uv run benchmark_nested.py <nested_zip> [--count N]
"""

import argparse
import io
import os
import statistics
import sys
import time
import zipfile
from pathlib import Path

import requests

SERVER_URL = "http://127.0.0.1:5555"
AUDIO_EXTENSIONS = {".wav", ".mp3", ".flac", ".ogg", ".m4a", ".aac"}


def check_server():
    try:
        r = requests.get(f"{SERVER_URL}/health", timeout=3)
        info = r.json()
        print(f"Server: {info['model']} on {info['device']}")
        return True
    except Exception:
        return False


def discover_nested_audio(zip_path: Path, max_per_inner: int = 0):
    """Discover audio files inside nested ZIPs. Returns list of (inner_zip_name, audio_entry_name)."""
    entries = []
    with zipfile.ZipFile(zip_path) as outer:
        for inner_name in sorted(outer.namelist()):
            if not inner_name.lower().endswith('.zip'):
                continue
            inner_bytes = outer.read(inner_name)
            inner_zf = zipfile.ZipFile(io.BytesIO(inner_bytes))
            count = 0
            for audio_name in sorted(inner_zf.namelist()):
                ext = os.path.splitext(audio_name)[1].lower()
                if ext in AUDIO_EXTENSIONS and "__MACOSX" not in audio_name:
                    entries.append((inner_name, audio_name))
                    count += 1
                    if max_per_inner and count >= max_per_inner:
                        break
            inner_zf.close()
    return entries


def benchmark_naive_reopen(zip_path: Path, entries: list[tuple[str, str]]):
    """Worst case: reopen outer ZIP + decompress inner ZIP for every single file."""
    times_total = []
    times_outer_read = []
    times_inner_open = []
    times_inner_extract = []
    times_upload = []
    errors = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: Naive nested ZIP (reopen everything per file)")
    print(f"Files: {len(entries)}")
    print(f"{'='*60}")

    for i, (inner_name, audio_name) in enumerate(entries):
        t0 = time.perf_counter()

        # Open outer ZIP and read inner ZIP bytes
        try:
            outer = zipfile.ZipFile(zip_path)
            inner_bytes = outer.read(inner_name)
            outer.close()
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] outer read: {e}")
            continue
        t1 = time.perf_counter()

        # Open inner ZIP from bytes
        try:
            inner_zf = zipfile.ZipFile(io.BytesIO(inner_bytes))
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] inner open: {e}")
            continue
        t2 = time.perf_counter()

        # Extract audio from inner ZIP
        try:
            audio_bytes = inner_zf.read(audio_name)
            inner_zf.close()
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] inner extract: {e}")
            continue
        t3 = time.perf_counter()

        # Upload to CLAP server
        try:
            filename = os.path.basename(audio_name)
            r = requests.post(
                f"{SERVER_URL}/embed/audio/upload",
                files={"audio": (filename, io.BytesIO(audio_bytes), "audio/wav")},
                timeout=60,
            )
            r.raise_for_status()
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] upload: {e}")
            continue
        t4 = time.perf_counter()

        times_outer_read.append(t1 - t0)
        times_inner_open.append(t2 - t1)
        times_inner_extract.append(t3 - t2)
        times_upload.append(t4 - t3)
        times_total.append(t4 - t0)

        print(
            f"  [{i+1}/{len(entries)}] outer={t1-t0:.3f}s inner_open={1000*(t2-t1):.1f}ms "
            f"extract={1000*(t3-t2):.1f}ms upload={t4-t3:.3f}s total={t4-t0:.3f}s"
        )

    return {
        "label": "Naive nested (reopen all per file)",
        "times_total": times_total,
        "times_outer_read": times_outer_read,
        "times_inner_open": times_inner_open,
        "times_inner_extract": times_inner_extract,
        "times_upload": times_upload,
        "errors": errors,
    }


def benchmark_cached_inner(zip_path: Path, entries: list[tuple[str, str]]):
    """Cache decompressed inner ZIPs in memory. Only re-read from outer when inner ZIP changes."""
    times_total = []
    times_outer_read = []
    times_inner_extract = []
    times_upload = []
    errors = 0
    cache_hits = 0
    cache_misses = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: Cached inner ZIP (keep decompressed inner in memory)")
    print(f"Files: {len(entries)}")
    print(f"{'='*60}")

    cached_inner_name = None
    cached_inner_zf = None
    outer = zipfile.ZipFile(zip_path)

    for i, (inner_name, audio_name) in enumerate(entries):
        t0 = time.perf_counter()

        # Read inner ZIP (cached or fresh)
        if inner_name != cached_inner_name:
            cache_misses += 1
            try:
                inner_bytes = outer.read(inner_name)
                if cached_inner_zf:
                    cached_inner_zf.close()
                cached_inner_zf = zipfile.ZipFile(io.BytesIO(inner_bytes))
                cached_inner_name = inner_name
            except Exception as e:
                errors += 1
                print(f"  ERROR [{i+1}] read inner: {e}")
                continue
        else:
            cache_hits += 1

        t1 = time.perf_counter()
        times_outer_read.append(t1 - t0)

        # Extract audio
        try:
            audio_bytes = cached_inner_zf.read(audio_name)
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] extract: {e}")
            continue
        t2 = time.perf_counter()
        times_inner_extract.append(t2 - t1)

        # Upload
        try:
            filename = os.path.basename(audio_name)
            r = requests.post(
                f"{SERVER_URL}/embed/audio/upload",
                files={"audio": (filename, io.BytesIO(audio_bytes), "audio/wav")},
                timeout=60,
            )
            r.raise_for_status()
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] upload: {e}")
            continue
        t3 = time.perf_counter()
        times_upload.append(t3 - t2)
        times_total.append(t3 - t0)

        status = "MISS" if t1 - t0 > 0.01 else "hit"
        print(
            f"  [{i+1}/{len(entries)}] cache={status} outer={t1-t0:.3f}s "
            f"extract={1000*(t2-t1):.1f}ms upload={t3-t2:.3f}s total={t3-t0:.3f}s"
        )

    if cached_inner_zf:
        cached_inner_zf.close()
    outer.close()

    return {
        "label": "Cached inner ZIP (outer open once, inner cached)",
        "times_total": times_total,
        "times_outer_read": times_outer_read,
        "times_inner_extract": times_inner_extract,
        "times_upload": times_upload,
        "cache_hits": cache_hits,
        "cache_misses": cache_misses,
        "errors": errors,
    }


def benchmark_cached_batched(zip_path: Path, entries: list[tuple[str, str]], batch_size: int = 4):
    """Cache inner ZIPs + batch upload multiple files at once."""
    times_total = []
    times_extract_batch = []
    times_upload_batch = []
    errors = 0
    cache_hits = 0
    cache_misses = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: Cached inner + batch upload (batch_size={batch_size})")
    print(f"Files: {len(entries)}")
    print(f"{'='*60}")

    cached_inner_name = None
    cached_inner_zf = None
    outer = zipfile.ZipFile(zip_path)

    # Process in batches, but respect inner ZIP boundaries
    i = 0
    while i < len(entries):
        # Collect a batch from the same inner ZIP (or across if cached)
        batch_entries = []
        batch_bytes = []
        batch_start = time.perf_counter()

        while len(batch_entries) < batch_size and i < len(entries):
            inner_name, audio_name = entries[i]

            # Load inner ZIP if needed
            if inner_name != cached_inner_name:
                cache_misses += 1
                try:
                    inner_bytes = outer.read(inner_name)
                    if cached_inner_zf:
                        cached_inner_zf.close()
                    cached_inner_zf = zipfile.ZipFile(io.BytesIO(inner_bytes))
                    cached_inner_name = inner_name
                except Exception as e:
                    errors += 1
                    print(f"  ERROR read inner: {e}")
                    i += 1
                    continue
            else:
                cache_hits += 1

            try:
                audio_bytes = cached_inner_zf.read(audio_name)
                batch_entries.append((inner_name, audio_name))
                batch_bytes.append(audio_bytes)
            except Exception as e:
                errors += 1
                print(f"  ERROR extract: {e}")
            i += 1

        if not batch_bytes:
            continue

        t_extract = time.perf_counter()
        extract_time = t_extract - batch_start

        # Batch upload
        try:
            files_payload = [
                ("files", (os.path.basename(entry[1]), io.BytesIO(data), "audio/wav"))
                for entry, data in zip(batch_entries, batch_bytes)
            ]
            r = requests.post(
                f"{SERVER_URL}/embed/audio/batch/upload",
                files=files_payload,
                timeout=120,
            )
            r.raise_for_status()
        except Exception as e:
            errors += len(batch_bytes)
            print(f"  ERROR batch upload: {e}")
            continue

        t_upload = time.perf_counter()
        upload_time = t_upload - t_extract
        total_time = t_upload - batch_start
        per_file = total_time / len(batch_bytes)

        times_extract_batch.append(extract_time)
        times_upload_batch.append(upload_time)
        times_total.extend([per_file] * len(batch_bytes))

        batch_num = (i + batch_size - 1) // batch_size
        print(
            f"  [files {i-len(batch_bytes)+1}-{i}/{len(entries)}] "
            f"extract={extract_time:.3f}s upload={upload_time:.3f}s "
            f"total={total_time:.3f}s ({per_file*1000:.1f}ms/file)"
        )

    if cached_inner_zf:
        cached_inner_zf.close()
    outer.close()

    return {
        "label": f"Cached inner + batch upload (batch_size={batch_size})",
        "times_total": times_total,
        "times_extract_batch": times_extract_batch,
        "times_upload_batch": times_upload_batch,
        "cache_hits": cache_hits,
        "cache_misses": cache_misses,
        "errors": errors,
    }


def print_stats(label, times, unit="s"):
    if not times:
        print(f"  {label}: no data")
        return
    multiplier = 1000 if unit == "ms" else 1
    fmt = ".1f" if unit == "ms" else ".3f"
    vals = [t * multiplier for t in times]
    total = sum(times)
    print(f"  {label}:")
    print(f"    mean={statistics.mean(vals):{fmt}}{unit}  "
          f"median={statistics.median(vals):{fmt}}{unit}  "
          f"min={min(vals):{fmt}}{unit}  "
          f"max={max(vals):{fmt}}{unit}  "
          f"total={total:.1f}s")


def main():
    parser = argparse.ArgumentParser(description="Benchmark nested ZIP CLAP processing")
    parser.add_argument("zipfile", help="Path to nested ZIP (ZIP containing ZIPs)")
    parser.add_argument("--count", "-n", type=int, default=0,
                        help="Max audio files per inner ZIP (0=all)")
    args = parser.parse_args()

    zip_path = Path(args.zipfile)
    if not zip_path.is_file():
        print(f"Error: {zip_path} is not a file")
        sys.exit(1)

    print("Checking CLAP server...")
    if not check_server():
        print("ERROR: CLAP server not running")
        sys.exit(1)

    print(f"\nDiscovering audio in nested ZIP: {zip_path.name}")
    entries = discover_nested_audio(zip_path, max_per_inner=args.count)
    print(f"Found {len(entries)} audio files across nested ZIPs")

    # Show structure
    inner_zips = {}
    for inner_name, audio_name in entries:
        inner_zips.setdefault(inner_name, []).append(audio_name)
    for name, files in inner_zips.items():
        print(f"  {name}: {len(files)} files")

    # Warmup
    if entries:
        print("\nWarmup (1 file)...")
        inner_name, audio_name = entries[0]
        with zipfile.ZipFile(zip_path) as outer:
            inner_bytes = outer.read(inner_name)
            inner_zf = zipfile.ZipFile(io.BytesIO(inner_bytes))
            audio_bytes = inner_zf.read(audio_name)
            inner_zf.close()
        requests.post(
            f"{SERVER_URL}/embed/audio/upload",
            files={"audio": ("warmup.wav", io.BytesIO(audio_bytes), "audio/wav")},
            timeout=60,
        )

    results = []

    # Benchmark 1: Naive (reopen everything per file) - limit to first inner ZIP
    # to avoid spending too long
    first_inner = entries[0][0] if entries else None
    naive_entries = [(i, a) for i, a in entries if i == first_inner][:8]
    if naive_entries:
        results.append(benchmark_naive_reopen(zip_path, naive_entries))

    # Benchmark 2: Cached inner ZIP (all files)
    if entries:
        results.append(benchmark_cached_inner(zip_path, entries))

    # Benchmark 3: Cached + batch upload (batch sizes 4 and 8)
    for bs in [4, 8]:
        if entries:
            results.append(benchmark_cached_batched(zip_path, entries, batch_size=bs))

    # Summary
    print(f"\n{'='*60}")
    print(f"SUMMARY")
    print(f"{'='*60}")

    for r in results:
        print(f"\n{r['label']} (errors: {r['errors']})")
        if "times_total" in r:
            print_stats("per file total", r["times_total"])
        if "times_outer_read" in r:
            print_stats("outer ZIP read", r["times_outer_read"])
        if "times_inner_open" in r:
            print_stats("inner ZIP open", r["times_inner_open"], unit="ms")
        if "times_inner_extract" in r:
            print_stats("inner extract", r["times_inner_extract"], unit="ms")
        if "times_upload" in r:
            print_stats("upload+inference", r["times_upload"])
        if "times_extract_batch" in r:
            print_stats("batch extract", r["times_extract_batch"])
        if "times_upload_batch" in r:
            print_stats("batch upload+inference", r["times_upload_batch"])
        if "cache_hits" in r:
            print(f"  cache: {r['cache_hits']} hits, {r['cache_misses']} misses")

    # Comparison
    naive = next((r for r in results if "Naive" in r["label"]), None)
    cached = next((r for r in results if "Cached" in r["label"]), None)

    if naive and cached and naive["times_total"] and cached["times_total"]:
        naive_mean = statistics.mean(naive["times_total"])
        cached_mean = statistics.mean(cached["times_total"])
        naive_outer = statistics.mean(naive["times_outer_read"])
        cached_outer = statistics.mean(cached["times_outer_read"])

        print(f"\n{'='*60}")
        print(f"COMPARISON")
        print(f"{'='*60}")
        print(f"  Naive (reopen all):  {naive_mean*1000:.1f}ms/file  (outer read: {naive_outer*1000:.1f}ms)")
        print(f"  Cached inner:        {cached_mean*1000:.1f}ms/file  (outer read: {cached_outer*1000:.1f}ms)")
        print(f"  Savings:             {(naive_mean - cached_mean)*1000:.1f}ms/file ({(1 - cached_mean/naive_mean)*100:.0f}% faster)")
        print(f"\n  For 64 files across 4 inner ZIPs:")
        print(f"    Naive:  {naive_mean * 64:.1f}s")
        print(f"    Cached: {cached_mean * 64:.1f}s")


if __name__ == "__main__":
    main()
