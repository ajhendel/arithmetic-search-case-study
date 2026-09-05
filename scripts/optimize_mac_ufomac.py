#!/usr/bin/env python3
"""Bounded use of the pinned ARITH-DAS UFO-MAC replication; no training imports."""
import os
for key in ('OMP_NUM_THREADS', 'OPENBLAS_NUM_THREADS', 'VECLIB_MAXIMUM_THREADS'):
    os.environ[key] = '1'
import ast
from collections import deque
import csv
import hashlib
import json
import logging
from pathlib import Path
import re
import time
from typing import Tuple, List
import numpy as np
import pulp

ROOT = Path('results/discovery/mac_ufomac_challenge_20260905')
SOURCE = Path('integrations/arithdas/vendor/compressor_tree.py')
METHODS = {'__init__', 'ufomac', 'compressor_assignment_ufomac', 'ufomac_router',
           'declare_wire', 'declare_fa', 'declare_ha'}


def load_upstream():
    tree = ast.parse(SOURCE.read_text())
    cls = next(n for n in tree.body if isinstance(n, ast.ClassDef) and n.name == 'CompressorTree')
    selected = [n for n in cls.body if isinstance(n, ast.FunctionDef) and n.name in METHODS]
    selected += [n for n in cls.body if isinstance(n, ast.Assign) and any(
        isinstance(t, ast.Name) and t.id == 'UFO_MAC_CONSTANT' for t in n.targets)]
    assert {n.name for n in selected if isinstance(n, ast.FunctionDef)} == METHODS
    # Only omit optional training imports/base class; method ASTs are unchanged.
    cls.bases, cls.body = [], selected
    module = ast.Module(body=[cls], type_ignores=[])
    env = dict(np=np, pulp=pulp, logging=logging, time=time, json=json,
               deque=deque, Tuple=Tuple, List=List)
    exec(compile(ast.fix_missing_locations(module), str(SOURCE), 'exec'), env)
    return env['CompressorTree']


class CheckedHiGHS(pulp.HiGHS):
    def actualSolve(self, lp, **kwargs):
        start = time.monotonic()
        result = super().actualSolve(lp, **kwargs)
        record = {'model': lp.name, 'elapsed_seconds': time.monotonic() - start,
                  'status': pulp.LpStatus[lp.status], 'solution_status': pulp.LpSolution[lp.sol_status],
                  'objective': pulp.value(lp.objective), 'variables': len(lp.variables()),
                  'constraints': len(lp.constraints), 'threads': 1}
        info = lp.solverModel.getInfo()
        record.update(mip_gap=info.mip_gap, mip_dual_bound=info.mip_dual_bound)
        valid = lp.sol_status in (pulp.LpSolutionOptimal, pulp.LpSolutionIntegerFeasible)
        valid = valid and all(v.varValue is not None and v.valid(1e-5) for v in lp.variables())
        valid = valid and all(c.valid(1e-5) for c in lp.constraints.values())
        record['validated_feasible'] = valid
        (ROOT / f'{lp.name}.status.json').write_text(json.dumps(record, indent=2) + '\n')
        if not valid:
            raise RuntimeError(f'No validated integer incumbent: {record}')
        # Guard upstream int(value) export against 0.999999999 truncation.
        for var in lp.variables():
            if var.cat == pulp.LpInteger:
                var.varValue = round(var.varValue)
        return result


def solver_factory(timeLimit, keepFiles, msg, options):
    assert not keepFiles and options == [('Threads', '1')]
    return CheckedHiGHS(timeLimit=timeLimit, threads=1, msg=msg, random_seed=0)


def export_graph(text, rows, width):
    aliases, cells = {}, []
    pp = [[] for _ in range(2 * width + 1)]
    for c in range(2 * width - 1):
        for a in range(max(0, c-width+1), min(width-1, c)+1):
            name = f'pp_{c}[{len(pp[c])}]'
            pp[c].append(name)
            aliases[name] = name
            cells.append({'kind': 'and', 'inputs': [f'a{a}', f'b{c-a}'], 'outputs': [name]})
    for c in range(2 * width):
        name = f'pp_{c}[{len(pp[c])}]'
        aliases[name] = f'aux{c}'
        pp[c].append(name)
    clean = re.sub(r'//[^\n]*', '', text)
    for statement in clean.split(';'):
        s = statement.strip()
        if not s or s.startswith('wire '):
            continue
        m = re.fullmatch(r'assign (\w+) = ([\w\[\]]+)', s)
        if m:
            assert m[1] not in aliases and m[2] in aliases, s
            aliases[m[1]] = aliases[m[2]]
            continue
        m = re.fullmatch(r'(FA|HA) \w+\s*\((.*)\)', s, re.S)
        assert m, s
        pins = dict(re.findall(r'\.(\w+)\s*\((\w+)\)', m[2]))
        names = ('a', 'b', 'cin') if m[1] == 'FA' else ('a', 'cin')
        inputs = [aliases[pins[p]] for p in names]
        outputs = [pins[p] for p in ('sum', 'cout')]
        assert all(n not in aliases for n in outputs)
        aliases.update({n: n for n in outputs})
        cells.append({'kind': m[1].lower(), 'inputs': inputs, 'outputs': outputs})
    return {'width': width, 'cells': cells, 'rows': [[aliases[n] for n in col] for col in rows]}


def main():
    os.nice(19)
    protocol = json.loads((ROOT / 'protocol.json').read_text())
    assert hashlib.sha256(SOURCE.read_bytes()).hexdigest() == protocol['upstream_sha256']
    assert not (ROOT / 'schedule.json').exists(), 'preserve previous attempt'
    cls = load_upstream()
    pulp.TinyBoundedHiGHS = solver_factory
    width = 24
    pp = np.zeros(2*width+1, dtype=int)
    for a in range(width):
        for b in range(width): pp[a+b] += 1
    pp[:2*width] += 1
    ct = cls.ufomac(pp)
    f, h, stages, status = ct.compressor_assignment_ufomac(
        max_stage_num=12, M=64, method='TinyBoundedHiGHS', n_processing=1,
        timeLimit=60, keepFiles=False)
    assert np.array_equal(f.sum(axis=0), ct.ct32) and np.array_equal(h.sum(axis=0), ct.ct22)
    (ROOT / 'schedule.json').write_text(json.dumps({'pp': pp.tolist(), 'f': f.tolist(),
        'h': h.tolist(), 'stages': stages, 'status': status}, indent=2)+'\n')
    arcs = list(csv.DictReader((ROOT / 'characterization/arcs.tsv').open(), delimiter='\t'))
    profiles = {}
    for kind in ('mux', 'xormaj'):
        profile = {'FA': {}, 'HA': {}}
        for cell, top in [('FA', 'fa_'+kind), ('HA', 'ha')]:
            for row in arcs:
                if row['module'] == top:
                    pin = {'a': 'a', 'b': 'b' if cell == 'FA' else 'c', 'cin': 'c'}[row['input']]
                    profile[cell]['T'+pin+row['output']] = float(row['delay_ns'])
        profiles[kind] = profile
    (ROOT / 'pin_profiles.json').write_text(json.dumps(profiles, indent=2)+'\n')
    # Equal measured profiles imply exactly the same ILP; solve once and reuse.
    assert profiles['mux'] == profiles['xormaj'], 'protocol requires two distinct solves if profiles differ'
    ct.UFO_MAC_CONSTANT = profiles['mux']
    text, rows = ct.ufomac_router(f, h, method='TinyBoundedHiGHS', Z_constant=100,
        n_processing=1, timeLimit=90, keepFiles=False, json_file=str(ROOT/'interconnect.json'))
    (ROOT / 'upstream_router.v.txt').write_text(text)
    graph = export_graph(text, rows, width)
    (ROOT / 'optimized_graph.json').write_text(json.dumps(graph, indent=2)+'\n')
    print(f'Exported {len(graph["cells"])} gates; identical mapped FA profiles reused for both implementations', flush=True)


if __name__ == '__main__':
    main()
