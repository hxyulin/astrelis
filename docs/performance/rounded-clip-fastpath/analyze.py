"""Summarize unmodified harness CSV output; run from this directory."""
import csv
from pathlib import Path
from statistics import median

root = Path(__file__).resolve().parent
runs = {}
for side in ('before', 'after', 'original'):
    for path in sorted(root.glob(f'primitives-{side}-*.csv')):
        run = int(path.stem.rsplit('-', 1)[1])
        for row in csv.DictReader(path.open()):
            key = (int(row['count']), row['workload'], row['side'])
            runs.setdefault(key, {}).setdefault(side, {})[run] = float(row['record_median_us'])
with (root / 'summary.csv').open('w') as output:
    writer = csv.writer(output)
    writer.writerow(('count', 'workload', 'side', 'before_median_us', 'after_median_us',
                     'original_median_us', 'after_vs_before_pct', 'after_vs_original_pct',
                     'paired_after_vs_before_pct'))
    for key, values in sorted(runs.items()):
        before, after, original = (median(values[s].values()) for s in ('before', 'after', 'original'))
        pairs = ';'.join(f'{i}:{100*(values["after"][i]/values["before"][i]-1):.2f}'
                         for i in sorted(values['before'].keys() & values['after'].keys()))
        writer.writerow((*key, f'{before:.3f}', f'{after:.3f}', f'{original:.3f}',
                         f'{100*(after/before-1):.2f}', f'{100*(after/original-1):.2f}', pairs))
