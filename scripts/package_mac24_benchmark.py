#!/usr/bin/env python3
"""Export a standalone W=24 benchmark with all matched controls and hashes."""
import argparse
import hashlib
import io
import json
from pathlib import Path
import subprocess
import tarfile


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('output', nargs='?', default='/tmp/tinytapeout2-mac24.tar.gz')
    args = parser.parse_args()
    repo = Path(__file__).resolve().parents[1]
    root = repo / 'results/discovery/mac_crosswidth_20260905/evo608_heldout'
    files = {p.name: p for p in (repo / 'benchmarks/mac24').iterdir() if p.is_file()}
    netlists = sorted(root.glob('mul24_mac_replay_*.v'))
    assert len(netlists) == 32, 'expected candidate, sibling, and all 30 controls'
    files.update({f'rtl/{p.name}': p for p in [root / 'cells.v', *netlists]})
    files['LICENSE'] = repo / 'LICENSE'
    files['genome.json'] = root / 'genome.json'
    files['PROTOCOL.md'] = root / 'PROTOCOL.md'
    files['PROOF.md'] = repo / 'docs/MAC_COMPOSED_PROOF.md'
    files['prove_mac_composed.py'] = repo / 'scripts/prove_mac_composed.py'
    proof = root.parent / 'evo608_composed_formal_strict'
    files['proof/summary.json'] = proof / 'summary.json'
    for name in ('cell_lemmas', 'mul24_mac_replay_candidate', 'mul24_mac_replay_norules'):
        for p in (proof / name).iterdir():
            if p.is_file():
                files[f'proof/{name}/{p.name}'] = p
    mapped_proof = root.parent / 'evo608_mapped_equivalence'
    files['mapped_proof/summary.json'] = mapped_proof / 'summary.json'
    files['mapped_proof/README.md'] = mapped_proof / 'README.md'
    for folder in mapped_proof.glob('mul24_*'):
        for p in folder.iterdir():
            if p.is_file():
                files[f'mapped_proof/{folder.name}/{p.name}'] = p
    for sub in ('', 'route24_default_2cores/'):
        physical = root / 'physical' / sub
        for name in ('summary.tsv', 'README.md', 'provenance.json', 'protocol.json', 'selection.tsv'):
            if (physical / name).is_file():
                files[f'evidence/{sub}{name}'] = physical / name
    challenge = repo / 'results/discovery/mac_ufomac_challenge_20260905'
    controls = sorted((challenge / 'rtl').glob('*.v'))
    assert len(controls) == 6
    files.update({f'rtl/{p.name}': p for p in controls})
    for name in ('RESULTS.md', 'protocol.json', 'environment.json', 'mapped_summary.tsv',
                 'routed_summary.tsv', 'default_combined_frontier.tsv', 'mapped_proof.json',
                 'compressor_assignment_49.status.json', 'ufomac_router_TinyBoundedHiGHS_49.status.json'):
        files[f'evidence/ufomac/{name}'] = challenge / name
    files['evidence/ufomac/ADAPTER.md'] = repo / 'integrations/arithdas/README.md'
    files['licenses/ARITH-DAS-MIT.txt'] = repo / 'integrations/arithdas/vendor/LICENSE'
    for p in (challenge / 'formal').rglob('*'):
        if p.is_file():
            files[f'proof/ufomac/{p.relative_to(challenge / "formal")}'] = p
    # GitHub/Zenodo source archives have no .git directory. Keep provenance
    # explicit instead of making the documented archive quick-start fail.
    revision = subprocess.run(['git', 'rev-parse', 'HEAD'], cwd=repo, text=True,
                              stdout=subprocess.PIPE, stderr=subprocess.DEVNULL)
    source_manifest = repo / 'SOURCE_MANIFEST.json'
    research_revision = (json.loads(source_manifest.read_text()).get('source_commit')
                         if source_manifest.is_file() else None)
    manifest = {
        'base_commit': revision.stdout.strip() if revision.returncode == 0 else None,
        'source_research_commit': research_revision,
        'snapshot': 'Working-tree benchmark export; not a tagged or public release.',
        'files': {name: hashlib.sha256(p.read_bytes()).hexdigest() for name, p in sorted(files.items())},
    }
    output = Path(args.output).resolve()
    output.parent.mkdir(parents=True, exist_ok=True)
    with tarfile.open(output, 'w:gz') as archive:
        for name, path in sorted(files.items()):
            archive.add(path, arcname=f'mac24/{name}')
        data = (json.dumps(manifest, indent=2) + '\n').encode()
        info = tarfile.TarInfo('mac24/MANIFEST.json')
        info.size = len(data)
        archive.addfile(info, io.BytesIO(data))
    with tarfile.open(output) as archive:
        for name, digest in manifest['files'].items():
            assert hashlib.sha256(archive.extractfile(f'mac24/{name}').read()).hexdigest() == digest
    print(f'Verified {len(files)} file hashes: {output}')


if __name__ == '__main__':
    main()
