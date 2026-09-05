# Bounded prior-art optimizer comparison: evo608 leaves the frontier

A control generated through ARITH-DAS's UFO-MAC replication **dominates evo608
under the default recipe**, both before and after global routing. The former
candidate no longer extends the expanded measured frontier.

| Default recipe | evo608 delay / area | UFO-MAC-derived BrentKung/mux | Improvement over evo608 |
|---|---:|---:|---:|
| Mapped | 10140 ps / 19884.07 um² | 9590 ps / 19842.78 um² | 5.42% faster, 0.21% smaller |
| Global route | 15250 ps / 19884 um² | 14010 ps / 19843 um² | **8.13% faster, 0.21% smaller** |

The routed area difference is only 41 um²; no placement-variability or statistical
significance claim follows. The 1240 ps delay difference is a result of this
fixed flow, not a universal performance guarantee.

## Comparison scope

The upstream code is the official ARITH-DAS repository at commit
`5b19ba12cb587e173606d432f3d3b7046c03ad85`. Its compressor methods explicitly
identify themselves as a replication of UFO-MAC (ICCAD 2024). The exact selected
method ASTs execute unchanged. A bounded HiGHS solver adapter, full 48-bit
accumulator heap, characterized cell-pin delays, and common final-adder/output
logic adapt that replication to this contract. See
[the adapter and upstream citations](../../../integrations/arithdas/README.md).

Stage assignment found an optimum of seven stages in 0.97 seconds. The
interconnection ILP stopped after 90.41 seconds with a validated integer solution:
2.946943 ns model objective, 1.109614501 ns lower bound, **62.35% gap**. PuLP's
coarse `Optimal` model status must not be mistaken for an optimality proof;
`Solution Found`, the raw HiGHS `Time limit reached` log, and the gap are retained.
The model objective is not the measured whole-design delay.

Standalone mux and XorMaj FAs mapped to identical pin-delay profiles, so a single
optimizer solution was reused for both. Six controls combine those two FA
implementations with Sklansky, BrentKung, and KoggeStone CPAs. All six controls
were mapped and globally routed under both recipes: 12 mappings and 12 routes.
No failing optimizer attempt or mapped/routed form was dropped.

The full MAC contract remains unsigned 24×24 multiplication, 48-bit accumulation,
49-bit full sum, fixed rounding/shift, 24-bit saturation, and status: 75 live
output bits. All controls use the same requantization roots as evo608.

![Measured default frontier](frontier.png)

## Frontier and fairness

[The combined default table](default_combined_frontier.tsv) contains 51 designs:
evo608, its ablation, the earlier 43 baselines, and six new bounded-optimizer
controls. Only the new BrentKung/mux control dominates evo608, and that control
is itself nondominated. Other points remain faster at larger area or smaller at
longer delay. Thus the new control advances this measured frontier; evo608 does
not retain its prior small extension.

Candidate timing/area are reused from the completed frozen campaign; the emitted
candidate still matches its frozen RTL byte for byte after the shared-root
refactor. The same Liberty, route-flow script, Docker image, synthesis recipes,
utilization 45, density 0.55, input slew, and output loads apply. The new results
use estimated global-route parasitics, not detailed-route extraction or signoff.

Under classic mapping none of these six new controls dominates evo608, but the
previously routed Wallace/BrentKung/mux control still does (11500 ps / 28836 um²
versus evo608's 11600 ps / 30415 um²). That known loss is not erased by this run.

## Verification and resource use

- All six imported forms passed 4096 deterministic random vectors plus directed
  arithmetic boundary cases, then independent composed all-input proofs.
- All twelve mapped forms passed direct ABC equivalence to their source RTL,
  checking all 96 input bits and all 75 output bits.
- All twelve global routes completed. Five existing MAC regression tests passed,
  including exhaustive W4 variants and mutation cases and W24/W32 corruption checks.
- Main lane only; serial jobs; one solver/Yosys/ABC thread; two-core container/
  OpenROAD limits; Cargo at most two build jobs; low-priority local runs. No
  parallel training or extra optimizer restart was launched.

Proof trust boundaries are described in
[MAC_COMPOSED_PROOF.md](../../../docs/MAC_COMPOSED_PROOF.md). Source hashes,
solver statuses/logs, protocols, graphs, emitted RTL, proof commands/results,
and physical reports are retained. `report_corrections.json` documents a
routed-endpoint metadata correction and preserves the exact executed runner;
timing/area measurements were unchanged. Large layout databases and redundant mapped
snapshots remain local. The paper archive includes the selected evidence and
upstream MIT source/license; the portable benchmark adds all six emitted controls.

## What this changes

The old 1.55% speed / +0.58% area tradeoff was frontier survival against a finite
baseline set. It was not a durable advantage over prior-art optimization. This
bounded comparison is enough to refute that stronger interpretation, despite
its incomplete optimization.

Retain evo608 as a verified ablation/flow-sensitivity example. Promote the
14010 ps / 19843 um² control to a stronger baseline and provisional replacement
benchmark point. The result strengthens the need for strong baselines in the
case-study manuscript; it weakens any claim of a competitive novel architecture.

Limitations: this is ARITH-DAS's UFO-MAC compressor replication, not the complete
original UFO-MAC flow or trained ARITH-DAS. It uses a single bounded solve,
fixed-load linear pin timing, zero initial heap arrivals, and three existing
CPAs rather than UFO-MAC's arrival-profile-optimized CPA. The candidate's search
budget was not matched to this solve. No superiority to either complete prior-art
method, cross-library transfer, statistical robustness, or novelty is claimed.

The next useful evaluation would challenge the replacement control under the
previous 35/45/50 utilization settings and verify whether the advantage persists.
Only then consider a larger optimizer budget or a different technology library.
