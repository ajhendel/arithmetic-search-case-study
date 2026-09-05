# W=24 default mapping: full controls and floorplan sensitivity

| utilization | candidate ps / um2 | no-rule ps / um2 | delay change | area change | completed controls | dominating controls |
|---:|---|---|---:|---:|---:|---:|
| 35 | 14180 / 19884 | 17060 / 19654 | -16.88% | +1.17% | 3 | 0 |
| 45 | 15250 / 19884 | 17350 / 19654 | -12.10% | +1.17% | 30 | 0 |
| 50 | 16150 / 19884 | 17110 / 19654 | -5.61% | +1.17% | 3 | 0 |

Completed 42/42 planned jobs. The full 30-control challenge applies only at utilization 45; the other two settings use frozen pre-layout neighbors. All timing uses estimated global-route parasitics. The three settings are sensitivity cases, not statistical replicates.

Reproduce in a fresh checkout/output location: `python3 scripts/route_mac_heldout.py --output <fresh-directory> --cores 2`, after the held-out replay and mapping commands. Selection, hashes, and protocol were recorded before routing.
