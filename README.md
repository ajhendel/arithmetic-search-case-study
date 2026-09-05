# Arithmetic Search Case Study

A reusable MAC benchmark bundle and a documented process for generating,
checking, mapping, and comparing arithmetic circuits. The archive includes
38 Verilog designs with the same output contract, specialized correctness
tools, and retained SKY130 comparison evidence.

The motivating search experiment ended with a stronger control beating the
evolved candidate. The reusable output is the benchmark and evaluation process;
no novel optimizer or performance advantage is claimed.

## What you can pick up and use

| Artifact | Possible use | Scope |
|---|---|---|
| [MAC24 benchmark](benchmarks/mac24/README.md) | Compare a compressor-tree generator or synthesis recipe against designs with the same arithmetic interface. | 38 designs: candidate, ablation, 30 textbook controls, and six bounded UFO-MAC-derived controls. Includes a behavioral reference. |
| [Composed checker](scripts/prove_mac_composed.py) | Check partial-product coverage and weighted conservation, then SAT-prove the output tail. | Accepts the explicit generated cell dialect; not arbitrary Verilog. See the [trust boundary](docs/MAC_COMPOSED_PROOF.md). |
| [Mapped equivalence tooling](scripts/prove_mac_mapped.py) | Compare selected mapped netlists with the structural RTL through ABC equivalence. | Campaign-specific paths and matching library inputs require adaptation for a new project. |
| [External-graph importer](examples/mac_import_ufomac.rs) and [comparison runner](scripts/compare_mac_ufomac.py) | Attach a shared output contract to an imported compressor graph and evaluate it under the same mapping and routing recipes. | Uses an adapted, bounded ARITH-DAS replication of UFO-MAC; see the [adapter](integrations/arithdas/README.md). |
| [Rust generator](src/) | Explore another scheduling rule or generate related arithmetic structures. | Research code with a historical crate name and project-specific representations. |

The MAC contract uses unsigned 24-bit operands and a 48-bit accumulator, with
all 75 outputs live: the full sum, rounded and saturated output, and status.
The serial smoke test checks the original candidate and ablation on 4,106
deterministic vectors each. It does not test all 38 designs or replace formal
proof. Recorded proofs and their scope are linked from the benchmark guide.
The composed and mapped checkers also have retained test sources.

## Start with the benchmark

[Download v1.0.0 and its portable benchmark](https://github.com/ajhendel/arithmetic-search-case-study/releases/tag/v1.0.0),
or create a bundle from this checkout:

```sh
# Package retained Verilog and evidence; no Rust, synthesis, or PDK required.
python3 scripts/package_mac24_benchmark.py /tmp/mac24-benchmark.tar.gz

# Optional integration check of the candidate and ablation only.
# Requires Python, Icarus Verilog, and nice; runs serially.
python3 benchmarks/mac24/smoke.py
```

The [benchmark guide](benchmarks/mac24/README.md) describes extraction, ports,
and reference behavior. [REPRODUCING.md](REPRODUCING.md) separates lightweight
replay from optional physical evaluation. The recorded process is:

1. Generate or import designs with the same full-output contract.
2. Check arithmetic structure and prove the tail.
3. Map the designs and check mapped equivalence.
4. Route under common settings and compare area/delay estimates.
5. Retain tool identities, commands, solver limits, and outcomes.

The RTL and structural checker can be inspected without a PDK. The retained
physical results use SKY130 and specific recorded recipes; they do not establish
transfer to other libraries or flows. The main numerical comparison below uses
one default setting. Other retained experiments do not constitute a broad
cross-flow robustness study. Historical runners use fixed paths and external
EDA dependencies, so this is not a turnkey toolchain installation.

No experiment is launched automatically. Use at most two Cargo jobs, run solver
and physical jobs one at a time, and limit physical jobs to two cores.

## What the comparison found

A frozen evolved scheduling rule (`evo608`) initially survived a finite
SKY130 area/delay frontier. A bounded prior-art compressor optimizer subsequently
produced a control that was both faster and smaller in the same default flow.
The initial frontier claim did not survive the stronger comparison.

| Default flow, estimated global-route parasitics | Delay | Cell area |
|---|---:|---:|
| Evolved candidate `evo608` | 15.25 ns | 19,884 µm² |
| Bounded UFO-MAC-derived BrentKung/mux control | 14.01 ns | 19,843 µm² |

The comparator is **8.13% faster and 0.21% smaller**. The small area difference
and a single physical-flow setting do not establish statistical robustness.
The optimizer stopped after 90 seconds with a 62.35% optimality gap. This is
ARITH-DAS's UFO-MAC compressor replication with an adapted exact-output contract,
not a reproduction of either complete method's best published performance.

Read the [short report](REPORT.md), [full comparison](results/discovery/mac_ufomac_challenge_20260905/RESULTS.md),
and [proof scope](docs/MAC_COMPOSED_PROOF.md).

![Measured frontier](results/discovery/mac_ufomac_challenge_20260905/frontier.png)

## Status, citation, and scope

This line of investigation is closed. This repository is an archival research
artifact, with no scheduled development, support commitment, or further compute
campaign. Reuse and independent follow-up are welcome under the licenses.

It is not a tapeout-ready chip, a fabricated-silicon result, a claim of novel
compressor scheduling, or a state-of-the-art arithmetic design. Formal correctness
of selected circuits does not establish performance superiority or novelty.

[![DOI](https://zenodo.org/badge/DOI/10.5281/zenodo.22341676.svg)](https://doi.org/10.5281/zenodo.22341676)

**Cite the exact version used:** Hendel, A. (2026). *Arithmetic Search Case Study:
Verified MAC Benchmarks and a Negative Frontier Result* (Version 1.0.0)
[Computer software]. Zenodo. https://doi.org/10.5281/zenodo.22341677

- Version 1.0.0 DOI: [10.5281/zenodo.22341677](https://doi.org/10.5281/zenodo.22341677).
- Concept DOI (all versions): [10.5281/zenodo.22341676](https://doi.org/10.5281/zenodo.22341676).
- Author: Andrew Hendel, [ORCID 0009-0000-9877-3623](https://orcid.org/0009-0000-9877-3623).
- [Download the release and portable benchmark](https://github.com/ajhendel/arithmetic-search-case-study/releases/tag/v1.0.0).

[CITATION.cff](CITATION.cff) supplies machine-readable citation metadata.
Zenodo preserves the tagged source snapshot; GitHub also provides the packaged
benchmark and report. [ARCHIVE.json](ARCHIVE.json) records the archived commit
and identifiers. Citation links were added to the main branch after deposit;
the archived tag and files remain fixed. This work has not been peer reviewed.
See [RELEASE_NOTES.md](RELEASE_NOTES.md).

The public-facing project name is `arithmetic-search-case-study`. Historical
paths and the Rust package identifier `tinytapeout2-search` remain unchanged for
replay. [SOURCE_MANIFEST.json](SOURCE_MANIFEST.json) identifies the private
research source revision and every copied or adapted file; access to its full
history is not needed for the documented quick-start workflows.

Code is Apache-2.0 except the explicitly marked MIT-licensed ARITH-DAS source.
See [LICENSE](LICENSE) and [NOTICE.md](NOTICE.md). Development and analysis used
AI coding assistance; no independent human review or additional authorship is
implied.
