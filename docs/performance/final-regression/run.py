#!/usr/bin/env python3
"""Run already-built benchmarks sequentially; never invoke Cargo during timing.

Usage: python3 run.py /absolute/path/to/binary-directory /path/to/output
Directory contains primitives-baseline and current primitives/mixed_ui/rendering/
paths/points. Build and copy both revisions first, as described in the report.
"""
import datetime
import hashlib
import json
import pathlib
import subprocess
import sys
import threading
import time


def now():
    return datetime.datetime.now(datetime.timezone.utc).isoformat()


def capture(*args):
    return subprocess.check_output(args, text=True).strip()


def main():
    binaries, output = map(pathlib.Path, sys.argv[1:])
    output.mkdir(parents=True, exist_ok=True)
    specs = []
    for run in range(1, 4):
        for revision, binary in [("baseline", "primitives-baseline"), ("current", "primitives")]:
            specs.append((f"primitives-{revision}-{run}", binary,
                          ["--counts", "100,1000,10000", "--samples", "25", "--warmup", "5"]))
    for run in range(1, 4):
        specs.append((f"mixed-ui-current-{run}", "mixed_ui",
                      ["--frames", "600", "--warmup", "60", "--in-flight", "3"]))
    for binary, counts, extra in [("rendering", "1,1025", ["--warmup", "1"]),
                                  ("paths", "1,1025", ["--warmup", "1"]),
                                  ("points", "256,4097", [])]:
        specs.append((f"smoke-{binary}", binary, ["--counts", counts, "--samples", "5"] + extra))
    metadata = {
        "started_utc": now(), "source_commit": capture("git", "rev-parse", "HEAD"),
        "baseline_commit": capture("git", "rev-parse", "5e5ea9d"),
        "toolchain": capture("rustc", "-Vv"), "cargo": capture("cargo", "-V"),
        "os": capture("sw_vers"), "hardware": capture("system_profiler", "SPHardwareDataType"),
        "profile": "Cargo default bench [optimized], no profile overrides; same target directory/dependencies",
        "environment": {k: __import__("os").environ.get(k) for k in
                        ["RUSTFLAGS", "CARGO_ENCODED_RUSTFLAGS", "WGPU_BACKEND", "WGPU_VALIDATION", "CARGO_PROFILE_BENCH_OPT_LEVEL"]},
        "sha256": {}, "runs": [],
        "measurement": "CPU durations only; no GPU timestamps or presentation; validation defaults unchanged",
        "host_note": "Parent tests/clippy completed before timing. Unrelated Flux workloads and possible upstream integration builds share host; see host-load.jsonl.",
    }
    # Retain only non-identifying hardware fields (system_profiler includes serial/UUID).
    metadata["hardware"] = "\n".join(line for line in metadata["hardware"].splitlines()
                                      if any(key in line for key in ["Model Name:", "Model Identifier:", "Chip:", "Total Number of Cores:", "Memory:"]))
    sources = [pathlib.Path("Cargo.lock"), pathlib.Path("Cargo.toml"),
               pathlib.Path("crates/astrelis/Cargo.toml"),
               pathlib.Path("crates/astrelis/benches/support/clipping.rs"),
               *pathlib.Path("crates/astrelis/benches").glob("*.rs")]
    for path in sources + [binaries / name for name in {s[1] for s in specs}]:
        metadata["sha256"][str(path)] = hashlib.sha256(path.read_bytes()).hexdigest()
    def save():
        (output / "metadata.json").write_text(json.dumps(metadata, indent=2) + "\n")
    stop = threading.Event()
    def monitor():
        with (output / "host-load.jsonl").open("w") as f:
            while not stop.is_set():
                rows = capture("ps", "-axo", "pid,ppid,%cpu,comm").splitlines()[1:]
                rows.sort(key=lambda row: float(row.split(None, 3)[2]), reverse=True)
                f.write(json.dumps({"utc": now(), "loadavg": capture("sysctl", "-n", "vm.loadavg"), "top_cpu": rows[:20]}) + "\n")
                f.flush()
                stop.wait(5)
    save()
    monitor_thread = threading.Thread(target=monitor)
    monitor_thread.start()
    try:
        for label, binary, args in specs:
            entry = {"label": label, "command": [str(binaries / binary)] + args, "started_utc": now()}
            metadata["runs"].append(entry)
            save()
            print(f"{now()} starting {label}", flush=True)
            start = time.monotonic()
            with (output / f"{label}.csv").open("w") as stdout, (output / f"{label}.log").open("w") as stderr:
                result = subprocess.run(entry["command"], stdout=stdout, stderr=stderr)
            entry.update(returncode=result.returncode, elapsed_s=time.monotonic() - start, ended_utc=now())
            save()
            print(f"{now()} finished {label}: exit={result.returncode}, {entry['elapsed_s']:.2f}s", flush=True)
            if result.returncode:
                raise RuntimeError(f"{label} failed; inspect retained CSV/log")
    finally:
        stop.set()
        monitor_thread.join()
        metadata["ended_utc"] = now()
        save()


if __name__ == "__main__":
    main()
