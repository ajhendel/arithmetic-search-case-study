# W=24 default mapping: full controls and floorplan sensitivity

| utilization | candidate ps / um2 | no-rule ps / um2 | delay change | area change | completed controls | dominating controls |
|---:|---|---|---:|---:|---:|---:|
| 35 | incomplete | incomplete | — | — | — | — |
| 45 | 15250 / 19884 | 17350 / 19654 | -12.10% | +1.17% | 12 | 0 |
| 50 | incomplete | incomplete | — | — | — | — |

Completed 14/42 planned jobs. The full 30-control challenge applies only at utilization 45; the other two settings use frozen pre-layout neighbors. All timing uses estimated global-route parasitics. The three settings are sensitivity cases, not statistical replicates.

Some jobs failed; see summary.tsv. No complete-frontier claim is warranted for incomplete settings.

Reproduce in a fresh checkout/output location: `python3 scripts/route_mac_heldout.py`, after the held-out replay and mapping commands. Selection, hashes, and protocol were recorded before routing.
