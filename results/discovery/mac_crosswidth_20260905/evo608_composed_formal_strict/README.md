# evo608 composed structural-RTL proofs

All ten frozen candidate/sibling netlists passed the strict composed proof.
See `docs/MAC_COMPOSED_PROOF.md` for the conservation argument and trust boundary.

| Width | Design | Status | Compressors | Frontier bits | Yosys tail time (s) |
|---:|---|---|---:|---:|---:|
| 4 | candidate | proved | 12 | 16 | 0.05 |
| 4 | sibling | proved | 12 | 16 | 0.05 |
| 8 | candidate | proved | 56 | 32 | 0.10 |
| 8 | sibling | proved | 56 | 32 | 0.10 |
| 12 | candidate | proved | 132 | 48 | 0.15 |
| 12 | sibling | proved | 132 | 48 | 0.20 |
| 16 | candidate | proved | 245 | 65 | 0.25 |
| 16 | sibling | proved | 240 | 64 | 0.24 |
| 24 | candidate | proved | 560 | 97 | 0.37 |
| 24 | sibling | proved | 552 | 96 | 0.37 |

All 13 checker tests passed, including required rejection/counterexamples for
malformed wiring, corrupted cell equations and high outputs, and X-valued cell
outputs. The initial development runs remain local; this directory records the
final checker using `opt -keepdc`, strict case inequality, defined primary
inputs, undefined-aware SAT, and `check -assert`.

Each proof ran serially at low priority with one compute thread, a 20-second
SAT budget and 35-second process budget. No timeout occurred. The recorded
times are Yosys log totals for each tail invocation, not benchmark speed claims.

This proves the supplied emitted structural RTL for every input triple at
each listed width. It does not prove the mapped netlists, all 30 controls,
other genomes/widths, or physical behavior. Historical monolithic SAT timeouts
remain inconclusive; these are newly constructed composed proofs.
