# Composed correctness proof for frozen fused MAC netlists

`scripts/prove_mac_composed.py` checks the emitted Verilog independently of
the Rust generator. It accepts a restricted explicit-cell Verilog dialect;
it does not implement general Verilog elaboration. Unrecognized constructs
and unsupported compressor boundaries are rejected. The intended initial
targets are the frozen evo608 candidate and exact no-rule sibling.

## Proof obligations

1. **Cell identities.** Yosys SAT proves, from the actual supplied `cells.v`,
   that a partial-product cell equals `a & b` and that every supported HA/FA
   variant satisfies `sum + 2*carry = sum(inputs)`, for all Boolean inputs.
   A successful run requires these lemmas; their names alone are not trusted.
2. **Partial-product coverage.** The checker requires exactly one AND for
   every ordered pair `(a[i], b[j])`. Its weight is `2^(i+j)`. Every accumulator
   bit starts as a live term with weight `2^i`. Consequently the initial live
   sum is exactly `a*b + aux`.
3. **Weighted conservation.** Each HA/FA consumes distinct currently live
   terms at the same weight, removes them, and creates a sum at that weight
   and a carry at twice that weight. This preserves the integer sum by the
   proved cell identities. Reuse, different-column inputs, missing products,
   duplicate drivers, or out-of-range boundary weights are rejected. The
   remaining frontier must contain at most two terms per column. The tail
   cannot bypass it to access consumed bits or original operands, and no
   frontier term may be silently dropped.
4. **All-output tail equivalence.** Replace each remaining frontier bit with
   an independent symbolic input and copy the parsed suffix cells and all
   output connections into a SAT miter. Pack the frontier's known weights
   into two words and add them in the independent behavioral reference.
   Prove the actual full sum, rounded/clamped result, saturation, and status
   equal that reference for **every** assignment to the frontier. This is
   stronger than proving only the reachable frontier states. No sampled
   equality or assumed internal matching signals are used. Undefined-value
   modeling is enabled, primary inputs must be defined, and a structural
   `check -assert` precedes SAT; strict case-inequality comparisons and
   `opt -keepdc` preserve undefined-value behavior; X-valued outputs cannot stand in for correctness.

The tail reference uses `2W+1`-bit modular addition and rounding, matching the
emitted datapath. For the real input domain the checker also establishes the
integer bound

```text
(2^W - 1)^2 + (2^(2W) - 1) + 2^(W/2 - 1) < 2^(2W+1).
```

Thus neither the real MAC sum nor its rounding overflows that width. Composing
the conserved frontier with the universally proved tail establishes the exact
unsigned contract for every primary-input triple, at each checked width.

## Scope and trust boundary

This is a specialized arithmetic-conservation proof composed with SAT, not a
single end-to-end behavioral SAT result. It trusts the Python parser/checker,
the stated integer argument, and Yosys/SAT. The emitted certificate includes
the weighted frontier, source/checker/cell hashes, tool version, generated
miters, commands, and solver logs, so the composition is reviewable. It is not
a proof-assistant-checked theorem or an independently checked SAT certificate.

It proves the supplied structural RTL. It does not establish equivalence of
subsequent mapped netlists, analog timing, power, PDK models, or silicon. It
does not prove all genome mutations or untested widths. Previous monolithic
SAT timeouts remain inconclusive historical results; this method is a new
proof route rather than a relabeling of those runs.

## Resource limits and regression checks

All commands run serially in the main lane. Each low-priority Yosys invocation
uses one thread, a default 20-second SAT budget, and a 35-second whole-process
wall budget. Requested SAT budgets are restricted to 1–60 seconds. Existing
output directories are never overwritten. No broad synthesis/search campaign
is launched by this checker.

```sh
python3 -m unittest discover -s scripts -p test_prove_mac_composed.py
python3 scripts/prove_mac_composed.py \
  --cells results/discovery/mac_crosswidth_20260905/evo608_heldout/cells.v \
  --output /tmp/mac24-composed-proof \
  results/discovery/mac_crosswidth_20260905/evo608_heldout/mul24_mac_replay_candidate.v \
  results/discovery/mac_crosswidth_20260905/evo608_heldout/mul24_mac_replay_norules.v
```

Negative regression cases corrupt partial-product coverage, compressor inputs,
column weights, frontier access, drivers, output coverage, and source syntax.
Additional SAT checks require counterexamples for a broken full-adder cell
and corruption of the high status and sum bits. These adversarial tests help
detect an unsound admission path; they do not remove the checker's trust boundary.

## Subsequent mapped-netlist equivalence

`scripts/prove_mac_mapped.py` separately proves all eight frozen W=12/24
candidate/sibling default/classic mappings equivalent to their proven source
using direct ABC CEC on Yosys-exported AIGs. It checks exact port correspondence
and records source/mapped/Liberty hashes. The W=24/default hashes match the
routing experiment's inputs. See `evo608_mapped_equivalence/` in the campaign
for the complete results and gate-model scope. This follows the structural
proof; the conservation checker itself still does not parse mapped cells.
