#!/usr/bin/env python3
"""Composed proof of emitted fused MACs: weighted conservation plus a SAT tail.

Accepts only the explicit generated cell dialect. Unknown syntax, duplicate
drivers, missing partial products, reused compressor inputs, dropped bits, or
non-two-row boundaries fail closed. This is a specialized proof checker, not
a general Verilog equivalence tool. See docs/MAC_COMPOSED_PROOF.md.
"""
import argparse
from collections import Counter
from dataclasses import dataclass
from datetime import datetime, timezone
import hashlib
import json
import os
from pathlib import Path
import re
import subprocess


# Number of input pins; remaining pins are outputs, in emitted positional order.
PINS = {'gen_and2': (2, 1), 'gen_or2': (2, 1), 'gen_xor2': (2, 1),
        'gen_xor3': (3, 1), 'gen_mux2': (3, 1), 'gen_ha': (2, 2),
        'gen_fa': (3, 2), 'gen_famux': (3, 2), 'gen_pg': (2, 2),
        'gen_black': (4, 2), 'gen_gray': (3, 1)}
CONST = {"1'b0", "1'b1"}


def require(condition, message):
    if not condition:
        raise ValueError(message)


@dataclass
class Cell:
    kind: str
    name: str
    pins: list

    @property
    def inputs(self):
        return self.pins[:PINS[self.kind][0]]

    @property
    def outputs(self):
        return self.pins[PINS[self.kind][0]:]

    def verilog(self):
        return f'{self.kind} {self.name}({", ".join(self.pins)});'


def parse(source):
    # No preprocessing, escaped identifiers, implicit assignments, or expressions.
    source = re.sub(r'//[^\n]*', '', source)
    lines = [line.strip() for line in source.splitlines() if line.strip()]
    header = re.fullmatch(r'module (\w+)\(input wire \[(\d+):0\] a, input wire \[(\d+):0\] b, input wire \[(\d+):0\] aux, output wire \[(\d+):0\] y\);', lines[0])
    require(header is not None, 'unsupported module header')
    top, ah, bh, ch, yh = header.groups()
    w = int(ah) + 1
    require(w >= 4 and w % 2 == 0 and (int(bh), int(ch), int(yh)) == (w-1, 2*w-1, 3*w+2), 'unexpected MAC dimensions')
    decl = re.fullmatch(r'wire \[(\d+):0\] n;', lines[1])
    require(decl is not None and lines[-1] == 'endmodule', 'unsupported wire declaration or module end')
    nmax = int(decl[1])
    available = CONST | {f'{port}[{i}]' for port, size in (('a', w), ('b', w), ('aux', 2*w)) for i in range(size)}
    instances, cells, outputs = set(), [], {}
    for line in lines[2:-1]:
        assignment = re.fullmatch(r'assign y\[(\d+)\] = (.+);', line)
        if assignment:
            i, wire = int(assignment[1]), assignment[2]
            require(i not in outputs and 0 <= i < 3*w+3 and wire in available, 'invalid output assignment')
            outputs[i] = wire
            continue
        match = re.fullmatch(r'(gen_\w+) (\w+)\(([^;]+)\);', line)
        require(match is not None and not outputs, 'unknown syntax or cell after outputs')
        kind, name, pins = match.groups()
        require(kind in PINS and name not in instances, 'unknown cell or duplicate instance')
        instances.add(name)
        cell = Cell(kind, name, [p.strip() for p in pins.split(',')])
        require(len(cell.pins) == sum(PINS[kind]), 'wrong port count')
        require(all(p in available for p in cell.inputs), 'undriven/forward input')
        for pin in cell.outputs:
            net = re.fullmatch(r'n\[(\d+)\]', pin)
            require(net is not None and int(net[1]) <= nmax and pin not in available, 'invalid/duplicate driver')
            available.add(pin)
        cells.append(cell)
    require(set(outputs) == set(range(3*w+3)), 'missing output bits')
    return top, w, nmax, cells, outputs


def decompose(source):
    top, w, nmax, cells, outputs = parse(source)
    ledger = {f'aux[{i}]': i for i in range(2*w)}
    products = set()
    index = 0
    for cell in cells:
        if cell.kind != 'gen_and2':
            break
        a = re.fullmatch(r'a\[(\d+)\]', cell.inputs[0])
        b = re.fullmatch(r'b\[(\d+)\]', cell.inputs[1])
        require(a is not None and b is not None, 'unexpected partial-product AND')
        pair = (int(a[1]), int(b[1]))
        require(pair not in products, 'duplicate partial product')
        products.add(pair)
        ledger[cell.outputs[0]] = sum(pair)
        index += 1
    require(products == {(i, j) for i in range(w) for j in range(w)}, 'missing partial products')
    compressor_count = 0
    while index < len(cells) and cells[index].kind in ('gen_ha', 'gen_fa', 'gen_famux'):
        cell = cells[index]
        require(len(set(cell.inputs)) == len(cell.inputs), 'compressor reuses an input')
        require(all(p in ledger for p in cell.inputs), 'compressor consumes missing/reused bit')
        weights = {ledger.pop(p) for p in cell.inputs}
        require(len(weights) == 1, 'compressor mixes column weights')
        weight = weights.pop()
        ledger[cell.outputs[0]] = weight
        ledger[cell.outputs[1]] = weight + 1
        index += 1
        compressor_count += 1
    require(ledger and max(ledger.values()) < 2*w+1, 'boundary exceeds MAC sum width')
    require(max(Counter(ledger.values()).values()) <= 2, 'boundary has more than two rows')
    # Tail may use only live frontier bits, constants, and its own prior outputs.
    available = set(ledger) | CONST
    used = set()
    for cell in cells[index:]:
        require(all(p in available for p in cell.inputs), 'tail bypasses compressor boundary')
        used.update(cell.inputs)
        available.update(cell.outputs)
    require(all(p in available for p in outputs.values()), 'output bypasses compressor boundary')
    used.update(outputs.values())
    require(set(ledger) <= used, 'unconsumed weighted boundary bit')
    # Actual unsigned inputs cannot overflow the specified total/rounding width.
    maximum = ((1 << w)-1)**2 + (1 << (2*w))-1
    require(maximum + (1 << (w//2-1)) < (1 << (2*w+1)), 'reference rounding can overflow')
    return {'top': top, 'width': w, 'nmax': nmax, 'cells': cells[index:],
            'outputs': outputs, 'ledger': ledger, 'compressors': compressor_count}


def tail_miter(proof):
    w = proof['width']
    boundary = sorted(proof['ledger'], key=lambda p: (proof['ledger'][p], p))
    rows = [["1'b0"] * (2*w+1) for _ in range(2)]
    lines = [f'module miter(input [{len(boundary)-1}:0] cut, output bad);',
             f'wire [{proof["nmax"]}:0] n;', f'wire [{2*w-1}:0] aux;',
             f'wire [{3*w+2}:0] actual;']
    for i, pin in enumerate(boundary):
        weight = proof['ledger'][pin]
        row = 0 if rows[0][weight] == "1'b0" else 1
        rows[row][weight] = f'cut[{i}]'
        lines.append(f'assign {pin} = cut[{i}];')
    lines.extend(c.verilog() for c in proof['cells'])
    lines.extend(f'assign actual[{i}] = {pin};' for i, pin in sorted(proof['outputs'].items()))
    for i, row in enumerate(rows):
        lines.append(f'wire [{2*w}:0] row{i} = {{{", ".join(reversed(row))}}};')
    lines.extend([
        f'wire [{2*w}:0] total = row0 + row1;',
        f'wire [{2*w}:0] rounded = total + {2*w+1}\'d{1 << (w//2-1)};',
        f'wire [{2*w}:0] scaled = rounded >> {w//2};',
        f'wire saturated = scaled > {2*w+1}\'d{(1 << w)-1};',
        f'wire [{w-1}:0] result = saturated ? {w}\'d{(1 << w)-1} : scaled[{w-1}:0];',
        f'wire status = saturated | total[{2*w}] | (|total[{w//2-1}:0]);',
        'assign bad = actual !== {status, saturated, result, total};', 'endmodule',
    ])
    return '\n'.join(lines) + '\n'


CELL_MITER = '''module miter(input a, b, c, output bad);
wire pp, hs, hc, fs, fc, ms, mc;
gen_and2 pp_dut(a, b, pp);
gen_ha ha_dut(a, b, hs, hc);
gen_fa fa_dut(a, b, c, fs, fc);
gen_famux mux_dut(a, b, c, ms, mc);
wire [2:0] two_inputs = {2'b0,a} + {2'b0,b};
wire [2:0] three_inputs = two_inputs + {2'b0,c};
assign bad = (pp !== (a & b)) | ({1'b0,hc,hs} !== two_inputs)
           | ({1'b0,fc,fs} !== three_inputs) | ({1'b0,mc,ms} !== three_inputs);
endmodule
'''


def sat(directory, cells, miter, seconds):
    directory.mkdir(parents=True)
    (directory / 'cells.v').write_text(cells)
    (directory / 'miter.v').write_text(miter)
    command = ('read_verilog cells.v miter.v; attrmap -modattr -remove keep_hierarchy=1; '
               'hierarchy -check -top miter; proc; flatten; opt -keepdc; techmap; opt -keepdc; check -assert; '
               f'sat -enable_undef -set-def-inputs -timeout {seconds} -prove bad 0 -show-inputs -show-outputs')
    (directory / 'command.ys').write_text(command + '\n')
    env = {**os.environ, 'OMP_NUM_THREADS': '1', 'OPENBLAS_NUM_THREADS': '1',
           'VECLIB_MAXIMUM_THREADS': '1'}
    with (directory / 'yosys.log').open('w') as log:
        try:
            result = subprocess.run(['nice', '-n', '19', 'yosys', '-Q', '-s', 'command.ys'],
                                    cwd=directory, env=env, stdout=log, stderr=subprocess.STDOUT,
                                    timeout=seconds+15)
        except subprocess.TimeoutExpired:
            return 'timeout'
    output = (directory / 'yosys.log').read_text()
    if 'SAT proof finished - model found: FAIL!' in output:
        return 'counterexample'
    if result.returncode == 0 and 'SAT proof finished - no model found: SUCCESS!' in output:
        return 'proved'
    if 'timeout' in output.lower():
        return 'timeout'
    return 'error'


def digest(data):
    return hashlib.sha256(data).hexdigest()


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('sources', type=Path, nargs='+')
    parser.add_argument('--cells', type=Path, required=True)
    parser.add_argument('--output', type=Path, required=True)
    parser.add_argument('--seconds', type=int, default=20, choices=range(1, 61))
    args = parser.parse_args()
    require(not args.output.exists(), 'output already exists; preserve prior attempts')
    cells = args.cells.read_text()
    sources = [(source, source.read_bytes()) for source in args.sources]
    require(len({s.stem for s, _ in sources}) == len(sources), 'duplicate source stems')
    args.output.mkdir(parents=True)
    record = {'started_utc': datetime.now(timezone.utc).isoformat(),
              'checker_sha256': digest(Path(__file__).read_bytes()),
              'cells_sha256': digest(args.cells.read_bytes()),
              'yosys': subprocess.check_output(['yosys', '-V'], text=True).strip(),
              'concurrent_jobs': 1, 'threads': 1, 'sat_timeout_seconds': args.seconds,
              'wall_timeout_seconds': args.seconds+15,
              'source_sha256': {str(s): digest(data) for s, data in sources},
              'results': []}
    record_path = args.output / 'summary.json'
    def save():
        record_path.write_text(json.dumps(record, indent=2) + '\n')
    save()
    record['cell_lemmas'] = sat(args.output / 'cell_lemmas', cells, CELL_MITER, args.seconds)
    save()
    require(record['cell_lemmas'] == 'proved', 'cell identities not proved')
    for source, data in sources:
        row = {'source': str(source), 'status': 'structural_rejection'}
        try:
            proof = decompose(data.decode())
            miter = tail_miter(proof)
            row.update(width=proof['width'], compressors=proof['compressors'],
                       boundary_bits=len(proof['ledger']), tail_miter_sha256=digest(miter.encode()))
            row['status'] = sat(args.output / source.stem, cells, miter, args.seconds)
            row['weighted_boundary'] = proof['ledger']
        except ValueError as error:
            row['error'] = str(error)
        record['results'].append(row)
        save()
        print(source.stem, row['status'], flush=True)
    require(all(r['status'] == 'proved' for r in record['results']), 'not all composed proofs completed')


if __name__ == '__main__':
    main()
