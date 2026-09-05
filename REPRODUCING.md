# Reproducing the archival artifact

The default path uses retained RTL and records. Physical replay and optimization
are optional and require external tools; they are not prerequisites for reading
or citing this artifact. The historical Rust package remains named
`tinytapeout2-search` to preserve generator commands.

## Lightweight replay

From the repository root:

```sh
python3 scripts/package_mac24_benchmark.py /tmp/mac24-benchmark.tar.gz
python3 benchmarks/mac24/smoke.py
```

The smoke check requires Icarus Verilog and verifies the original candidate and
no-rule sibling on 4,106 deterministic vectors each. It does not verify every
control or replace the recorded formal proofs.

To build the generator and replay the imported six-control graph without solving:

```sh
CARGO_BUILD_JOBS=2 nice -n 19 cargo run --locked --release --example mac_import_ufomac -- \
  results/discovery/mac_crosswidth_20260905/evo608_heldout/genome.json \
  results/discovery/mac_ufomac_challenge_20260905/optimized_graph.json \
  /tmp/mac-ufomac-replay
```

Use a fresh output directory. This checks the candidate byte for byte against
frozen RTL and verifies each imported form against the arithmetic reference.
An external replay should compare its emitted-file SHA-256 hashes to the
recorded source hashes. Compiler/tool identities remain in the evidence.

## Formal replay

Requires Python, Yosys, and `nice`:

```sh
python3 scripts/prove_mac_composed.py \
  --cells results/discovery/mac_crosswidth_20260905/evo608_heldout/cells.v \
  --output /tmp/mac-proof-replay \
  results/discovery/mac_ufomac_challenge_20260905/rtl/*.v
```

The composed checker runs one bounded solver at a time. Read
[docs/MAC_COMPOSED_PROOF.md](docs/MAC_COMPOSED_PROOF.md) for the exact trust
boundary. `mapped_proof.json` files record separately completed mapped-netlist
checks. Some redundant mapped netlists are omitted; regenerate them with their
retained `.ys` scripts before attempting those equivalence commands.

## Physical and optimizer replay

Read [the adapter](integrations/arithdas/README.md) and the campaign protocols.
External dependencies include the exact SKY130 TT Liberty and OpenROAD image
identities, Yosys/ABC, and (for optimization) NumPy, PuLP, and HiGHS. The three
standalone-cell characterization inputs and scripts are retained. No PDK or
container is bundled. A tool upgrade is a new comparison, not an exact replay.

Historical runners use fixed campaign paths and refuse to overwrite completed
outputs. Work in a separate checkout, preserve the archived evidence, and follow
the adapter's fresh-directory instructions. The old classic-neighbor supplement
requires its previously mapped control to be regenerated first. These are
research scripts, not a turnkey cross-platform EDA installation.

Use one measurement job at a time. Cargo builds should use at most two jobs,
Yosys/ABC/HiGHS one thread, and OpenROAD/container limits at most two cores.
The retained campaigns use explicit time limits; an inconclusive solve is not
a failed circuit. Do not rerun optimization merely to validate an archive.

## Evidence and provenance

SOURCE_MANIFEST.json records copied-file hashes and exact public-file adaptations.
Frozen optimizer/checker/tool hashes and raw logs remain unchanged, even where
they contain historical local directory paths. Those paths identify where the
run occurred; they are not instructions or external dependencies.

Some historical result pages refer to a larger private research history.
The quick-start and final W24 comparison above are the supported release paths;
links to omitted exploratory campaigns do not imply missing results in the
final comparison. REPORT.md is the current interpretation. Older protocols
and next-step notes are historical records, not an active research plan.
