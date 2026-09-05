# Unsigned MAC24 topology benchmark

This portable benchmark contains 38 designs with the same arithmetic interface:
evo608, its exact no-rule sibling, 30 textbook controls with the same
output/fusion plan, and six bounded UFO-MAC-derived controls. It also includes
an independent behavioral reference and a candidate/ablation smoke test.
The exported bundle needs
no Rust build, Docker, PDK, or access to the research repository for RTL use.
It is an evaluation artifact. A composed arithmetic-conservation/SAT proof
now covers candidate and sibling structural RTL at W=24. Direct ABC CEC
also proves the frozen default/classic SKY130 mappings equivalent to that
RTL. Physical signoff remains a separate obligation.

## Obtain and check the bundle

Download `mac24-benchmark-v1.0.0.tar.gz` from the
[public release](https://github.com/ajhendel/arithmetic-search-case-study/releases/tag/v1.0.0),
or create a fresh bundle from the root of the public source checkout:

```sh
python3 scripts/package_mac24_benchmark.py /tmp/mac24-benchmark.tar.gz
```

For a freshly generated bundle, extract into a fresh directory:

```sh
mkdir /tmp/mac24-reuse
tar -xzf /tmp/mac24-benchmark.tar.gz -C /tmp/mac24-reuse
cd /tmp/mac24-reuse/mac24
python3 smoke.py
```

For the downloaded release asset, substitute its actual path for the archive
path in the extraction command. Both archives contain a `mac24/` directory.
The packager's historical default filename is avoided by passing an explicit
output path above.

The smoke test requires Python 3, Icarus Verilog (`iverilog`, `vvp`), and `nice`.
It runs serially at low priority with compilation/simulation timeouts, checking
candidate and sibling against the behavioral RTL on 4,106 deterministic vectors
each, including carry, rounding, saturation, and unsaturated cases. It does not
check all 38 designs and is not exhaustive proof. From the public source root,
the equivalent command is `python3 benchmarks/mac24/smoke.py`.

## Interface

`a` and `b` are unsigned 24-bit operands; `aux` is the unsigned 48-bit
accumulator. There is no clock, reset, handshake, or internal state. All
outputs are combinational:

| Output bits | Meaning |
|---|---|
| `y[48:0]` | Exact 49-bit `a*b + aux` |
| `y[72:49]` | `min((sum + 2048) >> 12, 16777215)` |
| `y[73]` | Scaled value exceeds 16777215 |
| `y[74]` | Saturation OR sum carry-out OR any nonzero `sum[11:0]` |

In the extracted bundle, instantiate `mul24_mac_replay_candidate` from `rtl/`
together with `rtl/cells.v`.
Use `mul24_mac_replay_norules` for the exact ablation. The behavioral reference
is `reference.v`. Rounding is add-half then shift, not round-to-even. This
contract has no signed arithmetic, variable shift, zero point, or stateful MAC.

## Fair comparisons and evidence

For flattened synthesis, remove `keep_hierarchy` from the arithmetic cells
before flattening. The original Yosys sequence is:

```text
read_verilog rtl/cells.v rtl/mul24_mac_replay_candidate.v
attrmap -modattr -remove keep_hierarchy=1
hierarchy -top mul24_mac_replay_candidate
proc; flatten; opt; techmap; opt
```

Map each candidate/control against the same Liberty, recipe, loads, and
physical flow. The bundle includes all 30 controls because comparing only
against the no-rule sibling would overstate the practical result. Original
SKY130 default global-route estimates at utilization 45: candidate 15,250 ps /
19,884 um2; sibling 17,350 ps / 19,654 um2. None of the original 30 controls
dominated it. The subsequent bounded
prior-art comparison does: `mul24_mac_ufomac_brentkung_mux` reaches **14,010 ps /
19,843 um2**, 8.13% faster and 0.21% smaller. Evo608 therefore leaves the expanded
default frontier. Classic synthesis at W=24 also loses its frontier position.
See `evidence/` for complete tables;
the pre-layout table includes both W=12 and W=24. These are estimates, not
signoff or silicon measurements. New tool/library runs may change rankings.

`MANIFEST.json` hashes the exported inputs and evidence. The
[public source repository](https://github.com/ajhendel/arithmetic-search-case-study)
contains the physical reproduction scripts and selected research evidence.
The original exploratory Git history remains private and is not required for
the documented benchmark workflow. The bundle
does not include a PDK. The local RTL follows the Apache-2.0 notice;
the upstream optimizer
MIT license and attribution are also included under `licenses/`.

## Reproduce the composed proof

Requires Yosys in addition to Python and `nice`. From the extracted bundle:

```sh
python3 prove_mac_composed.py --cells rtl/cells.v --output /tmp/mac24-proof \
  rtl/mul24_mac_replay_candidate.v rtl/mul24_mac_replay_norules.v
```

Use a fresh output directory. This runs one low-priority solver at a time,
with a 20-second SAT / 35-second wall budget per proof. `PROOF.md` explains
weighted conservation, arbitrary-frontier SAT, the trusted checker boundary,
and why the result covers all primary inputs. `proof/` contains recorded
commands, miters, solver logs, and the broader five-width proof summary.
It proves these emitted structural netlists; it does not prove mapped RTL,
all controls, arbitrary widths/genomes, or silicon behavior.

Recorded mapped-netlist equivalence snapshots and commands are included in
`mapped_proof/`. Supply the matching Liberty (identified in its summary) to
replay those commands; library data is not bundled. The standalone bundle contains the circuits and selected proof records.

## Stronger prior-art-derived controls

The six `mul24_mac_ufomac_*` netlists have exactly the same interface and use
`rtl/cells.v`. `evidence/ufomac/` records the bounded optimizer comparison and
its limits; `proof/ufomac/` contains composed proof evidence for all six.
All twelve mapped forms passed ABC equivalence in the research campaign.
The quick smoke script still targets the original candidate/ablation pair.

These controls come from ARITH-DAS's replication of UFO-MAC compressor
optimization, followed by the shared local CPA/output roots. The interconnection
solve stopped at 90 seconds with a 62.35% gap. They are useful measured baselines,
not a reproduction of the complete original optimizer or a novelty claim.
