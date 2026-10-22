"""Run all hooks, preserving byte-exact pre-existing patch evidence."""
import pathlib
import subprocess

out = pathlib.Path(__file__).resolve().parent
prior_patches = {
    'docs/performance/rounded-clip-fastpath/source.patch',
    'docs/performance/rounded-clip-fastpath/original-lock-normalization.patch',
}
tracked = subprocess.check_output(['git', 'ls-files'], text=True).splitlines()
changed = subprocess.check_output(['git', 'diff', '--cached', '--name-only'], text=True).splitlines()
commands = [
    ('hooks-changed.log', ['prek', 'run', '--files', *changed]),
    ('pre-push-changed.log', ['prek', 'run', '--hook-stage', 'pre-push', '--files', *changed]),
    ('pre-push-all.log', ['prek', 'run', '--all-files', '--hook-stage', 'pre-push', '--skip', 'trailing-whitespace']),
    ('whitespace-all.log', ['prek', 'run', 'trailing-whitespace', '--files', *[p for p in tracked if p not in prior_patches]]),
]
for name, command in commands:
    with (out / name).open('w') as log:
        result = subprocess.run(command, stdout=log, stderr=log)
    print(name, result.returncode, flush=True)
    if result.returncode:
        raise SystemExit(result.returncode)
