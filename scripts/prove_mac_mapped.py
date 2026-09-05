#!/usr/bin/env python3
"""Bounded ABC equivalence of frozen held-out SKY130 netlists and proven RTL."""
import argparse
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import shutil
import subprocess
import tempfile

ROOT = Path('results/discovery/mac_crosswidth_20260905')
HELDOUT = ROOT / 'evo608_heldout'


def sha(path):
    return hashlib.sha256(path.read_bytes()).hexdigest()


def aiger_ports(path, width):
    lines = path.read_text().splitlines()
    magic, maximum, inputs, latches, outputs, gates = lines[0].split()
    assert magic == 'aag' and int(latches) == 0, 'not a combinational ASCII AIG'
    assert (int(inputs), int(outputs)) == (4*width, 3*width+3), 'lost MAC ports'
    start = 1 + int(inputs) + int(outputs) + int(gates)
    symbols = {}
    for line in lines[start:]:
        if line == 'c':
            break
        key, value = line.split(' ', 1)
        assert key not in symbols, 'duplicate AIG port symbol'
        symbols[key] = value
    expected = {f'{port}[{i}]' for port, size in (('a', width), ('b', width),
                                                ('aux', 2*width)) for i in range(size)}
    assert {symbols.get(f'i{i}') for i in range(4*width)} == expected, 'input symbols differ'
    assert {symbols.get(f'o{i}') for i in range(3*width+3)} == {
        f'y[{i}]' for i in range(3*width+3)}, 'output symbols differ'
    return symbols


def prove_pair(source, gate, cells, liberty, out, width, seconds=20):
    out.mkdir(parents=True)
    top = re.search(r'^module (\w+)\(', source.read_text(), re.M)[1]
    mapped_top = re.search(r'^module (\w+)\(', gate.read_text(), re.M)[1]
    assert top == source.stem and mapped_top == gate.stem, 'unexpected top module'
    # Fixed local filenames make commands reproducible regardless of host paths.
    command = f'''read_verilog cells.v gold.v
attrmap -modattr -remove keep_hierarchy=1
hierarchy -check -top {top}
proc; flatten; opt -keepdc; techmap; opt -keepdc; check -assert; aigmap
write_aiger -ascii -symbols gold.aag
write_aiger -symbols gold.aig
design -reset
read_liberty -ignore_miss_func library.lib
read_verilog gate.v
hierarchy -check -top {mapped_top}
proc; flatten; opt -keepdc; techmap; opt -keepdc; check -assert; aigmap
write_aiger -ascii -symbols gate.aag
write_aiger -symbols gate.aig
'''
    (out / 'command.ys').write_text(command)
    abc_command = f'cec -T {seconds} gold.aig gate.aig'
    (out / 'command.abc').write_text(abc_command + '\n')
    for path, name in ((source, 'gold.v'), (gate, 'gate.v'), (cells, 'cells.v')):
        shutil.copyfile(path, out / name)
    row = {'source_sha256': sha(source), 'mapped_sha256': sha(gate),
           'cells_sha256': sha(cells), 'liberty_sha256': sha(liberty), 'width': width,
           'status': 'error'}
    env = {**os.environ, 'OMP_NUM_THREADS': '1', 'OPENBLAS_NUM_THREADS': '1',
           'VECLIB_MAXIMUM_THREADS': '1'}
    # The Liberty file is required locally, but is not bundled into the result.
    with tempfile.TemporaryDirectory(prefix='mac-lec-') as tmp:
        work = Path(tmp)
        for name in ('gold.v', 'gate.v', 'cells.v', 'command.ys'):
            shutil.copyfile(out / name, work / name)
        shutil.copyfile(liberty, work / 'library.lib')
        try:
            with (out / 'yosys.log').open('w') as log:
                run = subprocess.run(['nice', '-n', '19', 'yosys', '-Q', '-s', 'command.ys'],
                                     cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT,
                                     timeout=seconds+15)
            if run.returncode:
                return row
            gold_ports = aiger_ports(work / 'gold.aag', width)
            gate_ports = aiger_ports(work / 'gate.aag', width)
            assert gold_ports == gate_ports, 'AIG port correspondence differs'
            row['aig_sha256'] = {name: sha(work / name) for name in ('gold.aig', 'gate.aig')}
            row['checked_inputs'], row['checked_outputs'] = 4*width, 3*width+3
            with (out / 'abc.log').open('w') as log:
                run = subprocess.run(['nice', '-n', '19', 'yosys-abc', '-c', abc_command],
                                     cwd=work, env=env, stdout=log, stderr=subprocess.STDOUT,
                                     timeout=seconds+15)
            result = (out / 'abc.log').read_text()
            if run.returncode == 0 and re.search(r'^Networks are equivalent\.', result, re.M):
                row['status'] = 'proved'
            elif 'NOT EQUIVALENT' in result:
                row['status'] = 'counterexample'
            elif 'undecided' in result.lower() or 'timeout' in result.lower():
                row['status'] = 'inconclusive'
        except subprocess.TimeoutExpired:
            row['status'] = 'timeout'
        except AssertionError as error:
            row['error'] = str(error)
    return row


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--liberty', type=Path, default=Path('pdk/sky130_fd_sc_hd__tt_025C_1v80.lib'))
    args = parser.parse_args()
    assert not args.output.exists(), 'preserve old results; use a fresh directory'
    structural = json.loads((ROOT / 'evo608_composed_formal_strict/summary.json').read_text())
    assert structural['cell_lemmas'] == 'proved'
    proven = {r['source']: structural['source_sha256'][r['source']]
              for r in structural['results'] if r['status'] == 'proved'}
    cells = HELDOUT / 'cells.v'
    assert sha(cells) == structural['cells_sha256']
    mapping = json.loads((HELDOUT / 'physical/provenance.json').read_text())
    assert sha(args.liberty) == mapping['liberty_sha256'], 'wrong mapping library'
    args.output.mkdir(parents=True)
    record = {'started_utc': datetime.now(timezone.utc).isoformat(),
              'checker_sha256': sha(Path(__file__)), 'base_commit': subprocess.check_output(
                  ['git', 'rev-parse', 'HEAD'], text=True).strip(),
              'yosys': subprocess.check_output(['yosys', '-V'], text=True).strip(),
              'abc': subprocess.check_output(['yosys-abc', '-c', 'version'], text=True).strip(),
              'structural_summary_sha256': sha(ROOT / 'evo608_composed_formal_strict/summary.json'),
              'concurrent_jobs': 1, 'threads': 1, 'abc_seconds': 20,
              'wall_seconds_per_process': 35, 'results': []}
    def save():
        (args.output / 'summary.json').write_text(json.dumps(record, indent=2) + '\n')
    save()
    for width in (12, 24):
        for kind in ('candidate', 'norules'):
            source = HELDOUT / f'mul{width}_mac_replay_{kind}.v'
            assert proven.get(str(source)) == sha(source), 'source lacks matching composed proof'
            for recipe in ('default', 'classic'):
                gate = HELDOUT / 'physical/mapped' / f'{source.stem}_{recipe}.v'
                row = prove_pair(source, gate, cells, args.liberty, args.output / gate.stem, width)
                row.update(module=gate.stem, kind=kind, recipe=recipe)
                record['results'].append(row)
                save()
                print(gate.stem, row['status'], flush=True)
    assert all(r['status'] == 'proved' for r in record['results']), 'incomplete equivalence campaign'


if __name__ == '__main__':
    main()
