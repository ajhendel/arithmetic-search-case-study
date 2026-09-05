# Unified MAC/requantization contract

For unsigned `W`-bit operands `a` and `b`, an unsigned `2W`-bit accumulator
`acc`, and fixed shift `S=W/2`:

1. `sum = a*b + acc`, represented in `2W+1` bits;
2. `rounded = sum + 2^(S-1)`;
3. `scaled = rounded >> S`;
4. `result = min(scaled, 2^W-1)`;
5. `saturated = scaled > 2^W-1`;
6. `status = saturated OR sum[2W] OR OR(sum[S-1:0])`.

One developed circuit exposes four semantic roots: the full accumulator sum,
the requantized result, the saturation flag, and status. These are parts of one
MAC datapath, not arbitrary functions joined after independent searches.

The first recorded run used a post-product accumulator and is retained as a
baseline. The active grammar also permits true compressor-level fusion:
accumulator bits enter the weighted partial-product columns before reduction,
and fusion itself is a genome allele. The genome jointly controls multiplier
development, accumulator placement/carry topology, rounding carry, saturation
and sticky reductions, clamp realization, predicate sharing, and routing-aware
structure. Correctness is exhaustive for `W=4`
(65,536 input triples). Search-time checks at `W=8` and `W=16` use deterministic
random and corner cases; scalable formal equivalence remains an admission gate.

This unsigned, fixed-shift contract is a research stepping stone. A production
ML/DSP contract would normally add signed accumulation, selectable shifts or
scales, zero points, defined rounding modes, and output clamping bounds. Those
features should be added as semantic genes only after complete matched controls
exist for the current contract.

## Initial fused-search admission status

The initial complete matched 16-bit pre-layout challenge has been run. All four
shortlisted candidates are dominated by the textbook control frontier after
SKY130 mapping and OpenSTA timing, in both preserved and flattened forms. They
are not tapeout candidates. See
`results/discovery/mac_fused_search_20260905/physical/` for the full matrix.

This is also a search-design result: structural depth, gate count, and the
routing proxy do not rank this workload accurately enough for admission.
Future evolution must incorporate mapped feedback—directly or through a
validated surrogate—before its population is scaled.

The first calibration rejects the existing routing score as an objective: it
is anticorrelated with measured pre-layout timing on the 120-control corpus.
Depth and gate count remain useful for preserved structures, but miss most of
the flattened physical frontier. The next engine must retain topology diversity
and periodically map candidates rather than discard them on routing score.

## Current evo608 admission status

The later diversity-retaining search found evo608. It survives the matched
W=16 pre-layout control frontiers for default/classic mapping and the W=24
held-out default frontier. A complete 30-control W=24/default global-route
comparison at utilization 45 also leaves it nondominated: 12.10% less delay
than its exact no-rule sibling at 1.17% greater area. This is conditional:
W=8, W=12, and W=24/classic have dominating matched controls. Composed weighted-conservation/SAT proofs now pass for the frozen candidate
and sibling at W=4/8/12/16/24. This specialized checker proves emitted
structural RTL. Direct ABC equivalence also passes for all eight W=12/24
candidate/sibling mappings under default/classic; routed-connectivity checks
and detailed-route signoff remain outstanding. See `docs/MAC_COMPOSED_PROOF.md` for the proof and trust boundary. These results do not establish production or tapeout readiness.
See `results/discovery/mac_crosswidth_20260905/RESULTS.md` and the portable
`benchmarks/mac24/` interface and smoke test.

## Frozen-genome replay and wide verification

`gen-mac-candidate <genome.json> <outdir> [widths...]` reloads a saved MAC genome
and emits its candidate, exact no-rule sibling, and all 30 textbook arithmetic
trunks with the same root/fusion plan. Default widths are 12 and 24; supported
widths are even values from 2 through 32. Physical ranking is a separate step.

MAC output bundles contain 3W+3 bits. W=24 and W=32 therefore require 75-bit
and 99-bit result storage. Replay uses the new u128 evaluator and arithmetic
reference; the previous u64 path was insufficient above W=20. Existing 8/16-bit
results are within the previous range. Explicit carry-out and rounding-boundary
checks now supplement random samples. Historical MAC `verify_mac` used only
exhaustive W=4 and deterministic random larger-width samples, despite earlier
prose referring to corner cases. The new fixed corner checks should not be
retroactively attributed to historical runs.
