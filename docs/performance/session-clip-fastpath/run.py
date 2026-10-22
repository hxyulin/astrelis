"""Bounded release comparison. Build/freeze all executables before running."""
import datetime
import hashlib
import io
import json
import pathlib
import platform
import subprocess
import tarfile
import tempfile

out = pathlib.Path(__file__).resolve().parent
bins = pathlib.Path('/tmp/astrelis-session-clip-bin')
prior = out.parent / 'rounded-clip-fastpath'
old = json.loads((prior / 'metadata.json').read_text())

def capture(cmd):
    return subprocess.check_output(cmd, text=True, stderr=subprocess.STDOUT)

def digest(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()

def snapshot(name):
    # Keep CPU-ranked process names only, never command-line arguments.
    processes = capture(['ps', '-Ao', 'pid,pcpu,pmem,comm', '-r']).splitlines()[:16]
    (out / (name + '.host.txt')).write_text(
        datetime.datetime.now(datetime.timezone.utc).isoformat() + '\n'
        + capture(['uptime']) + '\n'.join(processes) + '\n')

parent = capture(['git', 'rev-parse', 'HEAD']).strip()
assert parent == '856caefd2875e5df9c778664b4756ccf4f107b2f'
for side, previous in [('before', 'after'), ('original', 'original')]:
    assert digest(bins / ('primitives-' + side)) == old['binary_sha256']['primitives-' + previous]
for path in ['Cargo.lock', 'crates/astrelis/benches/primitives.rs']:
    assert digest(pathlib.Path(path)) == old['source_sha256'][path]
# Reconstruct the prior fixed build's sources, verifying they equal parent HEAD.
with tempfile.TemporaryDirectory(prefix='astrelis-session-provenance-') as directory:
    archive = subprocess.check_output(['git', 'archive', old['baseline']])
    with tarfile.open(fileobj=io.BytesIO(archive)) as tar:
        tar.extractall(directory)
    subprocess.run(['git', 'apply', '--unsafe-paths', str((prior / 'source.patch').resolve())], cwd=directory, check=True)
    paths = capture(['git', 'ls-files', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']).splitlines()
    for path in paths:
        expected = subprocess.check_output(['git', 'show', parent + ':' + path])
        assert (pathlib.Path(directory) / path).read_bytes() == expected, path
original = pathlib.Path('/tmp/astrelis-clip-fastpath-original')
for path in capture(['git', 'ls-tree', '-r', '--name-only', old['original'], 'crates', 'Cargo.toml', 'rust-toolchain.toml']).splitlines():
    expected = subprocess.check_output(['git', 'show', old['original'] + ':' + path])
    if path == 'crates/astrelis/benches/primitives.rs':
        expected = pathlib.Path(path).read_bytes()
    assert (original / path).read_bytes() == expected, path
assert digest(original / 'Cargo.lock') == old['original_normalized_lock_sha256']
metadata = {
    'parent': parent, 'original': old['original'],
    'reuse_verification': 'Prior executable SHA256, reconstructed patched baseline vs parent tracked crates/manifests/toolchain, original tracked sources vs git (shared primitives overlay), and lock hashes checked before timing.',
    'platform': platform.platform(), 'os': capture(['sw_vers']),
    'hardware': capture(['sysctl', 'hw.model', 'hw.memsize', 'hw.ncpu', 'machdep.cpu.brand_string']),
    'rust': capture(['rustc', '-Vv']), 'cargo': capture(['cargo', '-V']),
    'binary_sha256': {p.name: digest(p) for p in sorted(bins.iterdir())},
    'source_sha256': {p: digest(pathlib.Path(p)) for p in capture(['git', 'ls-files', 'crates', 'Cargo.toml', 'Cargo.lock', 'rust-toolchain.toml']).splitlines()},
    'original_normalized_lock_sha256': digest(original / 'Cargo.lock'),
    'runs': [],
}
(out / 'source.patch.log').write_text(capture(['git', 'diff', '--', 'crates/astrelis/src']))

def run(name, cmd):
    snapshot(name + '-start')
    with (out / (name + '.csv')).open('w') as csv, (out / (name + '.log')).open('w') as log:
        result = subprocess.run(cmd, stdout=csv, stderr=log)
    snapshot(name + '-end')
    metadata['runs'].append({'name': name, 'command': cmd, 'exit': result.returncode})
    (out / 'metadata.json').write_text(json.dumps(metadata, indent=2) + '\n')
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)

for round in range(1, 4):
    order = ['before', 'after', 'original'] if round % 2 else ['original', 'after', 'before']
    for side in order:
        run(f'primitives-{side}-{round}', [str(bins / f'primitives-{side}'), '--counts', '100,1000,10000', '--samples', '25', '--warmup', '5'])
for name, args in [
    ('mixed_ui', ['--frames', '360', '--warmup', '60', '--in-flight', '3']),
    ('rendering', ['--counts', '1,1025', '--samples', '5', '--warmup', '1']),
    ('paths', ['--counts', '1,1025', '--samples', '5', '--warmup', '1']),
    ('points', ['--counts', '256,4097', '--samples', '5']),
]:
    run(name, [str(bins / name)] + args)
