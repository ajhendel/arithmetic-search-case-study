# When stronger baselines remove an arithmetic-search frontier result

Andrew Hendel — software and benchmark archival report, September 2026.
Released as an open-source archival artifact; not peer reviewed.

## Question and outcome

Can a compact, evolved compressor-scheduling rule produce an exact
multiply-accumulate implementation that remains competitive after synthesis
and physical estimation?

The tested rule improved some matched comparisons against its own no-rule
ablation. It also survived an initially limited set of textbook and coarse
arrival-aware controls. A stronger, bounded prior-art-derived comparator then
dominated it. The evidence does not establish a novel competitive arithmetic
architecture. This release preserves the useful software and the negative result.

## Workload and method

The principal benchmark takes unsigned 24-bit operands `a` and `b` and a 48-bit
accumulator. It exposes the exact 49-bit `a*b + accumulator`, rounds by adding
2048 before a 12-bit shift, saturates the scaled value to 24 bits, and returns
saturation and status. All 75 output bits remain live in every comparison.

The candidate `evo608` uses a bounded FullNoHa/Earliest scheduling override
within a mostly Dadda/positional compressor schedule. It uses a Sklansky final
adder, mux full adders, fused accumulation, and shared output logic. Its exact
ablation removes only the scheduling override.

The experiments compare frozen RTL through the same SKY130 library, Yosys/ABC
recipes, loads and input transitions, and OpenROAD global-route estimation.
The principal physical endpoint uses utilization 45% and placement target
density 0.55. These are global-route parasitic estimates, not signoff or silicon
measurements. Historical campaigns test other widths and show flow-dependent
ranking reversals; they are preserved as context rather than general conclusions.

## What stronger comparisons changed

In the default recipe, evo608 reaches 15.25 ns / 19,884 µm² versus its ablation's
17.35 ns / 19,654 µm²: 12.10% lower delay at 1.17% more area. Adding coarse
arrival-aware controls shrinks the nearby tradeoff to 1.55% lower delay at
0.58% more area. Frontier survival in those finite sets did not mean global
optimality or superiority to prior-art optimization.

The next comparison uses the UFO-MAC compressor replication in the official
[ARITH-DAS repository](https://github.com/MIRALab-USTC/Arith-DAS), pinned to
`5b19ba12cb587e173606d432f3d3b7046c03ad85`. Its selected allocation, stage-assignment,
and interconnection method bodies run unchanged. The adapter supplies the full
48-bit accumulator heap, characterized asymmetric cell-pin delays, and a bounded
single-thread HiGHS solver. The shared local final-adder and requantization
implementations preserve the output contract.

Stage assignment reaches a proven seven-stage optimum. Interconnection stops at
90 seconds with a feasible solution and 62.35% gap; there is no optimality claim.
Six controls combine that topology with two FA implementations and three CPAs.
All twelve mapped and routed outcomes under two recipes are retained.

A default-flow BrentKung/mux control reaches **14.01 ns / 19,843 µm²**, dominating
evo608 by 8.13% lower delay and 0.21% less area. This control is nondominated in
the combined 51-design default comparison. Under classic mapping, an earlier
Wallace/BrentKung/mux control already dominates evo608 after global routing.

This refutes the interpretation that evo608 is a durable extension of the tested
arithmetic frontier. It does not establish the complete UFO-MAC method's best
result, trained ARITH-DAS performance, or a budget-matched comparison of search
methods. The adapted comparator uses only three existing CPAs rather than the
original UFO-MAC's targeted final-adder optimization.

## Correctness and limits

The independent composed checker establishes complete partial-product coverage,
weighted conservation through HA/FA cells, and all-output equivalence of the
remaining tail for arbitrary two-row inputs. It is a specialized trusted checker
plus Yosys SAT, not a proof-assistant-checked theorem. Selected mapped netlists
also pass direct ABC equivalence to their source RTL.

The final comparator campaign includes six structural proofs, twelve mapped
proofs, twelve completed routes, and five existing MAC regression tests. Earlier
candidate/ablation proofs and additional comparisons are retained with their
original scopes. These checks establish the tested circuits' functionality;
they do not establish timing, power, manufacturing, or novelty claims.

The small area margin has no demonstrated statistical robustness. There are no
independent physical replications of the replacement control, no signoff or
silicon measurements, and no broad technology-library sweep. The experiments
were not designed to estimate population-wide search effectiveness.

## Disposition and reuse

The candidate-focused research line is closed. The reusable outputs are the
generator, exact benchmark contract, proof tooling, prior-art adapter, and
selected comparison evidence. They may help others evaluate search proposals
against stronger baselines, including cases where an initial gain disappears.
A DOI will identify this software-and-evidence artifact; it should not be read
as peer review or certification of novelty.

[Detailed results](results/discovery/mac_ufomac_challenge_20260905/RESULTS.md),
[prior-art assessment](paper/PRIOR_ART.md), and
[reproduction guide](REPRODUCING.md) provide the implementation and evidence trail.
