//! Correctness-preserving, genuinely multi-root arithmetic genomes.
//!
//! A `FamilyGenome` develops one shared multiplier graph, then grows every
//! requested result root into that same graph.  The final prune is performed
//! over the union of the roots, so sharing is physical rather than a later
//! accounting convention.  Root-plan genes choose equivalent realizations;
//! every developed child is still verified jointly.

use crate::evo::{
    develop, reference_corpus, verify_operation_family, Genome, OP_ALL, OP_HIGH, OP_LOW,
    OP_OVERFLOW, OP_ROUND, OP_SATURATE,
};
use crate::gen::{Netlist, Wire};
use crate::Rng;
use serde_json::{json, Value};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ReductionShape {
    Linear,
    Balanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoundCarry {
    Ripple,
    Prefix,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SaturationImpl {
    Mux,
    OrMask,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RootPlan {
    pub overflow: ReductionShape,
    pub round_carry: RoundCarry,
    pub saturation: SaturationImpl,
    /// Reuse the exported overflow predicate in saturation.  If false, grow
    /// an independent but equivalent predicate cone, allowing placement and
    /// fanout to trade logic area for timing.
    pub share_predicate: bool,
}

impl Default for RootPlan {
    fn default() -> Self {
        Self {
            overflow: ReductionShape::Balanced,
            round_carry: RoundCarry::Ripple,
            saturation: SaturationImpl::Mux,
            share_predicate: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FamilyGenome {
    pub arithmetic: Genome,
    /// Fixed semantic contract. At least three of the five result roots must
    /// be present; evolution changes their shared realization, not the task.
    pub mask: u8,
    pub roots: RootPlan,
}

impl FamilyGenome {
    pub fn new(arithmetic: Genome, mask: u8, roots: RootPlan) -> Self {
        assert!(
            mask != 0 && mask & !OP_ALL == 0 && mask.count_ones() >= 3,
            "a multi-root family requires at least three valid outputs"
        );
        Self {
            arithmetic,
            mask,
            roots,
        }
    }

    pub fn mutate_named(&self, rng: &mut Rng) -> (Self, &'static str) {
        let mut child = self.clone();
        let name = match rng.below(5) {
            0 => {
                let (g, n) = child.arithmetic.mutate_named(rng);
                child.arithmetic = g;
                n
            }
            1 => {
                child.roots.overflow = flip_reduction(child.roots.overflow);
                "root_overflow_shape"
            }
            2 => {
                child.roots.round_carry = flip_round(child.roots.round_carry);
                "root_round_carry"
            }
            3 => {
                child.roots.saturation = flip_sat(child.roots.saturation);
                "root_saturation"
            }
            _ => {
                child.roots.share_predicate = !child.roots.share_predicate;
                "root_predicate_sharing"
            }
        };
        (child, name)
    }

    /// Homologous arithmetic crossover plus independently aligned root genes.
    /// Both parents must implement the same semantic contract.
    pub fn crossover(&self, other: &Self, rng: &mut Rng) -> Self {
        assert_eq!(self.mask, other.mask, "cannot cross different contracts");
        Self {
            arithmetic: self.arithmetic.crossover(&other.arithmetic, rng),
            mask: self.mask,
            roots: RootPlan {
                overflow: choose(self.roots.overflow, other.roots.overflow, rng),
                round_carry: choose(self.roots.round_carry, other.roots.round_carry, rng),
                saturation: choose(self.roots.saturation, other.roots.saturation, rng),
                share_predicate: choose(
                    self.roots.share_predicate,
                    other.roots.share_predicate,
                    rng,
                ),
            },
        }
    }

    /// JSON metadata suitable for manifests. The arithmetic genome already
    /// has the repository's canonical text encoding; keep it as lossless
    /// debug text here until serde derives are added to its public types.
    pub fn to_json(&self) -> Value {
        json!({
            "mask": self.mask,
            "minimum_outputs": 3,
            "arithmetic": serde_json::from_str::<Value>(&self.arithmetic.to_json())
                .expect("canonical arithmetic genome JSON"),
            "overflow_reduction": format!("{:?}", self.roots.overflow),
            "round_carry": format!("{:?}", self.roots.round_carry),
            "saturation": format!("{:?}", self.roots.saturation),
            "share_predicate": self.roots.share_predicate,
        })
    }
}

#[derive(Clone, Debug)]
pub struct FamilyElite {
    pub id: usize,
    pub genome: FamilyGenome,
    pub depth16: usize,
    pub gates16: usize,
    pub generation: usize,
    pub operator: &'static str,
}

#[derive(Clone, Debug)]
pub struct FamilyIsland {
    pub mask: u8,
    pub evaluated: usize,
    pub rejected: usize,
    pub elites: Vec<FamilyElite>,
    next_id: usize,
}

impl FamilyIsland {
    fn new(mask: u8) -> Self {
        Self {
            mask,
            evaluated: 0,
            rejected: 0,
            elites: Vec::new(),
            next_id: 0,
        }
    }

    fn offer(
        &mut self,
        genome: FamilyGenome,
        generation: usize,
        operator: &'static str,
        seed: u64,
    ) {
        self.evaluated += 1;
        let net = develop_family(&genome, 16);
        let counts = net.counts();
        let candidate = (counts.gate_depth, counts.simple_gates);
        if self.elites.iter().any(|elite| {
            elite.depth16 <= candidate.0
                && elite.gates16 <= candidate.1
                && (elite.depth16 < candidate.0 || elite.gates16 < candidate.1)
        }) {
            return;
        }
        // Exact construction makes the cheap structural rejection safe, but
        // every would-be archive member must still pass the independent joint
        // oracle before it can become a reported result.
        let check8 = develop_family(&genome, 8);
        if verify_family(&check8, self.mask, 0, seed).is_err()
            || verify_family(&net, self.mask, 256, seed ^ 16).is_err()
        {
            self.rejected += 1;
            return;
        }
        self.elites.retain(|elite| {
            !(candidate.0 <= elite.depth16
                && candidate.1 <= elite.gates16
                && (candidate.0 < elite.depth16 || candidate.1 < elite.gates16))
        });
        let id = self.next_id;
        self.next_id += 1;
        self.elites.push(FamilyElite {
            id,
            genome,
            depth16: candidate.0,
            gates16: candidate.1,
            generation,
            operator,
        });
    }

    pub fn best(&self) -> Option<&FamilyElite> {
        self.elites
            .iter()
            .min_by_key(|elite| (elite.depth16, elite.gates16))
    }
}

fn root_plans() -> Vec<RootPlan> {
    let mut plans = Vec::new();
    for overflow in [ReductionShape::Linear, ReductionShape::Balanced] {
        for round_carry in [RoundCarry::Ripple, RoundCarry::Prefix] {
            for saturation in [SaturationImpl::Mux, SaturationImpl::OrMask] {
                for share_predicate in [false, true] {
                    plans.push(RootPlan {
                        overflow,
                        round_carry,
                        saturation,
                        share_predicate,
                    });
                }
            }
        }
    }
    plans
}

/// Fixed-budget search over sixteen incomparable semantic contracts. Each
/// child is one union-pruned multi-root circuit; output count never provides
/// a fitness shortcut because families own independent Pareto archives.
pub fn evolve_multi_root_families(
    generations: usize,
    seed: u64,
    mut log: impl FnMut(&str),
) -> Vec<FamilyIsland> {
    let masks: Vec<u8> = (1..=OP_ALL).filter(|mask| mask.count_ones() >= 3).collect();
    let mut islands: Vec<_> = masks.iter().copied().map(FamilyIsland::new).collect();
    let mut rng = Rng::new(seed);
    for island in &mut islands {
        for (_, arithmetic) in reference_corpus() {
            for roots in root_plans() {
                island.offer(
                    FamilyGenome::new(arithmetic.clone(), island.mask, roots),
                    0,
                    "control_seed",
                    seed ^ island.mask as u64,
                );
            }
        }
    }
    for generation in 1..=generations {
        let island_index = rng.below(islands.len());
        let parent_index = rng.below(islands[island_index].elites.len());
        let parent = islands[island_index].elites[parent_index].genome.clone();
        let (child, operator) = if islands[island_index].elites.len() > 1 && rng.below(3) == 0 {
            let other_index = rng.below(islands[island_index].elites.len());
            let other = islands[island_index].elites[other_index].genome.clone();
            let crossed = parent.crossover(&other, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "multi_root_crossover")
        } else {
            parent.mutate_named(&mut rng)
        };
        let mask = islands[island_index].mask;
        islands[island_index].offer(
            child,
            generation,
            operator,
            seed ^ generation as u64 ^ mask as u64,
        );
        if generation % 5_000 == 0 {
            log(&format!(
                "generation {generation}: {} multi-root islands, {} Pareto elites, {} evaluations",
                islands.len(),
                islands
                    .iter()
                    .map(|island| island.elites.len())
                    .sum::<usize>(),
                islands.iter().map(|island| island.evaluated).sum::<usize>()
            ));
        }
    }
    islands
}

fn choose<T: Copy>(a: T, b: T, rng: &mut Rng) -> T {
    if rng.below(2) == 0 {
        a
    } else {
        b
    }
}
fn flip_reduction(x: ReductionShape) -> ReductionShape {
    match x {
        ReductionShape::Linear => ReductionShape::Balanced,
        ReductionShape::Balanced => ReductionShape::Linear,
    }
}
fn flip_round(x: RoundCarry) -> RoundCarry {
    match x {
        RoundCarry::Ripple => RoundCarry::Prefix,
        RoundCarry::Prefix => RoundCarry::Ripple,
    }
}
fn flip_sat(x: SaturationImpl) -> SaturationImpl {
    match x {
        SaturationImpl::Mux => SaturationImpl::OrMask,
        SaturationImpl::OrMask => SaturationImpl::Mux,
    }
}

fn reduce_or(net: &mut Netlist, mut xs: Vec<Wire>, shape: ReductionShape) -> Wire {
    if xs.is_empty() {
        return Wire::Const0;
    }
    if shape == ReductionShape::Linear {
        return xs.into_iter().fold(Wire::Const0, |a, b| net.or2(a, b));
    }
    while xs.len() > 1 {
        xs = xs
            .chunks(2)
            .map(|p| {
                if p.len() == 2 {
                    net.or2(p[0], p[1])
                } else {
                    p[0]
                }
            })
            .collect();
    }
    xs[0]
}

fn rounded_high(net: &mut Netlist, product: &[Wire], width: usize, kind: RoundCarry) -> Vec<Wire> {
    let high = &product[width..];
    let round = product[width - 1];
    match kind {
        RoundCarry::Ripple => {
            let mut carry = round;
            high.iter()
                .map(|&bit| {
                    let y = net.xor2(bit, carry);
                    carry = net.and2(bit, carry);
                    y
                })
                .collect()
        }
        RoundCarry::Prefix => {
            // Increment generate for bit i is round & high[0] & ... & high[i-1].
            // A doubling prefix computes every conjunction at logarithmic depth.
            let mut prefix = high.to_vec();
            let mut distance = 1;
            while distance < width {
                let old = prefix.clone();
                for i in distance..width {
                    prefix[i] = net.and2(old[i], old[i - distance]);
                }
                distance *= 2;
            }
            let mut result = Vec::with_capacity(width);
            for i in 0..width {
                let carry = if i == 0 {
                    round
                } else {
                    net.and2(round, prefix[i - 1])
                };
                result.push(net.xor2(high[i], carry));
            }
            result
        }
    }
}

/// Develop one union-pruned graph. Output order is canonical:
/// low, high, rounded, saturated, overflow; absent roots consume no pins.
pub fn develop_family(genome: &FamilyGenome, width: usize) -> Netlist {
    let (mut net, _) = develop(&genome.arithmetic, width);
    let product = net.outputs.clone();
    let low = product[..width].to_vec();
    let high = product[width..].to_vec();
    let overflow = reduce_or(&mut net, high.clone(), genome.roots.overflow);
    let rounded = if genome.mask & OP_ROUND != 0 {
        rounded_high(&mut net, &product, width, genome.roots.round_carry)
    } else {
        Vec::new()
    };
    let sat_predicate = if genome.roots.share_predicate || genome.mask & OP_OVERFLOW == 0 {
        overflow
    } else {
        reduce_or(&mut net, high.clone(), genome.roots.overflow)
    };
    let saturated: Vec<_> = if genome.mask & OP_SATURATE == 0 {
        Vec::new()
    } else {
        low.iter()
            .map(|&bit| match genome.roots.saturation {
                SaturationImpl::Mux => net.mux2(sat_predicate, bit, Wire::Const1),
                SaturationImpl::OrMask => net.or2(bit, sat_predicate),
            })
            .collect()
    };
    net.outputs.clear();
    if genome.mask & OP_LOW != 0 {
        net.outputs.extend_from_slice(&low);
    }
    if genome.mask & OP_HIGH != 0 {
        net.outputs.extend_from_slice(&high);
    }
    if genome.mask & OP_ROUND != 0 {
        net.outputs.extend_from_slice(&rounded);
    }
    if genome.mask & OP_SATURATE != 0 {
        net.outputs.extend_from_slice(&saturated);
    }
    if genome.mask & OP_OVERFLOW != 0 {
        net.outputs.push(overflow);
    }
    net.label = format!("multiroot_{:02x}", genome.mask);
    net.prune();
    net
}

/// Joint verification: a candidate survives only if its complete combined
/// output word is correct. The shared oracle exhausts all pairs through eight
/// bits, applies deterministic samples and corners above that, and handles
/// output families wider than one machine word in independent chunks.
pub fn verify_family(net: &Netlist, mask: u8, samples: usize, seed: u64) -> Result<(), String> {
    verify_operation_family(net, mask, samples, seed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Adder;

    #[test]
    fn every_root_plan_is_jointly_exact() {
        let mask = OP_LOW | OP_ROUND | OP_SATURATE | OP_OVERFLOW;
        for overflow in [ReductionShape::Linear, ReductionShape::Balanced] {
            for round_carry in [RoundCarry::Ripple, RoundCarry::Prefix] {
                for saturation in [SaturationImpl::Mux, SaturationImpl::OrMask] {
                    for share_predicate in [false, true] {
                        let g = FamilyGenome::new(
                            Genome::dadda(Adder::BrentKung),
                            mask,
                            RootPlan {
                                overflow,
                                round_carry,
                                saturation,
                                share_predicate,
                            },
                        );
                        verify_family(&develop_family(&g, 4), mask, 0, 1).unwrap();
                    }
                }
            }
        }
    }

    #[test]
    #[should_panic(expected = "at least three")]
    fn rejects_single_output_contracts() {
        FamilyGenome::new(Genome::dadda(Adder::Ripple), OP_LOW, RootPlan::default());
    }

    #[test]
    fn mutation_and_crossover_preserve_contract() {
        let mask = OP_HIGH | OP_ROUND | OP_SATURATE;
        let a = FamilyGenome::new(Genome::dadda(Adder::Ripple), mask, RootPlan::default());
        let b = FamilyGenome::new(
            Genome::wallace(Adder::KoggeStone),
            mask,
            RootPlan {
                share_predicate: false,
                ..RootPlan::default()
            },
        );
        let mut rng = Rng::new(19);
        for _ in 0..32 {
            let (child, _) = a.crossover(&b, &mut rng).mutate_named(&mut rng);
            assert_eq!(child.mask, mask);
            verify_family(&develop_family(&child, 4), mask, 0, 2).unwrap();
        }
    }
}
