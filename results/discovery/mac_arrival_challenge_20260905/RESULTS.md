# W24 candidate challenged by arrival-aware controls

Subsequent result: [the bounded prior-art optimizer challenge](../mac_ufomac_challenge_20260905/RESULTS.md)
now dominates evo608 under default mapping and routing. The frontier survival
reported below applies to the earlier baseline set.

The frozen evo608 candidate survives the expanded **default-recipe** measured
area/delay frontier. It does **not** survive the classic-recipe frontier:
a previously identified textbook control is both faster and smaller after routing.
This is evidence for a recipe-specific candidate and a reproducible comparison
artifact. It does not establish a novel compressor scheduler or a general winner.

## What was compared

The circuit computes unsigned 24-bit multiply plus 48-bit accumulation, followed
by rounding, a 12-bit shift, saturation to 24 bits, and status generation. All
comparisons preserve the complete 75-bit output bundle, including the full sum.

Before generation, `protocol.json` fixed 12 rule-free, coarse arrival-aware
controls: Dadda/FullNoHa × Sklansky/BrentKung/KoggeStone × mux/XorMaj full adders.
They retain the candidate's root plan and use the existing Earliest depth model.
The unchanged candidate, its no-rule sibling, and behavioral reference complete
15 designs. All were mapped and globally routed under both recipes: 30 mappings
and 30 routes, without selecting a favorable subset. A separate, explicitly
supplementary route tested the sole older W24 classic mapped dominator.

## Routed comparisons

Area is standard-cell area in um²; timing uses estimated global-route parasitics
at utilization 45 and placement target density 0.55. These are not signoff results.

| Recipe | Design | Delay (ps) | Area (um²) |
|---|---|---:|---:|
| default | evo608 candidate | 15250 | 19884 |
| default | exact no-rule sibling | 17350 | 19654 |
| default | arrival-aware Dadda/Sklansky/mux | 15490 | 19769 |
| default | arrival-aware Dadda/KoggeStone/mux | 11850 | 21128 |
| default | behavioral reference | 20160 | 21338 |
| classic | evo608 candidate | 11600 | 30415 |
| classic | older Wallace/BrentKung/mux control | 11500 | 28836 |

With default mapping, the candidate is 12.10% faster at 1.17% more area than
its exact ablation. Against the nearby arrival-aware Dadda/Sklansky/mux control,
that advantage narrows to **1.55% faster at 0.58% more area**. A larger control
is much faster; Pareto survival does not mean lowest delay. Against behavioral
RTL under this fixed synthesis script, it is 24.36% faster and 6.81% smaller.
That larger gain must not be presented as a win over optimized arithmetic tools.

The combined default table contains the earlier 30 positional controls, these
12 arrival-aware controls, behavioral RTL, the ablation, and candidate: 45 rows.
No comparator dominates the candidate. The earlier and fresh campaigns have
identical image, library, Yosys, and route-flow identities; the repeated candidate
and ablation reproduce timing, area, and HPWL exactly. See
[the combined frontier](default_combined_frontier.tsv). The 43 baselines are a
finite comparison set; the ablation is an additional comparison.

Under classic mapping, the older Wallace/BrentKung/mux control is **0.86% faster
and 5.19% smaller** than the candidate. Its 100 ps timing advantage survives this
route, but is not a robustness estimate. See [the supplement](classic_neighbor/README.md).
There was no complete classic routing census of the older 30 controls.

## Correctness, resources, and retained evidence

All 14 structural designs passed composed all-input correctness checks; the
behavioral design is the arithmetic reference. All 30 mapped forms passed direct
ABC equivalence to their input RTL. The supplemental older control also passed
composed correctness and mapped equivalence before its route: **15 structural
proofs, 31 mapped equivalence checks, and 31 completed routes** in this campaign.
The proof trust boundaries remain those in
[MAC_COMPOSED_PROOF.md](../../../docs/MAC_COMPOSED_PROOF.md).

Only the main lane ran verification and measurements. Jobs were serial, at low
priority, with Yosys/ABC one thread, OpenROAD/container caps of two cores, and
Cargo at most two build jobs. Every mapping and route completed within its bound.

`mapped_summary.tsv`, `routed_summary.tsv`, proof JSON, solver commands/logs,
route timing reports/logs, source RTL/genomes, and frozen protocol hashes retain
the evidence. Large layout databases and redundant mapped-proof netlist/library
logs remain local and are excluded from the curated artifact. Reproduction
requires the recorded external SKY130 library and tool image. The primary runner
accepts a fresh output directory; the classic supplement uses the recorded
campaign paths and requires the earlier held-out mapped netlist to be regenerated.

## Interpretation and next gate

This comparison strengthens the evidence for a useful default-recipe benchmark
point while shrinking the apparent gain against a stronger nearby baseline.
It supports the manuscript's flow-sensitivity case study. It does not establish
novelty or justify claiming state-of-the-art performance.

The next substantive gate is an independently implemented, pin-delay-aware
three-greedy baseline or a compatible modern arithmetic optimizer, using the
same exact output contract and physical flow. The coarse arrival variants here
are not a faithful reproduction of that prior art. Placement variability and
other technology libraries remain unmeasured. Keep the portable benchmark and
prepared LogikBench proposal as the reuse route; public submission remains pending.
