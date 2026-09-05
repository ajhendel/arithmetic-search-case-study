# Second diversity-retaining MAC tournament

## Latest prior-art challenge supersedes frontier survival

A bounded UFO-MAC-derived control now dominates evo608 after default mapping
and routing: 14010 ps / 19843 um² routed, versus 15250 / 19884. The earlier
frontier-survival results below are historical finite-set outcomes.
[Full evidence](../mac_ufomac_challenge_20260905/RESULTS.md).

## Subsequent arrival-aware challenge

[The follow-up](../mac_arrival_challenge_20260905/RESULTS.md) adds 12 arrival-aware
controls and behavioral RTL under both recipes. Default frontier survival extends
to 43 baselines plus ablation; the nearby control reduces the advantage to 1.55%
delay for 0.58% additional area. The previously mapped-dominating classic control
also dominates after routing (11500 ps / 28836 um² versus 11600 / 30415).

Seed `1835098964`, 3,000 generations, 2,187 occupied niches, and 128 retained
candidates. The existing tournament mapped 256 preserved/flattened forms at
16 bits against the frozen SKY130 matched-control corpus.

Despite the directory name, this run does **not** select on both widths:
`select_mac_physical_tournament` uses 16-bit depth/gates and genome diversity.
Emission verifies W=4 exhaustively and W=8/16 with samples; it does not establish
formal equivalence at the larger widths.

## Frontier audit

Five candidate forms appear in `physical/frontier.tsv`. `evo1250_flat` exactly
ties an existing control (7,510 ps / 8,987.37 um2), so it is not a new physical
point. `evo1822`, `evo1776`, and `evo1086` have no developmental rules. Only
`evo608` is a rule-bearing frontier lead. These results are relative to the
prior control corpus, not a complete factorial exploration of output plans.

Its single rule applies FullNoHa compression with Earliest operand selection
in normalized columns 4–13, stages 2–5, at heights 4–10. The remainder of the
genome is held fixed in the no-rule comparison.

## Independent recipe and physical follow-up

The complete 16-mapping comparison is in `evo608_recipes/README.md`.
At W=16 the candidate improves pre-layout delay under all four recipes:
3.01% default, 14.44% classic, 3.34% resyn, and 3.24% area. At W=8, default
loses both delay and area; classic improves both; the other recipes trade them.
This does not establish a consistent cross-width advantage.

Matched placement and global routing with estimated parasitics give:

| recipe | candidate ps | no-rule ps | candidate area (um2) | no-rule area (um2) |
|---|---:|---:|---:|---:|
| default | 10300 | 10240 | 9124 | 9285 |
| classic | 7680 | 9240 | 14418 | 14156 |

Default reverses the timing lead (+0.59%) while saving 1.73% area. Classic
retains a 16.88% timing gain at 1.85% more area and 5.71% more placement HPWL.
Raw compact evidence is in `evo608_placement/summary.tsv`. This is global-route
estimation, not detailed-route signoff or silicon measurement.

`evo608` remains a recipe-dependent topology probe, not a tapeout candidate.
Before admission it needs scalable formal proof, matched physical controls
under the same recipes, and genuinely cross-width selection/qualification.

## Reproduction

Run from the repository root with Yosys and the OpenROAD Docker image available:

```sh
cargo run --release -- evolve-mac-requant-diverse 3000 1835098964 /tmp/mac_replay
# The original generated inputs and tournament results are retained here.
scripts/score_mac_tournament.sh results/discovery/mac_crosswidth_20260905
scripts/score_mac_candidate_recipes.sh results/discovery/mac_crosswidth_20260905 608
scripts/place_evo477.sh results/discovery/mac_crosswidth_20260905 608
```

The tournament compares against frozen SKY130 controls; using another Liberty
requires remapping those controls before interpreting its frontier.

## Matched-control qualification, 2026-09-05

`evo608_controls/` now contains 132 new mappings: 30 textbook trunks with the
exact same root/fusion plan, shared behavioral RTL, candidate, and ablation,
at W=8/16 under default/classic. No matched control dominates the 16-bit
candidate under either recipe. At 8 bits it is dominated by two default and
12 classic control forms. This is a useful 16-bit tradeoff, not a fastest-core
claim. The complete table includes controls, not just the winning rows.

Nine designs were then placed and globally routed: candidate and ablation for
each recipe plus the fastest and area-adjacent points from each pre-layout
control frontier, deduplicated. No selected routed control dominates evo608.
Default evo608 is 10,300 ps / 9,124 um2 against the neighboring Dadda/ripple
mux control's 11,250 ps / 9,238 um2 (8.44% faster and 1.23% smaller).
The fastest selected default control is 7,810 ps / 10,356 um2.

Classic evo608 is 7,680 ps / 14,418 um2. A Dadda/ripple mux control is
7,710 ps / 11,042 um2: only 30 ps slower and 23.42% smaller than the candidate.
Thus the candidate's classic speed advantage over its no-rule sibling does
not establish a practically compelling improvement over strong controls.
The fastest selected classic control is 6,750 ps / 15,104 um2. This targeted
subset is not a full routed control-frontier audit, and 30 ps is too small to
interpret as a robust margin without placement/extraction sensitivity checks.

Independent behavioral SAT miters prove candidate and no-rule sibling at W=4.
W=8 and W=16 both time out at a 20-second SAT budget. Timeouts are inconclusive,
not failures or proofs. Commands, miter sources, input hashes, and verdicts are
recorded in `evo608_formal/` and `scripts/prove_evo608.py`.

## Frozen held-out-width results

The prospectively written local protocol in `evo608_heldout/PROTOCOL.md` froze
the existing genome, widths 12/24, recipes default/classic, and all 30 matched
trunks. All 128 mapped forms are retained in the compact physical summary.
Candidate and sibling passed 200,000 samples plus fixed corner checks per width;
controls passed 1,000 samples plus fixed corners. The new u128 path checks all
75 output bits at W=24; this is still sampled verification, not a formal proof.

| width | recipe | delay change vs sibling | area change | dominating textbook controls |
|---:|---|---:|---:|---:|
| 12 | default | -10.21% | +0.43% | 1 |
| 12 | classic | -0.89% | -0.85% | 4 |
| 24 | default | -9.87% | +1.17% | 0 |
| 24 | classic | +6.75% | -4.80% | 1 |

The declared consistent-improvement criterion is not met. Default timing gains
transfer to both held-out widths at small area cost, and W=24/default remains
a matched-control Pareto point. Classic reverses timing at W=24. The retained
lead is therefore a recipe-conditional rule and a width-dependent tradeoff,
not a width-independent PPA improvement. The W=24/default routed follow-up is reported below. Formal proof, routing
at W=12 and with classic mapping, and stronger optimizer baselines remain needed.


### W=24 default global-route follow-up

The frozen 42-job follow-up completed with one job at a time and a two-core
Docker quota / two OpenROAD threads. At utilization 45, evo608 is 15,250 ps /
19,884 um2 versus its no-rule sibling at 17,350 ps / 19,654 um2: 12.10% lower
delay with 1.17% greater area. None of all 30 matched textbook controls
dominates it, although faster, larger controls exist.

At utilizations 35 and 50, paired delay changes are -16.88% and -5.61%, with
the same +1.17% area cost. None of the three frozen neighboring controls
dominates the candidate at either setting. Only utilization 45 tests the full
control set. These settings measure floorplan/placement/routing sensitivity;
they are not statistical replicates. Timing uses estimated global-route RC.
The area cost still fails the protocol's consistent-improvement criterion.

Compact results, frozen selection hashes, and tool/resource provenance are in
`results/discovery/mac_crosswidth_20260905/evo608_heldout/physical/route24_default_2cores/`.
The earlier `route24_default/` attempt is preserved: Docker disconnected after
14 completed jobs. The new run uses the identical 42-job selection and netlist
hashes and reproduces those 14 jobs' timing, area, and HPWL values. W=12 and
classic held-out routing remain unfinished.

### Composed structural-RTL correctness follow-up

A specialized checker now proves the frozen evo608 candidate and no-rule
sibling at W=4/8/12/16/24 (ten netlists). It reads emitted Verilog independently
of the generator, proves HA/FA and partial-product cell identities with SAT,
checks exact partial-product coverage and weighted compressor conservation,
and proves all four output roots for arbitrary two-row frontier inputs.
The actual-input bound excludes sum/rounding overflow. This composition
covers every primary-input assignment at each checked width.

This result is distinct from the earlier timed-out monolithic SAT attempts.
The trust boundary includes the Python parser/conservation checker, the
integer composition argument, and Yosys/SAT. It is not a proof-assistant
certificate and does not prove subsequent mapped netlists. Thirteen checker
regressions include deliberately corrupted wiring, arithmetic cells, high
outputs, and undefined cell values. Strict case-inequality comparisons and
undefined-aware SAT prevent X values from being accepted as a proof.

See `docs/MAC_COMPOSED_PROOF.md` and
`results/discovery/mac_crosswidth_20260905/evo608_composed_formal_strict/`
for the method, hashes, weighted frontiers, miters, commands, and solver logs.


### Mapped-netlist equivalence and reuse follow-up

All eight frozen W=12/24 candidate/sibling mappings (default and classic)
pass direct ABC combinational equivalence against their composed-proof
structural RTL. Yosys elaborates the exact SKY130 Liberty Boolean functions;
AIG port checks cover every input and output. W=24/default proof input hashes
match the mapped candidate and sibling used in the 42-job routing experiment.
This closes the Boolean synthesis-preservation gap for those eight netlists.
It does not prove routed-database connectivity or analog behavior.

The checker has negative regressions for a lost port and a corrupted high
mapped output. All jobs are serial, low priority, with one compute thread
and 20-second ABC/35-second process budgets. Evidence is in
`results/discovery/mac_crosswidth_20260905/evo608_mapped_equivalence/`.

A concrete LogikBench proposal is prepared in `integrations/logikbench/`:
candidate and no-rule benchmark folders, SiliconCompiler descriptors,
Verilog-2005 smoke tests, provenance/license records, and a registry patch
against a pinned upstream revision. Both actual descriptor-generated filelists
pass 4,106-vector simulations. Full upstream CLI/CI and PPA are untested;
no upstream PR, contact, or public release has been made.

The latest prior-art review includes AC-Refiner, CircuitsDNA, and RTLScout.
The scientific priority is now a strong matched baseline, not a broad novelty
claim. The candidate remains useful for evaluation even if that comparison
finds it dominated.
