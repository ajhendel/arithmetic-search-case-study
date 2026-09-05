# Frozen W=24 arrival-aware control challenge

| Stage | Recipe | Candidate ps / um2 | Sibling ps / um2 | Delay change | Area change | Dominating new baselines |
|---|---|---|---|---:|---:|---:|
| mapped | default | 10140 / 19884.07 | 11250 / 19653.85 | -9.87% | +1.17% | 0 |
| mapped | classic | 6640 / 30415.42 | 6220 / 31949.39 | +6.75% | -4.80% | 0 |
| global-route | default | 15250 / 19884.00 | 17350 / 19654.00 | -12.10% | +1.17% | 0 |
| global-route | classic | 11600 / 30415.00 | 11010 / 31949.00 | +5.36% | -4.80% | 0 |

All 30 mappings and all 30 routes are retained. New baselines comprise
12 rule-free arrival-aware variants plus behavioral RTL, all with the exact
75-bit output contract. This corpus supplements the earlier 30 positional
textbook trunks; it is not the full space of modern arithmetic optimizers.

Arrival estimates use the existing generator's coarse depth model. These
are not a faithful pin-delay-aware three-greedy implementation, ARITH-DAS,
or UFO-MAC. Any forced completion follows the existing generator. The
candidate/genome are unchanged. No parameter was selected using this run's
physical results; all planned competitors were routed at utilization 45.

The behavioral baseline uses the same fixed lowering/mapping script as all
other designs. It is not a sweep of the best available behavioral synthesis
strategies. Routing uses estimated global-route parasitics; no signoff, power,
silicon, or statistical-replication claim follows.

Every structural form passed composed all-input proof; the behavioral form
is the arithmetic reference. Every mapped form passed direct ABC equivalence
to its input RTL. See the frozen protocol, proof records, mapping and routing
tables, logs, and source hashes. All computation ran serially, with one
Yosys/ABC thread and at most two OpenROAD/container cores.

Reproduce with `python3 scripts/compare_mac_arrival.py --output <fresh-relative-directory>`.
