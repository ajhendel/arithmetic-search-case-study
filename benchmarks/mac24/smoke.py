#!/usr/bin/env python3
"""Serial, bounded Icarus RTL smoke test; works in the repo or exported bundle."""
import os
from pathlib import Path
import subprocess
import tempfile


def main():
    here = Path(__file__).resolve().parent
    rtl = here / 'rtl'
    if not rtl.is_dir():
        rtl = here.parents[1] / 'results/discovery/mac_crosswidth_20260905/evo608_heldout'
    env = {**os.environ, 'OMP_NUM_THREADS': '1', 'OPENBLAS_NUM_THREADS': '1'}
    with tempfile.TemporaryDirectory(prefix='mac24-smoke-') as tmp:
        executable = str(Path(tmp) / 'smoke.vvp')
        sources = [rtl / name for name in ('cells.v', 'mul24_mac_replay_candidate.v',
                                           'mul24_mac_replay_norules.v')]
        subprocess.run(['nice', '-n', '19', 'iverilog', '-g2012', '-s', 'smoke_tb',
                        '-o', executable, *map(str, sources), str(here / 'reference.v'),
                        str(here / 'smoke_tb.sv')], check=True, timeout=30, env=env)
        subprocess.run(['nice', '-n', '19', 'vvp', executable], check=True,
                       timeout=60, env=env)


if __name__ == '__main__':
    main()
