"""Adversarial checks for the specialized proof checker, run serially."""
from pathlib import Path
import re
import tempfile
import unittest

import prove_mac_composed as checker


ROOT = Path(__file__).resolve().parents[1]
HELDOUT = ROOT / 'results/discovery/mac_crosswidth_20260905/evo608_heldout'
SOURCE = (HELDOUT / 'mul24_mac_replay_candidate.v').read_text()
CELLS = (HELDOUT / 'cells.v').read_text()


class StructuralRejection(unittest.TestCase):
    def test_frozen_source_parses(self):
        proof = checker.decompose(SOURCE)
        self.assertEqual(proof['width'], 24)
        self.assertGreater(proof['compressors'], 0)

    def test_missing_or_duplicate_partial_product(self):
        altered = SOURCE.replace('gen_and2 u1(a[1], b[0], n[1]);',
                                 'gen_and2 u1(a[0], b[0], n[1]);')
        with self.assertRaisesRegex(ValueError, 'duplicate partial product'):
            checker.decompose(altered)

    def test_compressor_reuse(self):
        pattern = r'(gen_famux \w+\()([^,]+), ([^,]+),'
        altered, count = re.subn(pattern, r'\1\2, \2,', SOURCE, count=1)
        self.assertEqual(count, 1)
        with self.assertRaisesRegex(ValueError, 'reuses an input'):
            checker.decompose(altered)

    def test_wrong_column_weight(self):
        # Auxiliary inputs are initially live; replace one compressor input
        # with an aux bit from a different column without duplicating a pin.
        match = re.search(r'gen_famux \w+\(([^,]+), ([^,]+), ([^,]+),', SOURCE)
        altered = SOURCE[:match.start(1)] + 'aux[47]' + SOURCE[match.end(1):]
        with self.assertRaisesRegex(ValueError, 'mixes column weights'):
            checker.decompose(altered)

    def test_tail_bypasses_frontier(self):
        altered = re.sub(r'(gen_pg \w+\()[^,]+,', r'\1a[0],', SOURCE, count=1)
        with self.assertRaisesRegex(ValueError, 'bypasses compressor boundary'):
            checker.decompose(altered)

    def test_duplicate_driver(self):
        altered = SOURCE.replace('gen_and2 u1(a[1], b[0], n[1]);',
                                 'gen_and2 u1(a[1], b[0], n[0]);')
        with self.assertRaisesRegex(ValueError, 'duplicate driver'):
            checker.decompose(altered)

    def test_unsupported_verilog_is_rejected(self):
        altered = SOURCE.replace('endmodule', 'assign n[0] = 1\'b0;\nendmodule')
        with self.assertRaises(ValueError):
            checker.decompose(altered)

    def test_missing_output_is_rejected(self):
        altered = re.sub(r'    assign y\[74\] = [^;]+;\n', '', SOURCE)
        with self.assertRaisesRegex(ValueError, 'missing output bits'):
            checker.decompose(altered)

    def test_oversized_boundary_is_rejected(self):
        # This malformed header must not quietly change the reference width.
        altered = SOURCE.replace('output wire [74:0] y', 'output wire [75:0] y')
        with self.assertRaisesRegex(ValueError, 'dimensions'):
            checker.decompose(altered)


class SatCounterexamples(unittest.TestCase):
    def run_sat(self, cells, miter):
        with tempfile.TemporaryDirectory(prefix='mac-proof-negative-') as tmp:
            return checker.sat(Path(tmp) / 'proof', cells, miter, 10)

    def test_mutated_cell_lemma_fails(self):
        altered = CELLS.replace('assign c = p ? cin : a;', "assign c = 1'b0;")
        self.assertNotEqual(altered, CELLS)
        self.assertEqual(self.run_sat(altered, checker.CELL_MITER), 'counterexample')

    def test_undefined_cell_output_fails(self):
        altered = CELLS.replace('assign c = p ? cin : a;', "assign c = 1'bx;")
        self.assertNotEqual(altered, CELLS)
        self.assertEqual(self.run_sat(altered, checker.CELL_MITER), 'counterexample')

    def test_high_status_corruption_fails(self):
        altered = re.sub(r'assign y\[74\] = [^;]+;', "assign y[74] = 1'b0;", SOURCE)
        # Structurally valid corruption must be caught by the semantic proof.
        miter = checker.tail_miter(checker.decompose(altered))
        self.assertEqual(self.run_sat(CELLS, miter), 'counterexample')

    def test_sum_bit_corruption_fails(self):
        altered = re.sub(r'assign y\[48\] = [^;]+;', "assign y[48] = 1'b0;", SOURCE)
        miter = checker.tail_miter(checker.decompose(altered))
        self.assertEqual(self.run_sat(CELLS, miter), 'counterexample')


if __name__ == '__main__':
    unittest.main()
