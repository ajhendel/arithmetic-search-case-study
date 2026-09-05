#!/usr/bin/env python3
"""Score gen-mac-candidate outputs at every emitted width with fixed recipes."""
import argparse
import csv
import hashlib
import json
from pathlib import Path
import subprocess


def run(args, **kwargs):
    return subprocess.run(args, check=True, text=True, **kwargs)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('directory', type=Path)
    args = parser.parse_args()
    root = args.directory
    out = root / 'physical'
    mapped = out / 'mapped'
    mapped.mkdir(parents=True, exist_ok=True)
    lib = Path('pdk/sky130_fd_sc_hd__tt_025C_1v80.lib')
    image = 'openroad/orfs:latest'
    manifest = list(csv.DictReader((root / 'manifest.tsv').open(), delimiter='\t'))
    rows = []
    for row in manifest:
        module = row['module']
        source = root / f'{module}.v'
        for recipe in ('default', 'classic'):
            top = f'{module}_{recipe}'
            recipe_arg = '' if recipe == 'default' else '-script scripts/abc/classic.script'
            command = f'read_verilog {root}/cells.v {source}; attrmap -modattr -remove keep_hierarchy=1; hierarchy -top {module}; proc; flatten; opt; techmap; opt; abc -liberty {lib} {recipe_arg}; opt_clean; rename {module} {top}; write_verilog -noattr -noexpr {mapped}/{top}.v'
            with (out / f'{top}.log').open('w') as log:
                run(['nice', '-n', '19', 'yosys', '-Q', '-p', command], stdout=log, stderr=subprocess.STDOUT)
            rows.append({**row, 'module': top, 'recipe': recipe, 'source_sha256': hashlib.sha256(source.read_bytes()).hexdigest()})
        print(f'mapped {module}', flush=True)
    run(['docker', 'run', '--platform', 'linux/amd64', '--rm', '-v', f'{Path.cwd()}:/work', '-w', '/work', '-e', f'LIB={lib}', '-e', f'MAPPED={mapped}', '-e', f'OUT={out}/sta.tsv', image, '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/sta', '-exit', 'scripts/sta_cores.tcl'])
    timing = {r['module']: r for r in csv.DictReader((out / 'sta.tsv').open(), delimiter='\t')}
    assert len(timing) == len(rows), 'stale or missing mapped netlists'
    rows = [{**r, **timing[r['module']]} for r in rows]
    with (out / 'summary.tsv').open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]), delimiter='\t', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)
    provenance = {'liberty_sha256': hashlib.sha256(lib.read_bytes()).hexdigest(),
                  'yosys': run(['yosys', '-V'], capture_output=True).stdout.strip(),
                  'docker_image': run(['docker', 'image', 'inspect', image, '--format', '{{.Id}}'], capture_output=True).stdout.strip(),
                  'genome_sha256': hashlib.sha256((root / 'genome.json').read_bytes()).hexdigest(),
                  'source_sha256': {str(p): hashlib.sha256(p.read_bytes()).hexdigest() for p in sorted(Path('src').glob('*.rs'))}}
    (out / 'provenance.json').write_text(json.dumps(provenance, indent=2) + '\n')
    lines = ['# Frozen MAC width replay\n\n', '| width | recipe | candidate ps / um2 | no-rule ps / um2 | delay change | area change | dominating controls |\n', '|---:|---|---|---|---:|---:|---:|\n']
    for width in sorted({r['width'] for r in rows}, key=int):
        for recipe in ('default', 'classic'):
            group = [r for r in rows if r['width'] == width and r['recipe'] == recipe]
            c = next(r for r in group if r['class'] == 'candidate')
            a = next(r for r in group if r['class'] == 'ablation')
            cd, ca = float(c['worst_arrival_ps']), float(c['area_um2'])
            ad, aa = float(a['worst_arrival_ps']), float(a['area_um2'])
            controls = [r for r in group if r['class'] == 'control']
            assert len(controls) == 30
            n = sum(float(r['worst_arrival_ps']) <= cd and float(r['area_um2']) <= ca and (float(r['worst_arrival_ps']) < cd or float(r['area_um2']) < ca) for r in controls)
            lines.append(f'| {width} | {recipe} | {cd:.0f} / {ca:.2f} | {ad:.0f} / {aa:.2f} | {(cd/ad-1)*100:+.2f}% | {(ca/aa-1)*100:+.2f}% | {n} |\n')
    lines.append('\nNegative changes favor the rule. All four semantic roots are included. See the frozen protocol and complete summary; these are pre-layout model estimates.\n')
    (out / 'README.md').write_text(''.join(lines))


if __name__ == '__main__':
    main()
