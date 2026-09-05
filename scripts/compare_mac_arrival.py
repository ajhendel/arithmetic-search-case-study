#!/usr/bin/env python3
"""Frozen W=24 arrival-aware control challenge, serial and resource-bounded."""
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess

from prove_mac_mapped import prove_pair

HELDOUT = Path('results/discovery/mac_crosswidth_20260905/evo608_heldout')
LIB = Path('pdk/sky130_fd_sc_hd__tt_025C_1v80.lib')
FLOW = Path('scripts/place_core.tcl')
ENV = dict(os.environ, OMP_NUM_THREADS='1', OPENBLAS_NUM_THREADS='1',
           VECLIB_MAXIMUM_THREADS='1', CARGO_BUILD_JOBS='2')


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def write_tsv(path, rows):
    with path.open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]), delimiter='\t',
                                lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)


def run(command, log, seconds):
    with log.open('w') as handle:
        subprocess.run(command, stdout=handle, stderr=subprocess.STDOUT,
                       env=ENV, check=True, timeout=seconds)


def dominates(a, b):
    d, s = 'worst_arrival_ps', 'area_um2'
    return (float(a[d]) <= float(b[d]) and float(a[s]) <= float(b[s])
            and (float(a[d]) < float(b[d]) or float(a[s]) < float(b[s])))


def summarize(out, mapped, routed):
    lines = ['# Frozen W=24 arrival-aware control challenge\n\n',
             '| Stage | Recipe | Candidate ps / um2 | Sibling ps / um2 | Delay change | Area change | Dominating new baselines |\n',
             '|---|---|---|---|---:|---:|---:|\n']
    frontier = []
    for stage, rows in (('mapped', mapped), ('global-route', routed)):
        for recipe in ('default', 'classic'):
            group = [r for r in rows if r['recipe'] == recipe]
            assert len(group) == 15
            c = next(r for r in group if r['class'] == 'candidate')
            a = next(r for r in group if r['class'] == 'ablation')
            controls = [r for r in group if r['class'] in ('arrival_control', 'behavioral')]
            dominating = [r for r in controls if dominates(r, c)]
            cd, ca = float(c['worst_arrival_ps']), float(c['area_um2'])
            ad, aa = float(a['worst_arrival_ps']), float(a['area_um2'])
            lines.append(f'| {stage} | {recipe} | {cd:.0f} / {ca:.2f} | {ad:.0f} / {aa:.2f} | {(cd/ad-1)*100:+.2f}% | {(ca/aa-1)*100:+.2f}% | {len(dominating)} |\n')
            for row in group:
                if not any(dominates(other, row) for other in group):
                    frontier.append({'stage': stage, 'recipe': recipe, 'module': row['module'],
                                     'class': row['class'], 'worst_arrival_ps': row['worst_arrival_ps'],
                                     'area_um2': row['area_um2']})
    lines.append('\nAll 30 mappings and all 30 routes are retained. New baselines comprise\n12 rule-free arrival-aware variants plus behavioral RTL, all with the exact\n75-bit output contract. This corpus supplements the earlier 30 positional\ntextbook trunks; it is not the full space of modern arithmetic optimizers.\n')
    lines.append('\nArrival estimates use the existing generator\'s coarse depth model. These\nare not a faithful pin-delay-aware three-greedy implementation, ARITH-DAS,\nor UFO-MAC. Any forced completion follows the existing generator. The\ncandidate/genome are unchanged. No parameter was selected using this run\'s\nphysical results; all planned competitors were routed at utilization 45.\n')
    lines.append('\nThe behavioral baseline uses the same fixed lowering/mapping script as all\nother designs. It is not a sweep of the best available behavioral synthesis\nstrategies. Routing uses estimated global-route parasitics; no signoff, power,\nsilicon, or statistical-replication claim follows.\n')
    lines.append('\nEvery structural form passed composed all-input proof; the behavioral form\nis the arithmetic reference. Every mapped form passed direct ABC equivalence\nto its input RTL. See the frozen protocol, proof records, mapping and routing\ntables, logs, and source hashes. All computation ran serially, with one\nYosys/ABC thread and at most two OpenROAD/container cores.\n')
    lines.append('\nReproduce with `python3 scripts/compare_mac_arrival.py --output <fresh-relative-directory>`.\n')
    (out / 'README.md').write_text(''.join(lines))
    write_tsv(out / 'frontier.tsv', frontier)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    args = parser.parse_args()
    out = args.output
    assert not out.is_absolute() and '..' not in out.parts
    assert re.fullmatch(r'[A-Za-z0-9_./-]+', str(out)) and not out.exists()
    old = json.loads((HELDOUT / 'physical/provenance.json').read_text())
    image = subprocess.check_output(['docker', 'image', 'inspect', 'openroad/orfs:latest',
                                     '--format', '{{.Id}}'], text=True).strip()
    yosys = subprocess.check_output(['yosys', '-V'], text=True).strip()
    assert image == old['docker_image'] and sha(LIB) == old['liberty_sha256']
    assert yosys == old['yosys'], 'tool changed since frozen comparison'
    out.mkdir(parents=True)
    protocol = {'recorded_before_generation_utc': datetime.now(timezone.utc).isoformat(),
                'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
                'candidate_genome_sha256': sha(HELDOUT / 'genome.json'),
                'runner_sha256': sha(Path(__file__)), 'generator_sha256': sha(Path('examples/mac_schedule_controls.rs')),
                'core_generator_sha256': sha(Path('src/evo.rs')),
                'image_id': image, 'yosys': yosys, 'liberty_sha256': sha(LIB),
                'flow_sha256': sha(FLOW), 'sta_sha256': sha(Path('scripts/sta_cores.tcl')),
                'recipes': ['default', 'classic'], 'width': 24,
                'control_actions': ['Dadda', 'FullNoHa'], 'control_pick': 'Earliest',
                'control_adders': ['sklansky', 'brentkung', 'koggestone'],
                'control_full_adders': ['Mux', 'XorMaj'],
                'fixed': 'candidate root/fusion/rounding/clamp/status plan; no rules on controls; same-stage setting unchanged',
                'expected_designs': 15, 'expected_mappings': 30, 'expected_routes': 30,
                'primary_endpoint': 'candidate nondomination against all 12 arrival controls and behavioral reference under each recipe after mapping and routing',
                'route_selection': 'all designs, utilization 45, placement density 0.55',
                'concurrent_jobs': 1, 'container_cpus': 2, 'openroad_threads': 2,
                'yosys_abc_threads': 1, 'cargo_build_jobs': 2,
                'mapping_timeout_seconds': 60, 'route_timeout_seconds': 120,
                'interpretation': 'finite coarse-arrival comparison, not a modern-optimizer or full three-greedy benchmark'}
    (out / 'protocol.json').write_text(json.dumps(protocol, indent=2) + '\n')
    run(['nice', '-n', '19', 'cargo', 'run', '--locked', '--release', '--example',
         'mac_schedule_controls', '--', str(HELDOUT / 'genome.json'), str(out / 'generated')],
        out / 'generation.log', 180)
    rtl = out / 'rtl'
    rtl.mkdir()
    shutil.copyfile(HELDOUT / 'cells.v', out / 'cells.v')
    rows = [{'module': 'mul24_mac_replay_candidate', 'class': 'candidate'},
            {'module': 'mul24_mac_replay_norules', 'class': 'ablation'}]
    for row in rows:
        shutil.copyfile(HELDOUT / f'{row["module"]}.v', rtl / f'{row["module"]}.v')
    for row in csv.DictReader((out / 'generated/manifest.tsv').open(), delimiter='\t'):
        rows.append({'module': row['module'], 'class': row['class']})
        shutil.copyfile(out / 'generated' / f'{row["module"]}.v', rtl / f'{row["module"]}.v')
    assert len(rows) == 14
    run(['python3', 'scripts/prove_mac_composed.py', '--cells', str(out / 'cells.v'),
         '--output', str(out / 'formal'), '--seconds', '20',
         *[str(rtl / f'{r["module"]}.v') for r in rows]], out / 'formal.log', 510)
    rows.append({'module': 'mac24_reference', 'class': 'behavioral'})
    shutil.copyfile(Path('benchmarks/mac24/reference.v'), rtl / 'mac24_reference.v')
    for row in rows:
        row['source_sha256'] = sha(rtl / f'{row["module"]}.v')
    write_tsv(out / 'sources.tsv', rows)
    print('Generated controls; all 14 structural forms proved', flush=True)
    mapped = out / 'mapped'
    mapped.mkdir()
    mapped_rows, equivalence = [], []
    for row in rows:
        top = row['module']
        for recipe in ('default', 'classic'):
            name = f'{top}_{recipe}'
            recipe_arg = '' if recipe == 'default' else '-script scripts/abc/classic.script'
            command = f'read_verilog {out}/cells.v {rtl}/{top}.v; attrmap -modattr -remove keep_hierarchy=1; hierarchy -top {top}; proc; flatten; opt; techmap; opt; abc -liberty {LIB} {recipe_arg}; opt_clean; rename {top} {name}; write_verilog -noattr -noexpr {mapped}/{name}.v'
            (out / f'{name}.ys').write_text(command + '\n')
            run(['nice', '-n', '19', 'yosys', '-Q', '-s', str(out / f'{name}.ys')],
                out / f'{name}.log', 60)
            proof = prove_pair(rtl / f'{top}.v', mapped / f'{name}.v', out / 'cells.v',
                               LIB, out / 'mapped_proof' / name, 24, 20)
            proof['module'] = name
            equivalence.append(proof)
            (out / 'mapped_proof.json').write_text(json.dumps(equivalence, indent=2) + '\n')
            assert proof['status'] == 'proved', f'{name}: mapped equivalence incomplete'
            mapped_rows.append({**row, 'module': name, 'recipe': recipe,
                                'mapped_sha256': sha(mapped / f'{name}.v')})
            print(f'Mapped/proved {len(mapped_rows)}/30 {name}', flush=True)
    run(['docker', 'run', '--platform', 'linux/amd64', '--rm', '--cpus', '2',
         '-v', f'{Path.cwd()}:/work', '-w', '/work', '-e', 'OMP_NUM_THREADS=2',
         '-e', f'LIB={LIB}', '-e', f'MAPPED={mapped}', '-e', f'OUT={out}/sta.tsv',
         image, '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/sta', '-exit',
         'scripts/sta_cores.tcl'], out / 'sta.log', 120)
    timing = {r['module']: r for r in csv.DictReader((out / 'sta.tsv').open(), delimiter='\t')}
    assert len(timing) == 30
    mapped_rows = [{**r, **timing[r['module']]} for r in mapped_rows]
    assert all(float(r['worst_arrival_ps']) > 0 and float(r['area_um2']) > 0 for r in mapped_rows)
    write_tsv(out / 'mapped_summary.tsv', mapped_rows)
    routed = []
    for i, row in enumerate(mapped_rows):
        name = row['module']
        prefix = out / 'route' / name
        prefix.parent.mkdir(exist_ok=True)
        container = f'macarrival-{i}'
        try:
            run(['docker', 'run', '--name', container, '--platform', 'linux/amd64',
                 '--rm', '--cpus', '2', '-v', f'{Path.cwd()}:/work', '-w', '/work',
                 '-e', 'OMP_NUM_THREADS=2', '-e', f'NETLIST={mapped}/{name}.v',
                 '-e', f'TOP={name}', '-e', f'OUT={prefix}', '-e', 'UTILIZATION=45',
                 image, '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/openroad',
                 '-threads', '2', '-exit', str(FLOW)], prefix.with_suffix('.log'), 120)
        finally:
            subprocess.run(['docker', 'rm', '-f', container], check=False,
                           stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
        log = prefix.with_suffix('.log').read_text()
        report = prefix.with_suffix('.checks.rpt').read_text()
        delay = float(re.search(r'([0-9.]+)\s+data arrival time', report)[1]) * 1000
        area = re.findall(r'^Design area\s+([0-9.]+)', log, re.M)[-1]
        hpwl = re.findall(r'^legalized HPWL\s+([0-9.]+)', log, re.M)[-1]
        routed.append({**row, 'worst_arrival_ps': f'{delay:.1f}', 'area_um2': area, 'hpwl_um': hpwl})
        write_tsv(out / 'routed_summary.tsv', routed)
        print(f'Routed {i+1}/30 {name}: {delay:.0f} ps / {area} um2', flush=True)
    summarize(out, mapped_rows, routed)


if __name__ == '__main__':
    main()
