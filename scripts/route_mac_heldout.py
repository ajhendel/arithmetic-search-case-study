#!/usr/bin/env python3
"""Full W=24 default control challenge plus frozen floorplan sensitivity cases."""
import argparse
import csv
from datetime import datetime, timezone
import hashlib
import json
from pathlib import Path
import re
import subprocess

ROOT = Path('results/discovery/mac_crosswidth_20260905/evo608_heldout/physical')
OUT = ROOT / 'route24_default'
IMAGE = 'openroad/orfs:latest'
FLOW = Path('scripts/place_core.tcl')


def number(row, key):
    return float(row[key])


def dominates(a, b):
    d, s = 'worst_arrival_ps', 'area_um2'
    return number(a, d) <= number(b, d) and number(a, s) <= number(b, s) and (number(a, d) < number(b, d) or number(a, s) < number(b, s))


def write_rows(path, rows):
    with path.open('w') as handle:
        writer = csv.DictWriter(handle, fieldnames=list(rows[0]), delimiter='\t', lineterminator='\n')
        writer.writeheader()
        writer.writerows(rows)


def main():
    global OUT
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, default=OUT)
    parser.add_argument('--cores', type=int, choices=range(1, 3), default=2,
                        help='CPU quota and OpenROAD threads (1 or 2; jobs run serially)')
    args = parser.parse_args()
    OUT = args.output
    OUT.mkdir(parents=True, exist_ok=True)
    rows = [r for r in csv.DictReader((ROOT / 'summary.tsv').open(), delimiter='\t') if r['width'] == '24' and r['recipe'] == 'default']
    assert len(rows) == 32
    candidate = next(r for r in rows if r['class'] == 'candidate')
    controls = [r for r in rows if r['class'] == 'control']
    front = [r for r in controls if not any(dominates(o, r) for o in controls)]
    selected = {r['module']: r for r in rows if r['class'] in ('candidate', 'ablation')}
    fastest = min(front, key=lambda r: (number(r, 'worst_arrival_ps'), number(r, 'area_um2'), r['module']))
    selected[fastest['module']] = fastest
    below = [r for r in front if number(r, 'area_um2') <= number(candidate, 'area_um2')]
    above = [r for r in front if number(r, 'area_um2') >= number(candidate, 'area_um2')]
    if below:
        r = max(below, key=lambda r: (number(r, 'area_um2'), -number(r, 'worst_arrival_ps'), r['module']))
        selected[r['module']] = r
    if above:
        r = min(above, key=lambda r: (number(r, 'area_um2'), number(r, 'worst_arrival_ps'), r['module']))
        selected[r['module']] = r
    jobs = [(45, r) for r in rows] + [(util, r) for util in (35, 50) for r in selected.values()]
    selection = [{'utilization': util, 'module': r['module'], 'class': r['class'], 'netlist_sha256': hashlib.sha256((ROOT / 'mapped' / f"{r['module']}.v").read_bytes()).hexdigest()} for util, r in jobs]
    if (OUT / 'summary.tsv').exists():
        raise RuntimeError('Existing results: use a fresh output directory or review explicitly before rerunning')
    write_rows(OUT / 'selection.tsv', selection)
    image_id = subprocess.check_output(['docker', 'image', 'inspect', IMAGE, '--format', '{{.Id}}'], text=True).strip()
    (OUT / 'protocol.json').write_text(json.dumps({
        'recorded_before_routing_utc': datetime.now(timezone.utc).isoformat(),
        'base_commit': subprocess.check_output(['git', 'rev-parse', 'HEAD'], text=True).strip(),
        'image_id': image_id,
        'flow_sha256': hashlib.sha256(FLOW.read_bytes()).hexdigest(),
        'runner_sha256': hashlib.sha256(Path(__file__).read_bytes()).hexdigest(),
        'selection': 'all 30 matched textbook trunks, candidate, and ablation at utilization 45; candidate/ablation plus pre-layout fastest and area-bracketing control frontier at 35 and 50',
        'primary_endpoint': 'candidate dominance against all 30 controls at utilization 45',
        'secondary_endpoint': 'paired candidate/no-rule delay and area changes at all three utilizations',
        'interpretation': 'floorplan-size and resulting placement/pin/routing sensitivity, not independent seeds, statistical replicates, or isolated cell-density effects',
        'placement_target_density': 0.55,
        'failures': 'record failed or timed-out jobs; exclude them from comparisons and label the challenge incomplete',
        'timeout_seconds_per_design': 120,
        'expected_jobs': len(jobs),
        'cpu_limit': args.cores,
        'openroad_threads': args.cores,
        'concurrent_jobs': 1,
    }, indent=2) + '\n')
    results = []
    for utilization, row in jobs:
        top = row['module']
        jobdir = OUT / f'util{utilization}'
        jobdir.mkdir(exist_ok=True)
        prefix = jobdir / top
        log_path = prefix.with_suffix('.log')
        result = {'utilization': utilization, 'module': top, 'class': row['class'], 'status': 'ok', 'worst_arrival_ps': '', 'area_um2': '', 'hpwl_um': '', 'error': ''}
        container_name = f'mac-heldout-{utilization}-{top}'.replace('_', '-')
        try:
            with log_path.open('w') as log:
                subprocess.run(['docker', 'run', '--name', container_name, '--platform', 'linux/amd64', '--cpus', str(args.cores), '--rm', '-v', f'{Path.cwd()}:/work', '-w', '/work', '-e', f'NETLIST={ROOT}/mapped/{top}.v', '-e', f'TOP={top}', '-e', f'OUT={prefix}', '-e', f'UTILIZATION={utilization}', '-e', f'OMP_NUM_THREADS={args.cores}', image_id, '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/openroad', '-threads', str(args.cores), '-exit', str(FLOW)], check=True, stdout=log, stderr=subprocess.STDOUT, timeout=120)
            log = log_path.read_text()
            checks = prefix.with_suffix('.checks.rpt').read_text()
            arrival = re.search(r'([0-9.]+)\s+data arrival time', checks)
            area = re.findall(r'^Design area\s+([0-9.]+)', log, re.M)
            hpwl = re.findall(r'^legalized HPWL\s+([0-9.]+)', log, re.M)
            if not (arrival and area and hpwl):
                raise ValueError('missing timing/area/HPWL report fields')
            result.update(worst_arrival_ps=f'{float(arrival[1])*1000:.1f}', area_um2=area[-1], hpwl_um=hpwl[-1])
        except subprocess.TimeoutExpired:
            subprocess.run(['docker', 'rm', '-f', container_name], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL, check=False)
            result.update(status='timeout', error='120-second job budget exhausted')
        except (subprocess.CalledProcessError, ValueError, OSError) as error:
            result.update(status='failed', error=str(error).replace('\n', ' '))
        results.append(result)
        write_rows(OUT / 'summary.tsv', results)
        print(f'{len(results)}/{len(jobs)} util={utilization} {top}: {result["status"]} {result["worst_arrival_ps"]} ps', flush=True)
    lines = ['# W=24 default mapping: full controls and floorplan sensitivity\n\n',
             '| utilization | candidate ps / um2 | no-rule ps / um2 | delay change | area change | completed controls | dominating controls |\n',
             '|---:|---|---|---:|---:|---:|---:|\n']
    for util in (35, 45, 50):
        group = [r for r in results if r['utilization'] == util and r['status'] == 'ok']
        c = next((r for r in group if r['class'] == 'candidate'), None)
        a = next((r for r in group if r['class'] == 'ablation'), None)
        if c is None or a is None:
            lines.append(f'| {util} | incomplete | incomplete | — | — | — | — |\n')
            continue
        cs = [r for r in group if r['class'] == 'control']
        cd, ca = number(c,'worst_arrival_ps'), number(c,'area_um2')
        ad, aa = number(a,'worst_arrival_ps'), number(a,'area_um2')
        lines.append(f'| {util} | {cd:.0f} / {ca:.0f} | {ad:.0f} / {aa:.0f} | {(cd/ad-1)*100:+.2f}% | {(ca/aa-1)*100:+.2f}% | {len(cs)} | {sum(dominates(r,c) for r in cs)} |\n')
    failures = [r for r in results if r['status'] != 'ok']
    lines.append(f'\nCompleted {len(results)-len(failures)}/{len(jobs)} planned jobs. The full 30-control challenge applies only at utilization 45; the other two settings use frozen pre-layout neighbors. All timing uses estimated global-route parasitics. The three settings are sensitivity cases, not statistical replicates.\n')
    if failures:
        lines.append('\nSome jobs failed; see summary.tsv. No complete-frontier claim is warranted for incomplete settings.\n')
    lines.append('\nReproduce in a fresh checkout/output location: `python3 scripts/route_mac_heldout.py --output <fresh-directory> --cores 2`, after the held-out replay and mapping commands. Selection, hashes, and protocol were recorded before routing.\n')
    (OUT / 'README.md').write_text(''.join(lines))


if __name__ == '__main__':
    main()
