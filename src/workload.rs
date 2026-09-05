//! Typed, coherent workload genomes.
//!
//! Unlike the exploratory operation-mask search, each genome implements one
//! numerical contract. Arithmetic growth and result/status lowering mutate in
//! one chromosome and develop into one union-pruned circuit.

use crate::evo::{develop, reference_corpus, Genome};
use crate::gen::{Netlist, Wire};
use crate::multiroot::{ReductionShape, RoundCarry, SaturationImpl};
use crate::Rng;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RequantPlan {
    pub sticky_reduction: ReductionShape,
    pub saturation_reduction: ReductionShape,
    pub round_carry: RoundCarry,
    pub saturation: SaturationImpl,
    pub share_saturation_predicate: bool,
}

impl Default for RequantPlan {
    fn default() -> Self {
        Self {
            sticky_reduction: ReductionShape::Balanced,
            saturation_reduction: ReductionShape::Balanced,
            round_carry: RoundCarry::Prefix,
            saturation: SaturationImpl::OrMask,
            share_saturation_predicate: true,
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RequantGenome {
    pub arithmetic: Genome,
    pub plan: RequantPlan,
}

impl RequantGenome {
    pub fn mutate_named(&self, rng: &mut Rng) -> (Self, &'static str) {
        let mut child = self.clone();
        let name = match rng.below(6) {
            0 => {
                let (arithmetic, name) = child.arithmetic.mutate_named(rng);
                child.arithmetic = arithmetic;
                name
            }
            1 => {
                child.plan.sticky_reduction = flip_reduction(child.plan.sticky_reduction);
                "sticky_reduction"
            }
            2 => {
                child.plan.saturation_reduction = flip_reduction(child.plan.saturation_reduction);
                "saturation_reduction"
            }
            3 => {
                child.plan.round_carry = flip_round(child.plan.round_carry);
                "round_carry"
            }
            4 => {
                child.plan.saturation = flip_saturation(child.plan.saturation);
                "saturation_impl"
            }
            _ => {
                child.plan.share_saturation_predicate = !child.plan.share_saturation_predicate;
                "saturation_sharing"
            }
        };
        child.arithmetic.drop16 = 0;
        (child, name)
    }

    pub fn crossover(&self, other: &Self, rng: &mut Rng) -> Self {
        Self {
            arithmetic: self.arithmetic.crossover(&other.arithmetic, rng),
            plan: RequantPlan {
                sticky_reduction: choose(
                    self.plan.sticky_reduction,
                    other.plan.sticky_reduction,
                    rng,
                ),
                saturation_reduction: choose(
                    self.plan.saturation_reduction,
                    other.plan.saturation_reduction,
                    rng,
                ),
                round_carry: choose(self.plan.round_carry, other.plan.round_carry, rng),
                saturation: choose(self.plan.saturation, other.plan.saturation, rng),
                share_saturation_predicate: choose(
                    self.plan.share_saturation_predicate,
                    other.plan.share_saturation_predicate,
                    rng,
                ),
            },
        }
    }
}

fn choose<T: Copy>(a: T, b: T, rng: &mut Rng) -> T {
    if rng.below(2) == 0 {
        a
    } else {
        b
    }
}

fn flip_reduction(value: ReductionShape) -> ReductionShape {
    match value {
        ReductionShape::Linear => ReductionShape::Balanced,
        ReductionShape::Balanced => ReductionShape::Linear,
    }
}

fn flip_round(value: RoundCarry) -> RoundCarry {
    match value {
        RoundCarry::Ripple => RoundCarry::Prefix,
        RoundCarry::Prefix => RoundCarry::Ripple,
    }
}

fn flip_saturation(value: SaturationImpl) -> SaturationImpl {
    match value {
        SaturationImpl::Mux => SaturationImpl::OrMask,
        SaturationImpl::OrMask => SaturationImpl::Mux,
    }
}

pub(crate) fn reduce_or(net: &mut Netlist, mut wires: Vec<Wire>, shape: ReductionShape) -> Wire {
    if wires.is_empty() {
        return Wire::Const0;
    }
    if shape == ReductionShape::Linear {
        return wires
            .into_iter()
            .fold(Wire::Const0, |acc, wire| net.or2(acc, wire));
    }
    while wires.len() > 1 {
        wires = wires
            .chunks(2)
            .map(|pair| {
                if pair.len() == 2 {
                    net.or2(pair[0], pair[1])
                } else {
                    pair[0]
                }
            })
            .collect();
    }
    wires[0]
}

/// Add the fixed rounding constant at bit `shift-1`. Returns the rounded full
/// word. Prefix mode constructs every increment carry with a logarithmic AND
/// prefix; ripple mode follows the carry serially.
pub(crate) fn add_rounding(
    net: &mut Netlist,
    product: &[Wire],
    shift: usize,
    mode: RoundCarry,
) -> Vec<Wire> {
    let start = shift - 1;
    let mut result = product.to_vec();
    result[start] = net.xor2(product[start], Wire::Const1);
    match mode {
        RoundCarry::Ripple => {
            let mut carry = product[start];
            for index in start + 1..product.len() {
                result[index] = net.xor2(product[index], carry);
                carry = net.and2(product[index], carry);
            }
        }
        RoundCarry::Prefix => {
            let source = &product[start..];
            let mut prefix = source.to_vec();
            let mut distance = 1;
            while distance < prefix.len() {
                let old = prefix.clone();
                for index in distance..prefix.len() {
                    prefix[index] = net.and2(old[index], old[index - distance]);
                }
                distance *= 2;
            }
            for index in start + 1..product.len() {
                result[index] = net.xor2(product[index], prefix[index - start - 1]);
            }
        }
    }
    result
}

/// Unsigned fixed-shift requantization. For shift S=W/2:
/// rounded=(a*b+2^(S-1))>>S; result=min(rounded,2^W-1);
/// saturated=(rounded>=2^W); inexact=OR((a*b)[S-1:0]).
/// Output order is result[W-1:0], saturated, inexact.
pub fn develop_requant(genome: &RequantGenome, width: usize) -> Netlist {
    assert!(
        width >= 2 && width.is_multiple_of(2),
        "requant width must be even"
    );
    let shift = width / 2;
    let (mut net, _) = develop(&genome.arithmetic, width);
    let product = net.outputs.clone();
    let rounded = add_rounding(&mut net, &product, shift, genome.plan.round_carry);
    let raw = rounded[shift..shift + width].to_vec();
    let saturated = reduce_or(
        &mut net,
        rounded[shift + width..].to_vec(),
        genome.plan.saturation_reduction,
    );
    let clamp_predicate = if genome.plan.share_saturation_predicate {
        saturated
    } else {
        reduce_or(
            &mut net,
            rounded[shift + width..].to_vec(),
            genome.plan.saturation_reduction,
        )
    };
    let result: Vec<_> = raw
        .iter()
        .map(|&bit| match genome.plan.saturation {
            SaturationImpl::Mux => net.mux2(clamp_predicate, bit, Wire::Const1),
            SaturationImpl::OrMask => net.or2(bit, clamp_predicate),
        })
        .collect();
    let sticky = reduce_or(
        &mut net,
        product[..shift].to_vec(),
        genome.plan.sticky_reduction,
    );
    let inexact = net.or2(saturated, sticky);
    net.outputs = result;
    net.outputs.push(saturated);
    net.outputs.push(inexact);
    net.label = "requant_result_sat_inexact".to_string();
    net.prune();
    net
}

fn expected(width: usize, a: u64, b: u64) -> u64 {
    let shift = width / 2;
    let product = a * b;
    let rounded = (product + (1 << (shift - 1))) >> shift;
    let limit = (1u64 << width) - 1;
    let saturated = rounded > limit;
    let result = rounded.min(limit);
    let inexact = saturated || product & ((1 << shift) - 1) != 0;
    result | ((saturated as u64) << width) | ((inexact as u64) << (width + 1))
}

pub fn verify_requant(net: &Netlist, samples: usize, seed: u64) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    let total = if width <= 8 {
        1usize << (2 * width)
    } else {
        samples
    };
    let mut rng = Rng::new(seed);
    for base in (0..total).step_by(64) {
        let lanes = (total - base).min(64);
        let mut a_bits = vec![0u64; width];
        let mut b_bits = vec![0u64; width];
        let mut want = vec![0u64; width + 2];
        let mut pairs = Vec::with_capacity(lanes);
        for lane in 0..lanes {
            let (a, b) = if width <= 8 {
                (
                    (base + lane) as u64 & mask,
                    ((base + lane) as u64 >> width) & mask,
                )
            } else {
                (rng.next_u64() & mask, rng.next_u64() & mask)
            };
            pairs.push((a, b));
            let value = expected(width, a, b);
            for (bit, word) in want.iter_mut().enumerate() {
                *word |= ((value >> bit) & 1) << lane;
            }
            for bit in 0..width {
                a_bits[bit] |= ((a >> bit) & 1) << lane;
                b_bits[bit] |= ((b >> bit) & 1) << lane;
            }
        }
        let got = net.eval_batch64(&a_bits, &b_bits);
        if let Some(bit) = got.iter().zip(&want).position(|(a, b)| a != b) {
            let lane = (got[bit] ^ want[bit]).trailing_zeros() as usize;
            return Err(format!(
                "requant mismatch at width {width}: a={} b={} output bit {bit}",
                pairs[lane].0, pairs[lane].1
            ));
        }
    }
    if width > 8 {
        for &a in &[0, 1, 2, mask / 2, mask - 1, mask] {
            for &b in &[0, 1, 2, mask / 2, mask - 1, mask] {
                if net.eval(a, b) != expected(width, a, b) {
                    return Err(format!("requant corner mismatch: {a} * {b}"));
                }
            }
        }
    }
    Ok(())
}

#[derive(Clone, Debug)]
pub struct RequantElite {
    pub id: usize,
    pub genome: RequantGenome,
    pub depth16: usize,
    pub gates16: usize,
    pub routing16: usize,
    pub generation: usize,
    pub operator: &'static str,
}

fn plans() -> Vec<RequantPlan> {
    let mut result = Vec::new();
    for sticky_reduction in [ReductionShape::Linear, ReductionShape::Balanced] {
        for saturation_reduction in [ReductionShape::Linear, ReductionShape::Balanced] {
            for round_carry in [RoundCarry::Ripple, RoundCarry::Prefix] {
                for saturation in [SaturationImpl::Mux, SaturationImpl::OrMask] {
                    for share_saturation_predicate in [false, true] {
                        result.push(RequantPlan {
                            sticky_reduction,
                            saturation_reduction,
                            round_carry,
                            saturation,
                            share_saturation_predicate,
                        });
                    }
                }
            }
        }
    }
    result
}

fn offer(
    archive: &mut Vec<RequantElite>,
    genome: RequantGenome,
    generation: usize,
    operator: &'static str,
    id: &mut usize,
    seed: u64,
    routing_aware: bool,
) {
    let net16 = develop_requant(&genome, 16);
    let counts = net16.counts();
    let routing = net16.routing_proxy().score;
    let score = (counts.gate_depth, counts.simple_gates, routing);
    if archive.iter().any(|elite| {
        elite.depth16 <= score.0
            && elite.gates16 <= score.1
            && (!routing_aware || elite.routing16 <= score.2)
    }) {
        return;
    }
    let net8 = develop_requant(&genome, 8);
    if verify_requant(&net8, 0, seed).is_err() || verify_requant(&net16, 512, seed ^ 16).is_err() {
        return;
    }
    archive.retain(|elite| {
        !(score.0 <= elite.depth16
            && score.1 <= elite.gates16
            && (!routing_aware || score.2 <= elite.routing16)
            && (score.0 < elite.depth16
                || score.1 < elite.gates16
                || (routing_aware && score.2 < elite.routing16)))
    });
    archive.push(RequantElite {
        id: *id,
        genome,
        depth16: score.0,
        gates16: score.1,
        routing16: score.2,
        generation,
        operator,
    });
    *id += 1;
}

pub fn evolve_requant(generations: usize, seed: u64, log: impl FnMut(&str)) -> Vec<RequantElite> {
    evolve_requant_objectives(generations, seed, false, log)
}

pub fn evolve_requant_routing(
    generations: usize,
    seed: u64,
    log: impl FnMut(&str),
) -> Vec<RequantElite> {
    evolve_requant_objectives(generations, seed, true, log)
}

fn evolve_requant_objectives(
    generations: usize,
    seed: u64,
    routing_aware: bool,
    mut log: impl FnMut(&str),
) -> Vec<RequantElite> {
    let mut archive = Vec::new();
    let mut next_id = 0;
    let mut rng = Rng::new(seed);
    for (_, arithmetic) in reference_corpus() {
        for plan in plans() {
            offer(
                &mut archive,
                RequantGenome {
                    arithmetic: arithmetic.clone(),
                    plan,
                },
                0,
                "control_seed",
                &mut next_id,
                seed,
                routing_aware,
            );
        }
    }
    for generation in 1..=generations {
        let parent = archive[rng.below(archive.len())].genome.clone();
        let (child, operator) = if archive.len() > 1 && rng.below(3) == 0 {
            let other = archive[rng.below(archive.len())].genome.clone();
            let crossed = parent.crossover(&other, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "requant_crossover")
        } else {
            parent.mutate_named(&mut rng)
        };
        offer(
            &mut archive,
            child,
            generation,
            operator,
            &mut next_id,
            seed ^ generation as u64,
            routing_aware,
        );
        if generation % 10_000 == 0 {
            log(&format!(
                "generation {generation}: {} requant Pareto elites",
                archive.len()
            ));
        }
    }
    archive
}

/// Pareto frontier before evolution, spanning every textbook arithmetic trunk
/// and every exact requant root plan. Kept separate so the physical audit can
/// never mistake a proxy-selected control for the complete baseline.
pub fn requant_control_pareto(seed: u64) -> Vec<RequantElite> {
    let mut archive = Vec::new();
    let mut next_id = 0;
    for (_, arithmetic) in reference_corpus() {
        for plan in plans() {
            offer(
                &mut archive,
                RequantGenome {
                    arithmetic: arithmetic.clone(),
                    plan,
                },
                0,
                "control_seed",
                &mut next_id,
                seed,
                false,
            );
        }
    }
    archive
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Adder;

    #[test]
    fn every_requant_root_plan_is_exact() {
        for plan in plans() {
            let genome = RequantGenome {
                arithmetic: Genome::dadda(Adder::BrentKung),
                plan,
            };
            verify_requant(&develop_requant(&genome, 8), 0, 7).unwrap();
        }
    }

    #[test]
    fn mutation_and_crossover_preserve_requant_contract() {
        let a = RequantGenome {
            arithmetic: Genome::dadda(Adder::Ripple),
            plan: RequantPlan::default(),
        };
        let b = RequantGenome {
            arithmetic: Genome::wallace(Adder::KoggeStone),
            plan: RequantPlan {
                share_saturation_predicate: false,
                ..RequantPlan::default()
            },
        };
        let mut rng = Rng::new(11);
        for _ in 0..16 {
            let (child, _) = a.crossover(&b, &mut rng).mutate_named(&mut rng);
            verify_requant(&develop_requant(&child, 8), 0, 9).unwrap();
        }
    }
}
