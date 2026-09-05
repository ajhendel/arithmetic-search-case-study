"""Negative regression checks for the bounded mapped-netlist CEC runner."""
from pathlib import Path
import tempfile
import unittest

import prove_mac_mapped as checker


ROOT = Path(__file__).resolve().parents[1]
HELDOUT = ROOT / checker.HELDOUT


class MappedChecks(unittest.TestCase):
    def test_wrong_port_count_is_rejected(self):
        with tempfile.TemporaryDirectory() as tmp:
            path = Path(tmp) / 'wrong.aag'
            path.write_text('aag 0 96 0 74 0\n')
            with self.assertRaisesRegex(AssertionError, 'lost MAC ports'):
                checker.aiger_ports(path, 24)

    def test_corrupt_high_mapped_output_is_not_proved(self):
        source = HELDOUT / 'mul24_mac_replay_candidate.v'
        gate = HELDOUT / 'physical/mapped/mul24_mac_replay_candidate_default.v'
        top = gate.stem
        corrupted = gate.read_text().replace(f'module {top}(', f'module {top}_inner(', 1)
        wrapper = f'''\nmodule {top}(input [23:0] a, b,
input [47:0] aux, output [74:0] y);
wire [74:0] raw;
{top}_inner inner(a, b, aux, raw);
assign y = raw ^ (75'd1 << 74);
endmodule
'''
        with tempfile.TemporaryDirectory(prefix='mac-lec-negative-') as tmp:
            tmp = Path(tmp)
            mapped = tmp / f'{top}.v'
            mapped.write_text(wrapper + corrupted)
            result = checker.prove_pair(
                source, mapped, HELDOUT / 'cells.v',
                ROOT / 'pdk/sky130_fd_sc_hd__tt_025C_1v80.lib',
                tmp / 'proof', 24, seconds=10)
            self.assertEqual(result['status'], 'counterexample')


if __name__ == '__main__':
    unittest.main()
