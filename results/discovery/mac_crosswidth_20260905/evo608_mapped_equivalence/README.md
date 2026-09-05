# Frozen SKY130 mapped-netlist equivalence

All eight held-out candidate/sibling mappings are equivalent to their
composed-proof structural RTL under the recorded Liberty Boolean functions.

| Width | Form | Recipe | Inputs / outputs checked | ABC verdict |
|---:|---|---|---|---|
| 12 | candidate | default | 48 / 39 | proved |
| 12 | candidate | classic | 48 / 39 | proved |
| 12 | norules | default | 48 / 39 | proved |
| 12 | norules | classic | 48 / 39 | proved |
| 24 | candidate | default | 96 / 75 | proved |
| 24 | candidate | classic | 96 / 75 | proved |
| 24 | norules | default | 96 / 75 | proved |
| 24 | norules | classic | 96 / 75 | proved |

The W=24/default hashes also match the candidate and sibling netlists used
in the completed 42-job routing experiment. All other result/source/library
hashes and actual tool versions are retained in `summary.json`.

Method: Yosys elaborates structural RTL and the mapped circuit using the
actual Liberty cell functions, checks hierarchy/drivers, and emits AIGs.
ABC performs direct combinational equivalence; internal same-name wires are
not assumed equivalent. Both AIG interfaces are checked for every a/b/aux/y
bit before CEC. No resynthesis or search is performed.

Two negative regression tests pass: losing an output bit is rejected, and
inverting the highest mapped output produces an ABC counterexample.
Runs are serial and low priority, with one compute thread, 20-second ABC
budgets, and 35-second process budgets. The earlier Yosys partitioned proof
attempts timed out locally; changing to direct ABC CEC resolved the check.

Scope: two-state Boolean equivalence of these exact mapped inputs, composed
with the prior structural proof. This does not verify routing database
connectivity, analog timing, power, other library corners, or all controls.
Undefined/unconnected net drivers are checked by Yosys; the recorded
combinational Liberty functions define the gate model. Cells lacking a
Liberty function are skipped during library import, but any such cell used
in the design must still pass hierarchy checking and AIG conversion.

Reproduce from the research repository with a fresh output directory:
`python3 scripts/prove_mac_mapped.py --output /tmp/mac-mapped-proof`.
The original mapped netlists and matching Liberty are required. Each result
folder retains gold/gate/cells RTL and Yosys/ABC commands; the library is
identified by hash and is not redistributed here.
