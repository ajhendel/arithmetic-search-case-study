#!/usr/bin/env python3
"""Route the sole previously recorded W=24/classic mapped dominator."""
import csv
from datetime import datetime, timezone
import json
from pathlib import Path
import re
import subprocess

from compare_mac_arrival import HELDOUT, LIB, FLOW, dominates, run, sha, write_tsv
from prove_mac_mapped import prove_pair


def main():
    primary = Path('results/discovery/mac_arrival_challenge_20260905')
    current = list(csv.DictReader((primary / 'routed_summary.tsv').open(), delimiter='\t'))
    assert len(current) == 30, 'finish the serial primary comparison first'
    candidate = next(r for r in current if r['class'] == 'candidate' and r['recipe'] == 'classic')
    old = list(csv.DictReader((HELDOUT / 'physical/summary.tsv').open(), delimiter='\t'))
    old = [r for r in old if r['width'] == '24' and r['recipe'] == 'classic']
    old_candidate = next(r for r in old if r['class'] == 'candidate')
    selected = [r for r in old if r['class'] == 'control' and dominates(r, old_candidate)]
    assert len(selected) == 1
    top = selected[0]['module']
    source = HELDOUT / f'{top.removesuffix("_classic")}.v'
    gate = HELDOUT / 'physical/mapped' / f'{top}.v'
    out = primary / 'classic_neighbor'
    assert not out.exists(), 'preserve prior results'
    out.mkdir()
    protocol = json.loads((primary / 'protocol.json').read_text())
    protocol.update(recorded_before_supplement_routing_utc=datetime.now(timezone.utc).isoformat(),
                    selection='sole dominator in committed W=24/classic pre-layout control table; supplementary to the 30-job primary challenge',
                    source_sha256=sha(source), mapped_sha256=sha(gate), module=top,
                    runner_sha256=sha(Path(__file__)), expected_routes=1, expected_designs=1,
                    expected_mappings=0, primary_protocol_sha256=sha(primary / 'protocol.json'))
    (out / 'protocol.json').write_text(json.dumps(protocol, indent=2) + '\n')
    run(['python3', 'scripts/prove_mac_composed.py', '--cells', str(HELDOUT / 'cells.v'),
         '--output', str(out / 'formal'), str(source)], out / 'formal.log', 80)
    proof = prove_pair(source, gate, HELDOUT / 'cells.v', LIB, out / 'mapped_proof', 24)
    (out / 'mapped_proof.json').write_text(json.dumps(proof, indent=2) + '\n')
    assert proof['status'] == 'proved'
    prefix = out / top
    container = 'macarrival-classic-neighbor'
    command = ['docker', 'run', '--name', container, '--platform', 'linux/amd64',
               '--rm', '--cpus', '2', '-v', f'{Path.cwd()}:/work', '-w', '/work',
               '-e', 'OMP_NUM_THREADS=2', '-e', f'NETLIST={gate}', '-e', f'TOP={top}',
               '-e', f'OUT={prefix}', '-e', 'UTILIZATION=45', protocol['image_id'],
               '/OpenROAD-flow-scripts/tools/install/OpenROAD/bin/openroad',
               '-threads', '2', '-exit', str(FLOW)]
    try:
        run(command, prefix.with_suffix('.log'), 120)
    finally:
        subprocess.run(['docker', 'rm', '-f', container], check=False,
                       stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    log = prefix.with_suffix('.log').read_text()
    report = prefix.with_suffix('.checks.rpt').read_text()
    delay = float(re.search(r'([0-9.]+)\s+data arrival time', report)[1]) * 1000
    row = {'module': top, 'class': 'prior_textbook_control', 'recipe': 'classic',
           'worst_arrival_ps': f'{delay:.1f}',
           'area_um2': re.findall(r'^Design area\s+([0-9.]+)', log, re.M)[-1],
           'mapped_sha256': sha(gate)}
    fields = list(row)
    write_tsv(out / 'summary.tsv', [{k: candidate[k] for k in fields}, row])
    verdict = dominates(row, candidate)
    (out / 'README.md').write_text(
        '# Previously mapped-dominating classic control\n\n'
        f'Candidate: {candidate["worst_arrival_ps"]} ps / {candidate["area_um2"]} um2.\n'
        f'Control `{top}`: {row["worst_arrival_ps"]} ps / {row["area_um2"]} um2.\n'
        f'The control dominates the candidate after global routing: **{verdict}**.\n\n'
        'This one-design supplement follows the previously committed mapped\n'
        'dominator, with the same library/image/flow and utilization 45. It is\n'
        'not a full classic routed-control census. Both structural correctness\n'
        'and mapped equivalence passed before routing. All checks ran serially.\n')
    print(row, 'dominates candidate:', verdict, flush=True)


if __name__ == '__main__':
    main()
