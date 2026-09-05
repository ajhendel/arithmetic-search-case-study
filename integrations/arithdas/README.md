# Bounded UFO-MAC compressor comparison through ARITH-DAS

This adapter evaluates the UFO-MAC replication shipped by the official
[ARITH-DAS repository](https://github.com/MIRALab-USTC/Arith-DAS), commit
`5b19ba12cb587e173606d432f3d3b7046c03ad85`. The unmodified source file and its
MIT license are under `vendor/`. `scripts/optimize_mac_ufomac.py` loads only the
allocation, stage-assignment, interconnection, and formatting methods via AST.
It omits training imports and the abstract base class; the selected method ASTs
are unchanged. No PyTorch installation or learned-optimizer training is needed.

This is a bounded comparison to that implementation of UFO-MAC's compressor
optimization, not a reproduction of the original UFO-MAC or ARITH-DAS results.
The original paper is [Zuo et al., ICCAD 2024](https://arxiv.org/abs/2408.06935).
The original UFO-MAC's targeted CPA optimization and ARITH-DAS's learned search
are not run here. A win against these controls cannot be advertised as a win
against either complete method.

## Explicit adaptations

- Our input heap has all 24×24 unsigned AND partial products, 48 accumulator
  bits, and a 49th output column to preserve carry. Upstream `Mac` supplies only
  a 24-bit accumulator, so its wrapper is not used.
- Upstream allocation and ILPs take this heap directly. Stage assignment is
  limited to 12 stages, with M=64 and a 60-second solver cap. Interconnection
  uses Z=100 ns, a 90-second cap, one HiGHS thread, and random seed zero.
- Standalone FAs and HA are characterized through the recorded default Yosys/
  ABC mapping and SKY130 TT library, at input slew 0.05 ns and load 0.01 pF.
  Six asymmetric FA and four HA pin/output delays populate the existing linear
  timing model. This model does not account for dynamic fanout, slew, or wires.
  The upstream model's zero initial PP/accumulator arrivals remain unchanged.
- Mux and XorMaj standalone FAs mapped to identical profiles. One optimized
  interconnection is therefore reused for both implementations; no duplicate
  solve was run. This does not imply that whole-design mapping will be identical.
- HiGHS replaces upstream's Gurobi command interface. Every variable and
  constraint is checked for feasibility before export; integral values within
  tolerance are rounded to avoid upstream `int(value)` truncation. The solver
  gap and raw log are retained. PuLP labels the time-limited incumbent `Optimal`
  at its coarse model-status level, but its solution status is `Solution Found`;
  the native log and nonzero gap explicitly show that optimality was not proved.
- The upstream router emits assignments and FA/HA connections. The adapter
  resolves aliases into a named graph without rescheduling or changing pins.
  The Rust importer attaches the existing Sklansky/BrentKung/KoggeStone CPA and
  exactly the candidate's requantization roots. It generates six controls.
  This three-CPA sweep is not an arrival-profile-optimized final adder. The
  generic RTL header retains the internal `dadda` metadata tag; the imported
  compressor connections come from the external graph, not a Dadda scheduler.

## Correctness and evaluation

The refactored root attachment reproduces the frozen candidate byte for byte.
Each imported design must pass random/directed reference checks and the existing
independent composed proof (complete partial products, weighted conservation,
and arbitrary two-row tail SAT). Each mapped form must also pass direct ABC
RTL/netlist equivalence before routing. The full 75-bit output contract is live.
Only the main lane runs checks or measurements, one job at a time; local
synthesis is one thread and containers/OpenROAD use at most two cores.

[Campaign evidence](../../results/discovery/mac_ufomac_challenge_20260905/RESULTS.md)
records the planned comparisons, outcomes, and limitations. Protocol and source
hashes were recorded before the corresponding optimizer/evaluation runs.

The optimizer requires Python, NumPy, PuLP, and highspy (HiGHS). The measured run
uses the exact versions in the campaign's environment record. The PDK and tool
image are external dependencies. Large layout databases and duplicate proof
netlists are omitted from the curated archive.

For a fresh reproduction, use a separate checkout, move the existing campaign
folder outside the checkout, restore its protocol and characterization inputs,
then run characterization and `scripts/optimize_mac_ufomac.py`, followed by
`scripts/compare_mac_ufomac.py`. These scripts deliberately refuse to overwrite
completed stage outputs. The retained `optimized_graph.json` also permits replay
without solving: run `cargo run --locked --release --example mac_import_ufomac --
<genome.json> <optimized_graph.json> <fresh-rtl-directory>` with
`CARGO_BUILD_JOBS=2`. Do not start reproduction alongside another measurement job.
