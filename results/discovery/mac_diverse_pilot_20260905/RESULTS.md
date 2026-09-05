# Diversity-retaining MAC physical pilot

The 1,000-generation MAP-Elites pilot occupied 819 exact structural and semantic niches. A deterministic 128-member tournament was exhaustively verified at W=4, sampled at W=8/16, then synthesized in both preserved and flattened forms. Timing reports the worst path to any of the complete four-output contract's bits.

Of 256 candidate forms, 3 add points to the prior matched-control frontier:

| point | delay (ps) | area (um2) | interpretation |
|---|---:|---:|---|
| `mul16_mac_requant_evo477_flat` | 8730 | 8978.61 | one developmental rule; active search lead |
| `mul16_mac_requant_evo411_flat` | 8410 | 8986.12 | no developmental rules; ordinary combination |
| `mul16_mac_requant_evo164_flat` | 5860 | 9114.99 | no developmental rules; ordinary combination |

## Exact topology ablation

`evo477_flat` is 1.24% faster and 0.13% larger than the otherwise identical genome with its sole developmental rule removed (8840 ps / 8967.35 um2). The rule defers high-column compression at stage 3. This is a real pre-layout tradeoff, but its 110 ps separation is not silicon-admissible until it survives independent ABC recipes, placement/global routing, and width generalization.

`evo164` and `evo411` demonstrate that the original four-plan control matrix was not a full factorial exploration of output lowering. They are useful compiler configuration points, not developmental-search discoveries.

## Independent recipes and placement

At 16 bits the `evo477` rule wins pre-layout timing under all four tested ABC recipes: 1.24% under default, 8.18% under classic delay mapping, 0.34% under resynthesis, and 0.16% under area mapping. Under classic it is also 2.15% smaller. At 8 bits the rule loses or ties under every recipe.

Matched placement/global routing splits by recipe. Default mapping reverses the lead (12,040 versus 11,850 ps), while classic mapping strengthens it (7,390 versus 8,040 ps, with less area and wire). Accordingly `evo477` is a mapping/physical-interaction probe, not a robust superior implementation or a tapeout candidate.
