#!/usr/bin/env python3
# /// script
# requires-python = ">=3.11"
# dependencies = [
#     "requests>=2.28.0",
# ]
# ///
"""
CLAP Processing Benchmark

Measures where time is spent during CLAP embedding generation:
- Pure inference time (file path -> embedding via /embed/audio)
- ZIP extraction + upload time (extract in Python, send bytes via /embed/audio/upload)
- librosa loading overhead (measured server-side, visible in logs)

Usage:
    # Start CLAP server first, then:
    python benchmark.py <extracted_folder> <zip_file> [--count N]

Example:
    python benchmark.py "../perf-test/Wild West Sound FX Pack Vol. 1" "../perf-test/Wild West Sound FX Pack Vol. 1.zip" --count 50
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
    """Check if CLAP server is running."""
    try:
        r = requests.get(f"{SERVER_URL}/health", timeout=3)
        info = r.json()
        print(f"Server: {info['model']} on {info['device']}")
        return True
    except Exception:
        return False


def find_audio_files(folder: Path, count: int) -> list[Path]:
    """Find audio files in a folder."""
    files = []
    for f in sorted(folder.rglob("*")):
        if f.suffix.lower() in AUDIO_EXTENSIONS and "__MACOSX" not in str(f):
            files.append(f)
            if len(files) >= count:
                break
    return files


def find_zip_audio_entries(zip_path: Path, count: int) -> list[str]:
    """Find audio entries in a ZIP file."""
    entries = []
    with zipfile.ZipFile(zip_path) as zf:
        for name in sorted(zf.namelist()):
            ext = os.path.splitext(name)[1].lower()
            if ext in AUDIO_EXTENSIONS and "__MACOSX" not in name:
                entries.append(name)
                if len(entries) >= count:
                    break
    return entries


def benchmark_filesystem(files: list[Path]) -> dict:
    """Benchmark: send file paths directly to /embed/audio."""
    times = []
    errors = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: Filesystem files via /embed/audio")
    print(f"Files: {len(files)}")
    print(f"{'='*60}")

    for i, f in enumerate(files):
        start = time.perf_counter()
        try:
            r = requests.post(
                f"{SERVER_URL}/embed/audio",
                json={"audio_path": str(f.resolve())},
                timeout=30,
            )
            r.raise_for_status()
            elapsed = time.perf_counter() - start
            times.append(elapsed)
        except Exception as e:
            elapsed = time.perf_counter() - start
            errors += 1
            print(f"  ERROR [{i+1}] {f.name}: {e}")
            continue

        if (i + 1) % 10 == 0 or i == 0:
            print(f"  [{i+1}/{len(files)}] {elapsed:.3f}s - {f.name}")

    return {"times": times, "errors": errors, "label": "Filesystem -> /embed/audio"}


def benchmark_zip_upload(zip_path: Path, entries: list[str]) -> dict:
    """Benchmark: extract from ZIP in Python, upload bytes to /embed/audio/upload."""
    times_total = []  # full round-trip
    times_extract = []  # just ZIP extraction
    times_upload = []  # just HTTP upload + inference
    errors = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: ZIP extraction + upload via /embed/audio/upload")
    print(f"ZIP: {zip_path.name} ({zip_path.stat().st_size / 1024 / 1024:.1f} MB)")
    print(f"Entries: {len(entries)}")
    print(f"{'='*60}")

    # Open ZIP once (best case — this is what a cache would do)
    with zipfile.ZipFile(zip_path) as zf:
        for i, entry in enumerate(entries):
            t0 = time.perf_counter()

            # Extract bytes
            try:
                audio_bytes = zf.read(entry)
            except Exception as e:
                errors += 1
                print(f"  ERROR [{i+1}] extract {entry}: {e}")
                continue

            t1 = time.perf_counter()

            # Upload bytes
            try:
                filename = os.path.basename(entry)
                r = requests.post(
                    f"{SERVER_URL}/embed/audio/upload",
                    files={"audio": (filename, io.BytesIO(audio_bytes), "audio/wav")},
                    timeout=30,
                )
                r.raise_for_status()
            except Exception as e:
                errors += 1
                print(f"  ERROR [{i+1}] upload {entry}: {e}")
                continue

            t2 = time.perf_counter()

            times_extract.append(t1 - t0)
            times_upload.append(t2 - t1)
            times_total.append(t2 - t0)

            if (i + 1) % 10 == 0 or i == 0:
                print(
                    f"  [{i+1}/{len(entries)}] extract={t1-t0:.3f}s upload={t2-t1:.3f}s total={t2-t0:.3f}s"
                )

    return {
        "times_total": times_total,
        "times_extract": times_extract,
        "times_upload": times_upload,
        "errors": errors,
        "label": "ZIP extract + /embed/audio/upload (ZIP opened once)",
    }


def benchmark_zip_reopen(zip_path: Path, entries: list[str]) -> dict:
    """Benchmark: reopen ZIP for each file (simulates current Rust behavior)."""
    times_total = []
    times_open = []  # ZIP open + central directory parse
    times_extract = []  # entry extraction
    times_upload = []
    errors = 0

    print(f"\n{'='*60}")
    print(f"BENCHMARK: ZIP reopen per file (current Rust behavior)")
    print(f"ZIP: {zip_path.name}")
    print(f"Entries: {len(entries)}")
    print(f"{'='*60}")

    for i, entry in enumerate(entries):
        t0 = time.perf_counter()

        # Open ZIP fresh each time
        try:
            zf = zipfile.ZipFile(zip_path)
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] open ZIP: {e}")
            continue

        t1 = time.perf_counter()

        # Extract bytes
        try:
            audio_bytes = zf.read(entry)
            zf.close()
        except Exception as e:
            zf.close()
            errors += 1
            print(f"  ERROR [{i+1}] extract {entry}: {e}")
            continue

        t2 = time.perf_counter()

        # Upload bytes
        try:
            filename = os.path.basename(entry)
            r = requests.post(
                f"{SERVER_URL}/embed/audio/upload",
                files={"audio": (filename, io.BytesIO(audio_bytes), "audio/wav")},
                timeout=30,
            )
            r.raise_for_status()
        except Exception as e:
            errors += 1
            print(f"  ERROR [{i+1}] upload {entry}: {e}")
            continue

        t3 = time.perf_counter()

        times_open.append(t1 - t0)
        times_extract.append(t2 - t1)
        times_upload.append(t3 - t2)
        times_total.append(t3 - t0)

        if (i + 1) % 10 == 0 or i == 0:
            print(
                f"  [{i+1}/{len(entries)}] open={1000*(t1-t0):.1f}ms extract={1000*(t2-t1):.1f}ms upload={t3-t2:.3f}s"
            )

    return {
        "times_total": times_total,
        "times_open": times_open,
        "times_extract": times_extract,
        "times_upload": times_upload,
        "errors": errors,
        "label": "ZIP reopen per file + /embed/audio/upload",
    }


def benchmark_batch_filesystem(files: list[Path], batch_sizes: list[int]) -> list[dict]:
    """Benchmark: send files in batches to /embed/audio/batch."""
    results = []

    for batch_size in batch_sizes:
        times_batch = []  # time per batch
        times_per_file = []  # amortized time per file
        errors = 0
        total_files = 0

        print(f"\n{'='*60}")
        print(f"BENCHMARK: Batch filesystem via /embed/audio/batch (batch_size={batch_size})")
        print(f"Files: {len(files)}")
        print(f"{'='*60}")

        for batch_start in range(0, len(files), batch_size):
            batch = files[batch_start:batch_start + batch_size]
            paths = [str(f.resolve()) for f in batch]

            start = time.perf_counter()
            try:
                r = requests.post(
                    f"{SERVER_URL}/embed/audio/batch",
                    json={"audio_paths": paths},
                    timeout=120,
                )
                r.raise_for_status()
                resp = r.json()
                batch_errors = sum(1 for item in resp["results"] if item.get("error"))
                errors += batch_errors
            except Exception as e:
                elapsed = time.perf_counter() - start
                errors += len(batch)
                print(f"  ERROR batch {batch_start//batch_size + 1}: {e}")
                continue

            elapsed = time.perf_counter() - start
            times_batch.append(elapsed)
            per_file = elapsed / len(batch)
            times_per_file.extend([per_file] * len(batch))
            total_files += len(batch)

            batch_num = batch_start // batch_size + 1
            total_batches = (len(files) + batch_size - 1) // batch_size
            print(
                f"  [batch {batch_num}/{total_batches}] {elapsed:.3f}s for {len(batch)} files "
                f"({per_file*1000:.1f}ms/file)"
            )

        results.append({
            "times_per_file": times_per_file,
            "times_batch": times_batch,
            "batch_size": batch_size,
            "errors": errors,
            "label": f"Batch /embed/audio/batch (size={batch_size})",
        })

    return results


def benchmark_concurrent(files: list[Path], concurrency_levels: list[int]) -> list[dict]:
    """Benchmark: fire N concurrent single-file requests to measure throughput scaling."""
    from concurrent.futures import ThreadPoolExecutor, as_completed

    results = []

    def send_request(f: Path) -> float:
        """Send a single embed request, return elapsed time."""
        start = time.perf_counter()
        r = requests.post(
            f"{SERVER_URL}/embed/audio",
            json={"audio_path": str(f.resolve())},
            timeout=60,
        )
        r.raise_for_status()
        return time.perf_counter() - start

    for concurrency in concurrency_levels:
        print(f"\n{'='*60}")
        print(f"BENCHMARK: Concurrent single requests (concurrency={concurrency})")
        print(f"Files: {len(files)}")
        print(f"{'='*60}")

        errors = 0
        per_request_times = []

        wall_start = time.perf_counter()

        with ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = {pool.submit(send_request, f): f for f in files}
            done_count = 0
            for future in as_completed(futures):
                done_count += 1
                try:
                    elapsed = future.result()
                    per_request_times.append(elapsed)
                except Exception as e:
                    errors += 1
                    print(f"  ERROR: {futures[future].name}: {e}")

                if done_count % 10 == 0 or done_count == len(files):
                    wall_so_far = time.perf_counter() - wall_start
                    throughput = done_count / wall_so_far
                    print(f"  [{done_count}/{len(files)}] {throughput:.1f} files/sec")

        wall_total = time.perf_counter() - wall_start
        throughput = len(files) / wall_total if wall_total > 0 else 0

        results.append({
            "concurrency": concurrency,
            "wall_total": wall_total,
            "throughput": throughput,
            "per_request_times": per_request_times,
            "errors": errors,
            "label": f"Concurrent single requests (workers={concurrency})",
        })

        print(f"  Wall time: {wall_total:.1f}s | Throughput: {throughput:.1f} files/sec | Errors: {errors}")

    return results


def benchmark_concurrent_batches(files: list[Path], batch_size: int, concurrency_levels: list[int]) -> list[dict]:
    """Benchmark: fire N concurrent BATCH requests to measure throughput scaling."""
    from concurrent.futures import ThreadPoolExecutor, as_completed

    results = []

    def send_batch(batch: list[Path]) -> tuple[float, int]:
        """Send a batch embed request, return (elapsed, count)."""
        paths = [str(f.resolve()) for f in batch]
        start = time.perf_counter()
        r = requests.post(
            f"{SERVER_URL}/embed/audio/batch",
            json={"audio_paths": paths},
            timeout=120,
        )
        r.raise_for_status()
        elapsed = time.perf_counter() - start
        return elapsed, len(batch)

    # Pre-split files into batches
    batches = [files[i:i + batch_size] for i in range(0, len(files), batch_size)]

    for concurrency in concurrency_levels:
        print(f"\n{'='*60}")
        print(f"BENCHMARK: Concurrent batches (batch_size={batch_size}, concurrency={concurrency})")
        print(f"Files: {len(files)} in {len(batches)} batches")
        print(f"{'='*60}")

        errors = 0
        files_done = 0

        wall_start = time.perf_counter()

        with ThreadPoolExecutor(max_workers=concurrency) as pool:
            futures = {pool.submit(send_batch, b): b for b in batches}
            batch_done = 0
            for future in as_completed(futures):
                batch_done += 1
                try:
                    elapsed, count = future.result()
                    files_done += count
                except Exception as e:
                    errors += 1
                    print(f"  ERROR batch: {e}")

                if batch_done % 3 == 0 or batch_done == len(batches):
                    wall_so_far = time.perf_counter() - wall_start
                    throughput = files_done / wall_so_far
                    print(f"  [batch {batch_done}/{len(batches)}] {throughput:.1f} files/sec")

        wall_total = time.perf_counter() - wall_start
        throughput = files_done / wall_total if wall_total > 0 else 0

        results.append({
            "concurrency": concurrency,
            "batch_size": batch_size,
            "wall_total": wall_total,
            "throughput": throughput,
            "files_processed": files_done,
            "errors": errors,
            "label": f"Concurrent batches (batch={batch_size}, workers={concurrency})",
        })

        print(f"  Wall time: {wall_total:.1f}s | Throughput: {throughput:.1f} files/sec | Errors: {errors}")

    return results


def print_stats(label: str, times: list[float], unit: str = "s"):
    """Print statistics for a list of timings."""
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


def print_summary(results: list[dict]):
    """Print comparison summary."""
    print(f"\n{'='*60}")
    print(f"SUMMARY")
    print(f"{'='*60}")

    for r in results:
        label = r["label"]
        errors = r["errors"]
        print(f"\n{label} (errors: {errors})")

        if "times" in r:
            print_stats("per file", r["times"])
        if "times_per_file" in r:
            print_stats("per file (amortized)", r["times_per_file"])
        if "times_batch" in r:
            print_stats("per batch", r["times_batch"])
        if "times_total" in r:
            print_stats("per file (total)", r["times_total"])
        if "times_open" in r:
            print_stats("ZIP open", r["times_open"], unit="ms")
        if "times_extract" in r:
            print_stats("ZIP extract", r["times_extract"], unit="ms")
        if "times_upload" in r:
            print_stats("upload+inference", r["times_upload"])

    # Comparison
    fs_result = next((r for r in results if "times" in r), None)
    zip_cached = next((r for r in results if "ZIP extract + " in r.get("label", "")), None)
    zip_reopen = next((r for r in results if "reopen" in r.get("label", "")), None)

    if fs_result and zip_reopen:
        print(f"\n{'='*60}")
        print(f"COMPARISON")
        print(f"{'='*60}")

        fs_mean = statistics.mean(fs_result["times"]) if fs_result["times"] else 0
        reopen_mean = statistics.mean(zip_reopen["times_total"]) if zip_reopen["times_total"] else 0
        upload_mean = statistics.mean(zip_reopen["times_upload"]) if zip_reopen["times_upload"] else 0
        open_mean = statistics.mean(zip_reopen["times_open"]) if zip_reopen["times_open"] else 0
        extract_mean = statistics.mean(zip_reopen["times_extract"]) if zip_reopen["times_extract"] else 0

        print(f"\n  Filesystem (path):    {fs_mean*1000:.1f}ms/file  (server loads file itself)")
        print(f"  ZIP reopen + upload:  {reopen_mean*1000:.1f}ms/file  (current Rust behavior)")
        print(f"    - ZIP open:         {open_mean*1000:.1f}ms")
        print(f"    - ZIP extract:      {extract_mean*1000:.1f}ms")
        print(f"    - upload+inference:  {upload_mean*1000:.1f}ms")
        print(f"    - ZIP overhead:     {(open_mean + extract_mean)*1000:.1f}ms ({(open_mean+extract_mean)/reopen_mean*100:.0f}% of total)")

        if zip_cached:
            cached_mean = statistics.mean(zip_cached["times_total"]) if zip_cached["times_total"] else 0
            cached_extract = statistics.mean(zip_cached["times_extract"]) if zip_cached["times_extract"] else 0
            print(f"\n  ZIP cached + upload:  {cached_mean*1000:.1f}ms/file  (ZIP opened once)")
            print(f"    - ZIP extract:      {cached_extract*1000:.1f}ms")
            print(f"    - Cache saves:      {(reopen_mean - cached_mean)*1000:.1f}ms/file ({(reopen_mean - cached_mean)/reopen_mean*100:.0f}%)")

        if fs_mean > 0:
            overhead = reopen_mean - fs_mean
            print(f"\n  ZIP overhead vs filesystem: +{overhead*1000:.1f}ms/file ({overhead/fs_mean*100:.0f}% slower)")

        # Batch results
        batch_results = [r for r in results if "times_per_file" in r]
        if batch_results:
            print(f"\n  Batch inference:")
            for br in batch_results:
                bs = br["batch_size"]
                batch_mean = statistics.mean(br["times_per_file"])
                speedup = fs_mean / batch_mean if batch_mean > 0 else 0
                print(f"    batch_size={bs:>2}: {batch_mean*1000:.1f}ms/file ({speedup:.1f}x vs single)")

        # Concurrency results
        conc_results = [r for r in results if "throughput" in r]
        if conc_results:
            print(f"\n  Concurrency throughput:")
            for cr in conc_results:
                tp = cr["throughput"]
                per_file = 1.0 / tp if tp > 0 else 0
                speedup = tp * fs_mean if fs_mean > 0 else 0
                print(f"    {cr['label']:50s}: {tp:5.1f} files/sec ({per_file*1000:.1f}ms/file, {speedup:.1f}x)")

        print(f"\n  Extrapolation (for 283,125 audio files):")
        print(f"    Single file:       {fs_mean * 283125 / 3600:.1f} hours")
        print(f"    ZIP (current):     {reopen_mean * 283125 / 3600:.1f} hours")
        if batch_results:
            for br in batch_results:
                bs = br["batch_size"]
                batch_mean = statistics.mean(br["times_per_file"])
                print(f"    Batch (size={bs:>2}):   {batch_mean * 283125 / 3600:.1f} hours")
        if conc_results:
            for cr in conc_results:
                tp = cr["throughput"]
                if tp > 0:
                    hours = 283125 / tp / 3600
                    print(f"    {cr['label']:25s}: {hours:.1f} hours")


def main():
    parser = argparse.ArgumentParser(description="Benchmark CLAP processing pipeline")
    parser.add_argument("folder", help="Path to extracted audio files")
    parser.add_argument("zipfile", help="Path to ZIP containing audio files")
    parser.add_argument("--count", "-n", type=int, default=50, help="Number of files to test (default: 50)")
    args = parser.parse_args()

    folder = Path(args.folder)
    zip_path = Path(args.zipfile)

    if not folder.is_dir():
        print(f"Error: {folder} is not a directory")
        sys.exit(1)
    if not zip_path.is_file():
        print(f"Error: {zip_path} is not a file")
        sys.exit(1)

    # Check server
    print("Checking CLAP server...")
    if not check_server():
        print("ERROR: CLAP server not running. Start it first:")
        print("  cd clap-server && venv/Scripts/python -m uvicorn clap_server:app --host 127.0.0.1 --port 5555")
        sys.exit(1)

    # Find files
    files = find_audio_files(folder, args.count)
    entries = find_zip_audio_entries(zip_path, args.count)
    print(f"\nFound {len(files)} filesystem files, {len(entries)} ZIP entries (testing {args.count})")

    # Warmup: single request to ensure model is hot
    print("\nWarmup (1 file)...")
    if files:
        requests.post(f"{SERVER_URL}/embed/audio", json={"audio_path": str(files[0].resolve())}, timeout=30)

    results = []

    # Benchmark 1: Filesystem files
    if files:
        results.append(benchmark_filesystem(files))

    # Benchmark 2: ZIP with cached handle
    if entries:
        results.append(benchmark_zip_upload(zip_path, entries))

    # Benchmark 3: ZIP with reopen per file (current behavior)
    if entries:
        results.append(benchmark_zip_reopen(zip_path, entries))

    # Benchmark 4: Batch inference at various batch sizes
    if files:
        batch_results = benchmark_batch_filesystem(files, batch_sizes=[4, 8, 16])
        results.extend(batch_results)

    # Benchmark 5: Concurrent single requests
    if files:
        conc_results = benchmark_concurrent(files, concurrency_levels=[1, 2, 4, 8])
        results.extend(conc_results)

    # Benchmark 6: Concurrent batches (best of both worlds?)
    if files:
        conc_batch_results = benchmark_concurrent_batches(files, batch_size=8, concurrency_levels=[1, 2, 4])
        results.extend(conc_batch_results)

    # Summary
    print_summary(results)


def main_multiworker():
    """Separate entry point for testing multi-process scaling.

    Starts multiple independent uvicorn instances on different ports,
    round-robins requests across them to test true multi-process parallelism.
    """
    import subprocess
    from concurrent.futures import ThreadPoolExecutor, as_completed

    parser = argparse.ArgumentParser(description="Benchmark CLAP with multiple server processes")
    parser.add_argument("folder", help="Path to extracted audio files")
    parser.add_argument("--count", "-n", type=int, default=50, help="Number of files to test")
    parser.add_argument("--workers", "-w", type=int, nargs="+", default=[1, 2, 4],
                        help="Worker counts to test (default: 1 2 4)")
    parser.add_argument("--base-port", type=int, default=5560, help="Base port (default: 5560)")
    args = parser.parse_args()

    folder = Path(args.folder)
    if not folder.is_dir():
        print(f"Error: {folder} is not a directory")
        sys.exit(1)

    files = find_audio_files(folder, args.count)
    print(f"Found {len(files)} audio files (testing {args.count})")

    venv_python = Path("venv/Scripts/python.exe")
    if not venv_python.exists():
        venv_python = Path("venv/bin/python")
    if not venv_python.exists():
        venv_python = Path("python")
    print(f"Using Python: {venv_python}")

    all_results = []

    for num_workers in args.workers:
        ports = [args.base_port + i for i in range(num_workers)]
        urls = [f"http://127.0.0.1:{p}" for p in ports]

        print(f"\n{'='*60}")
        print(f"TESTING: {num_workers} independent server process(es)")
        print(f"Ports: {ports}")
        print(f"RAM needed: ~{num_workers * 2}GB for model copies")
        print(f"{'='*60}")

        # Start N independent servers
        procs = []
        for port in ports:
            proc = subprocess.Popen(
                [
                    str(venv_python), "-m", "uvicorn",
                    "clap_server:app",
                    "--host", "127.0.0.1",
                    "--port", str(port),
                ],
                stdout=subprocess.DEVNULL,
                stderr=subprocess.DEVNULL,
            )
            procs.append(proc)

        # Wait for all servers to be ready
        print(f"Waiting for {num_workers} server(s) to load model...")
        all_ready = True
        for i, (port, url) in enumerate(zip(ports, urls)):
            ready = False
            for attempt in range(120):  # 60s max per server
                time.sleep(0.5)
                try:
                    r = requests.get(f"{url}/health", timeout=2)
                    if r.status_code == 200:
                        ready = True
                        print(f"  Server on port {port} ready")
                        break
                except Exception:
                    pass
                if procs[i].poll() is not None:
                    print(f"  Server on port {port} died (exit code {procs[i].returncode})")
                    break
            if not ready:
                all_ready = False
                break

        if not all_ready:
            print(f"Failed to start all {num_workers} servers, skipping")
            for p in procs:
                try:
                    p.terminate()
                    p.wait(timeout=5)
                except Exception:
                    p.kill()
            continue

        # Warmup all servers
        print("Warmup...")
        for url in urls:
            try:
                requests.post(f"{url}/embed/audio",
                             json={"audio_path": str(files[0].resolve())}, timeout=60)
            except Exception as e:
                print(f"  Warmup failed on {url}: {e}")

        # --- Benchmark: round-robin single requests across N servers ---
        def send_to_server(args_tuple):
            f, url = args_tuple
            start = time.perf_counter()
            r = requests.post(
                f"{url}/embed/audio",
                json={"audio_path": str(f.resolve())},
                timeout=60,
            )
            r.raise_for_status()
            return time.perf_counter() - start

        # Assign files to servers round-robin
        file_url_pairs = [(f, urls[i % len(urls)]) for i, f in enumerate(files)]

        # Test with concurrency = num_workers (one in-flight per server)
        # and 2*num_workers (two in-flight per server for pipelining)
        for concurrency in sorted(set([num_workers, num_workers * 2])):
            print(f"\n  --- {num_workers} servers, concurrency={concurrency} ---")

            errors = 0
            per_request_times = []
            wall_start = time.perf_counter()

            with ThreadPoolExecutor(max_workers=concurrency) as pool:
                futures = {pool.submit(send_to_server, pair): pair for pair in file_url_pairs}
                done_count = 0
                for future in as_completed(futures):
                    done_count += 1
                    try:
                        elapsed = future.result()
                        per_request_times.append(elapsed)
                    except Exception as e:
                        errors += 1
                        print(f"    ERROR: {e}")
                    if done_count % 10 == 0 or done_count == len(files):
                        wall_so_far = time.perf_counter() - wall_start
                        tp = done_count / wall_so_far
                        print(f"    [{done_count}/{len(files)}] {tp:.1f} files/sec")

            wall_total = time.perf_counter() - wall_start
            throughput = len(files) / wall_total if wall_total > 0 else 0

            all_results.append({
                "label": f"{num_workers} server(s), concurrent={concurrency}",
                "num_servers": num_workers,
                "concurrency": concurrency,
                "wall_total": wall_total,
                "throughput": throughput,
                "per_request_times": per_request_times,
                "errors": errors,
            })

        # --- Benchmark: concurrent batches across N servers ---
        def send_batch_to_server(args_tuple):
            batch, url = args_tuple
            paths = [str(f.resolve()) for f in batch]
            start = time.perf_counter()
            r = requests.post(
                f"{url}/embed/audio/batch",
                json={"audio_paths": paths},
                timeout=120,
            )
            r.raise_for_status()
            return time.perf_counter() - start, len(batch)

        batch_size = 8
        batches = [files[i:i + batch_size] for i in range(0, len(files), batch_size)]
        batch_url_pairs = [(b, urls[i % len(urls)]) for i, b in enumerate(batches)]

        print(f"\n  --- {num_workers} servers, batch=8, concurrency={num_workers} ---")

        errors = 0
        files_done = 0
        wall_start = time.perf_counter()

        with ThreadPoolExecutor(max_workers=num_workers) as pool:
            futures = {pool.submit(send_batch_to_server, pair): pair for pair in batch_url_pairs}
            batch_done = 0
            for future in as_completed(futures):
                batch_done += 1
                try:
                    elapsed, count = future.result()
                    files_done += count
                except Exception as e:
                    errors += 1
                    print(f"    ERROR: {e}")
                if batch_done % 3 == 0 or batch_done == len(batches):
                    wall_so_far = time.perf_counter() - wall_start
                    tp = files_done / wall_so_far
                    print(f"    [batch {batch_done}/{len(batches)}] {tp:.1f} files/sec")

        wall_total = time.perf_counter() - wall_start
        throughput = files_done / wall_total if wall_total > 0 else 0

        all_results.append({
            "label": f"{num_workers} server(s), batch=8, concurrent={num_workers}",
            "num_servers": num_workers,
            "concurrency": num_workers,
            "wall_total": wall_total,
            "throughput": throughput,
            "files_processed": files_done,
            "errors": errors,
        })

        # Stop all servers
        print(f"\nStopping {num_workers} server(s)...")
        for p in procs:
            try:
                p.terminate()
                p.wait(timeout=10)
            except Exception:
                p.kill()
                p.wait()
        print("Stopped.")

    # Print comparison
    print(f"\n{'='*60}")
    print(f"MULTI-PROCESS SCALING RESULTS")
    print(f"{'='*60}")

    baseline_tp = None
    for r in all_results:
        tp = r["throughput"]
        per_file = 1.0 / tp if tp > 0 else 0
        if baseline_tp is None:
            baseline_tp = tp
        speedup = tp / baseline_tp if baseline_tp > 0 else 0
        hours = 283125 / tp / 3600 if tp > 0 else float('inf')
        print(f"  {r['label']:50s}: {tp:5.1f} files/sec  {speedup:.1f}x  -> {hours:.1f}h for 283K")


if __name__ == "__main__":
    if "--multiworker" in sys.argv:
        sys.argv.remove("--multiworker")
        main_multiworker()
    else:
        main()
