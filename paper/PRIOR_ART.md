# Prior-art assessment and reuse plan

## Direct comparison completed after this review

A bounded run of the official ARITH-DAS repository's UFO-MAC compressor
replication generated a control that dominates evo608 under default mapping
and routing: **8.13% lower routed delay and 0.21% less area**. The run used the
full 48-bit accumulator, shared output roots, and the same physical flow. All
six emitted controls passed structural proofs and all twelve mapped forms
passed equivalence. The interconnection solve stopped at 90 seconds with a
62.35% gap; it is not a reproduction of the complete original UFO-MAC flow or
trained ARITH-DAS. [Results](../results/discovery/mac_ufomac_challenge_20260905/RESULTS.md).
The prior frontier result no longer supports a competitive-architecture claim.
The reviewed history below predates this measurement.

Reviewed 2026-09-05. Targeted review of primary papers, author-hosted papers,
official proceedings, and project repositories. No experiments, tests, synthesis,
or measurements were run for this review. This is a research positioning review,
not an exhaustive literature or patent search. Novelty is not established.

Follow-up since the literature review: composed structural-RTL proofs now
pass for evo608 and sibling at W=4/8/12/16/24. See
[proof scope](../docs/MAC_COMPOSED_PROOF.md). This advances the first proposed
verification task; it does not change the prior-art or novelty assessment.

## Finding

`evo608` is a potentially useful measured implementation of an established class
of arithmetic optimizations. Its existence on the tested 24-bit/default
SKY130 global-route area-delay frontier is useful evidence, but does not establish
a new arithmetic principle or superiority over current arithmetic optimizers.

The strongest current manuscript direction is an openly reproducible study of
how a compact, width-generative compressor rule transfers or fails to transfer
between widths, synthesis recipes, and physical estimation stages. Even this
positioning needs stronger comparisons: flow sensitivity and width-dependent
arithmetic optimization already have prior art. A research artifact can still
be useful without a claim to invent the underlying technique.

## What the local candidate actually changes

The frozen [genome](../results/discovery/mac_crosswidth_20260905/evo608_heldout/genome.json)
uses Dadda compression and positional operand selection by default. Its one
override uses `FullNoHa` with `Earliest` selection for heights 4–10, normalized
columns 4–13, and stages 2–5. `FullNoHa` applies full adders and passes a remaining
pair without a half adder. The implementation explicitly describes `Earliest`
as inspired by three-greedy scheduling and sorts by its internal arrival model
in [src/evo.rs](../src/evo.rs). This is not direct physical-arrival feedback.

The rest of the frozen design includes a Sklansky final adder, mux-based full
adders, accumulator fusion, prefix rounding carry, balanced reductions, and
shared saturation. The no-rule sibling retains those choices. Consequently,
the paired result attributes the change to the scheduling override within this
configuration; it does not demonstrate that accumulator fusion or rounding
logic was newly discovered.

The measured contract exposes the full unsigned MAC sum, rounded/scaled/clamped
result, saturation, and status. Keeping all those outputs live matters when
comparing against tools that generate only a multiplier or a truncated result.
See the [contract](../docs/MAC_REQUANT_CONTRACT.md) and
[result history](../results/discovery/mac_crosswidth_20260905/RESULTS.md).

## Closest primary prior art

| Work | Established overlap | Consequence for this project |
|---|---|---|
| Oklobdzija, Villeger, and Liu, *A Method for Speed Optimized Partial Product Reduction and Generation of Fast Parallel Multipliers Using an Algorithmic Approach*, IEEE TC 1996. [Author-hosted paper](https://www.ece.ucdavis.edu/~vojin/CLASSES/EEC280/Web-page/papers/Arithmetic/TDM-Multipl-IEEE-TC-96.pdf) | Algorithmic partial-product reduction considers unequal signal and cell-pin delays and constructs multipliers across sizes and technologies. | Arrival-aware operand assignment is old. Compare the bounded override with a faithful three-greedy implementation, not only positional textbook schedules. |
| Verma and Ienne, *Automatic Synthesis of Compressor Trees: Reevaluating Large Counters*, DATE 2007. [Paper](https://www.cecs.uci.edu/~papers/date07/PAPERS/2007/DATE07/PDFFILES/03.7_1.PDF) | Explicitly describes selecting least-arrival inputs in three-greedy scheduling and studies compressor selection, counter implementation, and tree optimization with ILP. | Choosing compressors and changing their interconnections is a mature search space. A new parameter tuple is not by itself an architectural invention. |
| Brunie et al., *Arithmetic Core Generation Using Bit Heaps*, FPL 2013. [Author-hosted paper](https://perso.citi-lab.fr/fdedinec/recherche/publis/2013-FPL-BitHeap.pdf) | Represents unevaluated sums of weighted bits and optimizes whole arithmetic expressions in FloPoCo. | Combining partial products, accumulator bits, and constants in one weighted structure is established. Its FPGA target differs from this SKY130 ASIC study. |
| Xiao, Qian, and Liu, *GOMIL: Global Optimization of Multiplier by Integer Linear Programming*, DATE 2021. [Proceedings](https://past.date-conference.com/proceedings-archive/2021/html/1386.html), [official code](https://github.com/SJTU-ECTL/GOMIL) | Joint optimization of compressor tree and final prefix adder, with AND and Booth partial-product generation. | Thirty textbook trunks do not cover the stronger space of optimized tree/adder combinations. |
| Zuo et al., *UFO-MAC*, ICCAD 2024. [Author-hosted paper](https://personal.hkust-gz.edu.cn/yuzhema/papers/C39-ICCAD2024-UFO-MAC.pdf) | ILP refines compressor stage assignment and interconnection; final-adder optimization accounts for nonuniform arrival profiles; supports fused MACs. | Especially close conventional optimizer. It challenges any claim that region-specific scheduling, arrival profiles, or fused-MAC optimization is new. |
| Zuo et al., *RL-MUL 2.0*. [Author preprint](https://arxiv.org/abs/2404.00639), [journal DOI](https://doi.org/10.1145/3711850) | Pareto-directed RL over compressor representations, multiple widths, and fused-MAC support. | Learning-based MAC search and Pareto evaluation predate this study. Different numerical contracts must be adapted before comparing. |
| Lai et al., *Scalable and Effective Arithmetic Tree Generation for Adder and Multiplier Designs*, NeurIPS 2024. [Proceedings](https://proceedings.neurips.cc/paper_files/paper/2024/hash/fb23cf87a9e04d7677b73c47acd060ef-Abstract-Conference.html), [ArithTreeRL code](https://github.com/laiyao1/ArithTreeRL) | Arithmetic-tree generation as RL games; explores speed/area tradeoffs in adders and multipliers. | Generative arithmetic topology search is established and has stronger comparators than Wallace/Dadda alone. |
| Wang et al., *Computing Circuits Optimization via Model-Based Circuit Genetic Evolution*, ICLR 2025 (MUTE). [Proceedings](https://proceedings.iclr.cc/paper_files/paper/2025/hash/d067d16e3e5fe8fa8a3e62909907659a-Abstract-Conference.html) | Grid-based circuit genomes, mutation, column-range crossover, diverse exploration, and objective-based evaluation for multipliers/adders/MACs. | Direct evolutionary prior art. Avoid claims of the first evolved MAC, first circuit genome, or first physical tournament. |
| Xia et al., *High-Performance Arithmetic Circuit Optimization via Differentiable Architecture Search*, NeurIPS 2025 (ARITH-DAS). [Paper](https://papers.nips.cc/paper_files/paper/2025/file/2026b8cac62265e48c630c4449522550-Paper-Conference.pdf), [official code](https://github.com/MIRALab-USTC/Arith-DAS) | Combines evolutionary compressor allocation with learned interconnect selection guided by post-synthesis metrics; evaluates multipliers/MACs. Official code includes MAC generation and Yosys/OpenROAD integration. | Highest-priority modern baseline: it overlaps topology diversity, compressor connectivity, fused arithmetic, and mapped evaluation. Its published percentages cannot be compared directly with this project's SKY130 numbers. |
| Xue et al., *DOMAC: Differentiable Optimization for High-Speed Multipliers and Multiply-Accumulators*, 2025. [Author preprint](https://arxiv.org/abs/2503.23943) | Process-aware compressor-tree optimization using differentiable timing and area models for multiplier/MAC design. | Physical/process-aware arithmetic search is already an explicit research topic. |
| Wanna et al., *Multiplier Optimization via E-Graph Rewriting*, 2023 (OptiMult). [Author preprint](https://arxiv.org/abs/2312.06004) | Searches alternative multiplier representations using rewriting and extracts optimized circuits. | Rule-based multiplier transformation search is established. Compare a rewriting approach before claiming a superior representation. |
| Coward et al., *Automatic Datapath Optimization Using E-Graphs*, 2022. [Author preprint](https://arxiv.org/abs/2204.11478) | Reports that the best architecture for an arithmetic expression depends on operand width. | Width-dependent rankings are not a new observation by themselves. |

MAP-Elites itself is an existing diversity-preserving search algorithm, credited
to [Mouret and Clune (2015)](https://arxiv.org/abs/1504.04909). Its application here
could merit study, but needs budget-matched ablations against ordinary evolution,
random sampling, and direct mapped selection before claiming a search-method
advance.

Likewise, synthesis/layout interaction is longstanding: IBM's
[transformational placement and synthesis work (DATE 2000)](https://research.ibm.com/publications/transformational-placement-and-synthesis)
combines Boolean, electrical, and physical optimization. That supports the
general motivation, not a claim that this paper previously reported evo608's
specific recipe-dependent reversal. This review did not establish whether an
identical frozen multi-width/recipe/global-route audit already exists.

MAC output scaling is also an established application pattern. For example,
[Gemmini's official documentation](https://github.com/ucb-bar/gemmini/blob/master/README.md?plain=1)
describes accumulator-to-input scaling and activation. It is not the same
four-output unsigned fixed-shift contract and should not be described as a
drop-in compatible consumer.

## Additional 2026 prior art checked against the user-supplied list

- **AC-Refiner (ASP-DAC 2026; preprint July 2025)** uses conditional diffusion
  and fine-tuning on explored designs to focus search near an arithmetic
  Pareto frontier. This adds direct prior art for learned frontier-focused
  arithmetic search. [Paper](https://arxiv.org/abs/2507.02598),
  [conference presentation](https://www.aspdac.com/aspdac2026/archive/pdf/2F-3.pdf).
- **CircuitsDNA (September 1, 2026 preprint)** evolves accuracy-configurable
  approximate multipliers and combines bounded verification with adaptive
  mutation. Its reported 28 nm experiment uses Design Compiler synthesis and
  PrimeTime PX power estimation. The checked paper does not substantiate the
  supplied table's "post-layout" wording. Its error-tolerant, multi-mode
  contract also differs from evo608's exact unsigned MAC contract.
  [Paper](https://arxiv.org/html/2609.01735v1).
- **RTLScout (June 2026 preprint)** combines agentic RTL/gate rewriting,
  architecture sweeps, and Yosys/OpenROAD feedback. It is additional evidence
  that the PPA feedback loop itself is established. Its floating-point
  multiplier results cannot be numerically compared with our fixed unsigned
  contract without adapting the workload and matching the technology/flow.
  [Paper](https://arxiv.org/abs/2606.06530),
  [official repository](https://github.com/huawei-csl/rtlscout).

These works reinforce the existing verdict; they do not establish that the
exact emitted evo608 graph already appeared elsewhere. The older scheduling
works cover the underlying idea, but "the exact idea" should not imply
identity of the bounded rule, pin timing model, or resulting graph. Conversely,
not finding an identical graph is insufficient evidence of a research advance.

Practical consequence: retain the tested circuit as a reusable benchmark and
candidate, prioritize direct strong-baseline comparisons, and defer a novelty
or state-of-the-art manuscript claim. Proof completion establishes correctness;
it does not establish novelty. No new expensive training run is justified
merely to compete with the size of these prior systems.

## Claims the current evidence can and cannot support

Potentially distinctive **artifact**, subject to further comparison:

- A compact bounded scheduling rule, replayed at held-out widths without changing
  the genome, with the exact no-rule sibling and identical output semantics.
- Complete retained outcomes for 30 matched textbook controls in the primary
  24-bit/default global-route setting, plus explicitly limited neighboring-control
  checks under other floorplans.
- Positive and negative transfer examples in one reproducible package, useful
  for testing search proxies and flow robustness.

Not supported: a newly invented MAC operation, novel earliest-arrival scheduling,
first evolutionary arithmetic search, global Pareto optimality, superiority to
ARITH-DAS/MUTE/UFO-MAC, production readiness, or a generally faster architecture.
We also have not demonstrated that the bounded rule generates a graph absent
from prior optimizers' spaces. Distinct source code or genome syntax does not
answer that structural question.

The local 12.10% routed-delay reduction is against the no-rule sibling with
1.17% more area. Nondomination by the 30 matched controls is a separate result.
Neither number means 12.10% faster than the best available competing design.
The working manuscript's emphasis on conditional transfer is appropriate; a
major new-architecture claim would be premature.

## Next comparisons, in priority order

1. Finish larger-width correctness using a scalable decomposed proof or a
   independently checked arithmetic invariant, rather than only increasing a
   monolithic SAT timeout. Preserve inconclusive outcomes.
2. Add faithful arrival-aware three-greedy and stronger tree/final-adder controls.
   Identify whether the existing 30 controls already cover the relevant policies;
   do not infer coverage from a Dadda or Wallace name.
3. Import an ARITH-DAS or UFO-MAC generated trunk under the same full-output
   contract, library, constraints, recipes, and physical stage. Compare the
   wrapper costs identically. Use documented published RTL where available
   before undertaking an expensive retraining run.
4. Test whether the rule remains useful after final-adder/root-plan retuning and
   compare small local changes around its bounds. This separates a robust useful
   region from an isolated heuristic outcome.
5. For a search-method paper, predeclare a small budget of independent seeds
   and compare MAP-Elites, ordinary evolution, and random search with equal
   numbers of physical evaluations. Report all selected cases and failures.

These are proposed experiments, not completed work. Only the main lane should
run tests or measurements; retain one job at a time and the two-core local cap.

## Reuse and manuscript path

First package a small standalone artifact from a pinned source revision:
contract, frozen genome, parameterized generator or emitted RTL, behavioral
reference, verification status, exact commands/tool identities, compact outcome
tables, citation metadata, and license/provenance. Keep discovery history in the
original repository and cross-link it. This package can help others reproduce
the finding before a manuscript is accepted. It should advertise an experimental
arithmetic candidate with measured constraints.

Potential external destinations are recommendations, not contacts or promises
of acceptance:

- [LogikBench](https://github.com/zeroasiccorp/logikbench) is a plausible benchmark
  integration target: it documents self-contained RTL, provenance, parameterized
  designs, and synthesis/place-and-route flows. It already has a
  [requantization benchmark](https://github.com/zeroasiccorp/logikbench/blob/main/logikbench/benchmarks/arithmetic/requant/README.md)
  with a different signed, runtime-scale/shift contract. A candidate/reference
  pair with this project's exact semantics could be complementary.
- [ARITH-DAS](https://github.com/MIRALab-USTC/Arith-DAS) or
  [ArithTreeRL](https://github.com/laiyao1/ArithTreeRL) are plausible future homes
  for an interoperability adapter or evaluation case after a clean standalone
  artifact exists. Upstream integration needs maintainer interest and a concrete
  compatible contribution.
- [FloPoCo](https://flopoco.org/flopoco_manual.pdf) documents extension through
  `CompressionStrategy`. It is a relevant future strategy-port target, but FPGA
  results would be new experiments; an ASIC improvement cannot be assumed to
  carry over.

My recommendation is to develop the reusable artifact and restrained case-study
manuscript together. Decide on a publication venue after the stronger comparator
and correctness work clarify whether the result is chiefly a benchmark artifact,
a robustness study, or a competitive arithmetic optimization. No second
repository, submission, upstream message, or public release was created by this
review.
