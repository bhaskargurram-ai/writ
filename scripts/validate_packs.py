import os, pathlib, subprocess, sys, yaml

root = pathlib.Path(r'C:\Users\Bhaskar\AppData\Local\Cline\writ')
exe = root / 'target' / 'debug' / 'writ.exe'
tmp = pathlib.Path(os.environ['TEMP'])
ok = True

targets = [p for p in root.glob('packs/*/pack.yaml')] + [
    root / 'examples' / 'ci.yaml',
    root / 'examples' / 'strict.yaml',
    root / 'examples' / 'writ.yaml',
]
for p in targets:
    doc = yaml.safe_load(p.read_text(encoding='utf-8'))
    if 'default' not in doc:  # pack file -> wrap into a policy
        doc = {'version': 1, 'default': 'ask', 'rules': doc['rules']}
    probe = tmp / f'packval_{p.parent.name}_{p.name}'
    probe.write_text(yaml.safe_dump(doc), encoding='utf-8')
    out = subprocess.run(
        [str(exe), 'doctor', '--policy', str(probe), '--ledger', str(tmp / 'none.jsonl')],
        capture_output=True, text=True).stdout
    line = next((l for l in out.splitlines() if l.startswith('policy')), '')
    good = 'FAILED' not in line
    ok &= good
    print(('PASS' if good else 'FAIL'), p.name, '->', line.strip()[:90])

sys.exit(0 if ok else 1)
