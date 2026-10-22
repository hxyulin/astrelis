#!/usr/bin/env python3
"""Summarize all retained run distributions without discarding outliers."""
import csv
import json
import pathlib
import re
import statistics


def main():
    root = pathlib.Path(__file__).resolve().parent
    runs = {}
    for revision in ["baseline", "current"]:
        for run in range(1, 4):
            with (root / f"primitives-{revision}-{run}.csv").open() as f:
                runs[revision, run] = {(row["count"], row["workload"], row["side"]): row
                                       for row in csv.DictReader(f)}
    columns = ["count", "workload", "side", "metric", "baseline_us", "current_us", "delta_us", "delta_percent",
               "baseline_run_min_us", "baseline_run_max_us", "current_run_min_us", "current_run_max_us",
               "paired_delta_min_percent", "paired_delta_max_percent"]
    with (root / "primitives-summary.csv").open("w") as f:
        writer = csv.DictWriter(f, fieldnames=columns, lineterminator="\n")
        writer.writeheader()
        for key in runs["baseline", 1]:
            for metric in ["record_median_us", "record_p95_us", "cpu_total_median_us", "cpu_total_p95_us"]:
                b, c = [[float(runs[revision, i][key][metric]) for i in range(1, 4)]
                        for revision in ["baseline", "current"]]
                bm, cm = statistics.median(b), statistics.median(c)
                paired = [(y / x - 1) * 100 for x, y in zip(b, c)]
                values = [*key, metric, bm, cm, cm - bm, (cm / bm - 1) * 100,
                          min(b), max(b), min(c), max(c), min(paired), max(paired)]
                writer.writerow({column: f"{value:.3f}" if isinstance(value, float) else value
                                 for column, value in zip(columns, values)})
    mixed = []
    counters = {}
    for run in range(1, 4):
        with (root / f"mixed-ui-current-{run}.csv").open() as f:
            mixed.extend(dict(run=run, **row) for row in csv.DictReader(f))
        log = (root / f"mixed-ui-current-{run}.log").read_text()
        counters[run] = {}
        for phase, boundary, text in re.findall(r"phase=(\w+); upload_(before|after)=UploadStats \{([^}]+)\}", log):
            counters[run].setdefault(phase, {})[boundary] = {
                key: int(value) for key, value in re.findall(r"(\w+): (\d+)", text)}
    with (root / "mixed-ui-summary.csv").open("w") as f:
        writer = csv.DictWriter(f, fieldnames=["run", "phase", "cpu_stage", "samples", "p50_us", "p95_us", "p99_us", "max_us"], lineterminator="\n")
        writer.writeheader()
        writer.writerows(mixed)
    (root / "mixed-ui-counters.json").write_text(json.dumps(counters, indent=2) + "\n")


if __name__ == "__main__":
    main()
