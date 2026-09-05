//! One-genome fused multiply-accumulate and requantization search.

use crate::evo::{develop, develop_fused_mac, reference_corpus, Genome};
use crate::gen::{Netlist, Wire};
use crate::multiroot::{ReductionShape, RoundCarry, SaturationImpl};
use crate::workload::{add_rounding, reduce_or, RequantPlan};
use crate::Rng;
use std::collections::BTreeMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AccumulatorCarry {
    Ripple,
    Prefix,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MacGenome {
    pub arithmetic: Genome,
    pub requant: RequantPlan,
    pub accumulator_carry: AccumulatorCarry,
    pub fuse_accumulator: bool,
}

impl MacGenome {
    /// Load the frozen JSON format emitted by the MAC search.
    pub fn from_json(text: &str) -> Result<Self, String> {
        let v: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let string = |key: &str| {
            v[key]
                .as_str()
                .ok_or_else(|| format!("missing string {key}"))
        };
        let boolean = |key: &str| {
            v[key]
                .as_bool()
                .ok_or_else(|| format!("missing boolean {key}"))
        };
        let reduction = |key: &str| match string(key)? {
            "Balanced" => Ok(ReductionShape::Balanced),
            "Linear" => Ok(ReductionShape::Linear),
            name => Err(format!("unknown {key}: {name}")),
        };
        let arithmetic = Genome::from_json(&v["arithmetic"].to_string())?;
        if arithmetic.drop16 != 0 {
            return Err("MAC requires exact arithmetic (drop16=0)".into());
        }
        Ok(Self {
            arithmetic,
            accumulator_carry: match string("accumulator_carry")? {
                "Ripple" => AccumulatorCarry::Ripple,
                "Prefix" => AccumulatorCarry::Prefix,
                name => return Err(format!("unknown accumulator_carry: {name}")),
            },
            fuse_accumulator: boolean("fuse_accumulator")?,
            requant: RequantPlan {
                sticky_reduction: reduction("sticky_reduction")?,
                saturation_reduction: reduction("saturation_reduction")?,
                round_carry: match string("round_carry")? {
                    "Ripple" => RoundCarry::Ripple,
                    "Prefix" => RoundCarry::Prefix,
                    name => return Err(format!("unknown round_carry: {name}")),
                },
                saturation: match string("saturation_impl")? {
                    "Mux" => SaturationImpl::Mux,
                    "OrMask" => SaturationImpl::OrMask,
                    name => return Err(format!("unknown saturation_impl: {name}")),
                },
                share_saturation_predicate: boolean("share_saturation")?,
            },
        })
    }

    pub fn mutate_named(&self, rng: &mut Rng) -> (Self, &'static str) {
        let mut child = self.clone();
        let name = match rng.below(8) {
            0 => {
                let (arithmetic, name) = child.arithmetic.mutate_named(rng);
                child.arithmetic = arithmetic;
                name
            }
            1 => {
                child.accumulator_carry = match child.accumulator_carry {
                    AccumulatorCarry::Ripple => AccumulatorCarry::Prefix,
                    AccumulatorCarry::Prefix => AccumulatorCarry::Ripple,
                };
                "accumulator_carry"
            }
            2 => {
                child.fuse_accumulator = !child.fuse_accumulator;
                "accumulator_fusion"
            }
            3 => {
                child.requant.sticky_reduction = flip_reduction(child.requant.sticky_reduction);
                "sticky_reduction"
            }
            4 => {
                child.requant.saturation_reduction =
                    flip_reduction(child.requant.saturation_reduction);
                "saturation_reduction"
            }
            5 => {
                child.requant.round_carry = match child.requant.round_carry {
                    RoundCarry::Ripple => RoundCarry::Prefix,
                    RoundCarry::Prefix => RoundCarry::Ripple,
                };
                "round_carry"
            }
            6 => {
                child.requant.saturation = match child.requant.saturation {
                    SaturationImpl::Mux => SaturationImpl::OrMask,
                    SaturationImpl::OrMask => SaturationImpl::Mux,
                };
                "saturation_impl"
            }
            _ => {
                child.requant.share_saturation_predicate =
                    !child.requant.share_saturation_predicate;
                "saturation_sharing"
            }
        };
        child.arithmetic.drop16 = 0;
        (child, name)
    }

    pub fn crossover(&self, other: &Self, rng: &mut Rng) -> Self {
        Self {
            arithmetic: self.arithmetic.crossover(&other.arithmetic, rng),
            requant: RequantPlan {
                sticky_reduction: pick(
                    self.requant.sticky_reduction,
                    other.requant.sticky_reduction,
                    rng,
                ),
                saturation_reduction: pick(
                    self.requant.saturation_reduction,
                    other.requant.saturation_reduction,
                    rng,
                ),
                round_carry: pick(self.requant.round_carry, other.requant.round_carry, rng),
                saturation: pick(self.requant.saturation, other.requant.saturation, rng),
                share_saturation_predicate: pick(
                    self.requant.share_saturation_predicate,
                    other.requant.share_saturation_predicate,
                    rng,
                ),
            },
            accumulator_carry: pick(self.accumulator_carry, other.accumulator_carry, rng),
            fuse_accumulator: pick(self.fuse_accumulator, other.fuse_accumulator, rng),
        }
    }
}

fn pick<T: Copy>(a: T, b: T, rng: &mut Rng) -> T {
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

fn add_words(
    net: &mut Netlist,
    left: &[Wire],
    right: &[Wire],
    mode: AccumulatorCarry,
) -> Vec<Wire> {
    assert_eq!(left.len(), right.len());
    match mode {
        AccumulatorCarry::Ripple => {
            let mut result = Vec::with_capacity(left.len() + 1);
            let mut carry = Wire::Const0;
            for (&a, &b) in left.iter().zip(right) {
                let (sum, next) = if carry == Wire::Const0 {
                    net.ha(a, b)
                } else {
                    net.fa(a, b, carry)
                };
                result.push(sum);
                carry = next;
            }
            result.push(carry);
            result
        }
        AccumulatorCarry::Prefix => {
            let base_p: Vec<_> = left
                .iter()
                .zip(right)
                .map(|(&a, &b)| net.xor2(a, b))
                .collect();
            let mut p = base_p.clone();
            let mut g: Vec<_> = left
                .iter()
                .zip(right)
                .map(|(&a, &b)| net.and2(a, b))
                .collect();
            let mut distance = 1;
            while distance < left.len() {
                let old_p = p.clone();
                let old_g = g.clone();
                for index in distance..left.len() {
                    let propagated = net.and2(old_p[index], old_g[index - distance]);
                    g[index] = net.or2(old_g[index], propagated);
                    p[index] = net.and2(old_p[index], old_p[index - distance]);
                }
                distance *= 2;
            }
            let mut result = Vec::with_capacity(left.len() + 1);
            result.push(base_p[0]);
            for index in 1..left.len() {
                result.push(net.xor2(base_p[index], g[index - 1]));
            }
            result.push(g[left.len() - 1]);
            result
        }
    }
}

/// Outputs are four semantic roots packed as:
/// accumulator[2W:0], result[W-1:0], saturated, status.
/// status is inexact OR accumulator overflow.
pub fn develop_mac(genome: &MacGenome, width: usize) -> Netlist {
    assert!(width >= 2 && width.is_multiple_of(2));
    let (mut net, _) = if genome.fuse_accumulator {
        develop_fused_mac(&genome.arithmetic, width)
    } else {
        develop(&genome.arithmetic, width)
    };
    let sum = if genome.fuse_accumulator {
        net.outputs.clone()
    } else {
        let product = net.outputs.clone();
        net.aux_width = width * 2;
        let accumulator: Vec<_> = (0..width * 2).map(Wire::Aux).collect();
        add_words(&mut net, &product, &accumulator, genome.accumulator_carry)
    };
    finish_mac_roots(net, sum, genome)
}

/// Attach the identical live output roots to an independently generated MAC sum.
pub(crate) fn finish_mac_roots(mut net: Netlist, sum: Vec<Wire>, genome: &MacGenome) -> Netlist {
    let width = net.width;
    let shift = width / 2;
    assert_eq!(sum.len(), 2 * width + 1);
    let rounded = add_rounding(&mut net, &sum, shift, genome.requant.round_carry);
    let raw = rounded[shift..shift + width].to_vec();
    let saturated = reduce_or(
        &mut net,
        rounded[shift + width..].to_vec(),
        genome.requant.saturation_reduction,
    );
    let clamp = if genome.requant.share_saturation_predicate {
        saturated
    } else {
        reduce_or(
            &mut net,
            rounded[shift + width..].to_vec(),
            genome.requant.saturation_reduction,
        )
    };
    let result: Vec<_> = raw
        .iter()
        .map(|&bit| match genome.requant.saturation {
            SaturationImpl::Mux => net.mux2(clamp, bit, Wire::Const1),
            SaturationImpl::OrMask => net.or2(bit, clamp),
        })
        .collect();
    let sticky = reduce_or(
        &mut net,
        sum[..shift].to_vec(),
        genome.requant.sticky_reduction,
    );
    let acc_overflow = sum[width * 2];
    let inexact = net.or2(saturated, sticky);
    let status = net.or2(inexact, acc_overflow);
    net.outputs = sum;
    net.outputs.extend(result);
    net.outputs.push(saturated);
    net.outputs.push(status);
    net.label = "mac_requant_acc_result_sat_status".to_string();
    net.prune();
    net
}

fn expected(width: usize, a: u64, b: u64, accumulator: u64) -> u128 {
    let (a, b, accumulator) = (a as u128, b as u128, accumulator as u128);
    let shift = width / 2;
    let sum = a * b + accumulator;
    let rounded = (sum + (1 << (shift - 1))) >> shift;
    let limit = (1u128 << width) - 1;
    let saturated = rounded > limit;
    let result = rounded.min(limit);
    let overflow = sum >> (2 * width) != 0;
    let status = saturated || overflow || sum & ((1 << shift) - 1) != 0;
    let sum_mask = (1u128 << (2 * width + 1)) - 1;
    (sum & sum_mask)
        | (result << (2 * width + 1))
        | ((saturated as u128) << (3 * width + 1))
        | ((status as u128) << (3 * width + 2))
}

pub fn verify_mac(net: &Netlist, samples: usize, seed: u64) -> Result<(), String> {
    let width = net.width;
    assert!((2..=32).contains(&width) && width.is_multiple_of(2));
    let operand_mask = (1u64 << width) - 1;
    let accumulator_mask = if width == 32 {
        u64::MAX
    } else {
        (1u64 << (2 * width)) - 1
    };
    // Exercise carry-out, saturation and rounding boundaries independently of RNG.
    let bias = 1u64 << (width / 2 - 1);
    for a in [0, 1, operand_mask - 1, operand_mask] {
        for b in [0, 1, operand_mask - 1, operand_mask] {
            for accumulator in [0, 1, bias - 1, bias, accumulator_mask - 1, accumulator_mask] {
                let got = net.eval_with_aux_u128(a, b, accumulator);
                let want = expected(width, a, b, accumulator);
                if got != want {
                    return Err(format!("MAC corner mismatch width={width} a={a} b={b} accumulator={accumulator}: got={got} want={want}"));
                }
            }
        }
    }
    let mut rng = Rng::new(seed);
    let total = if width <= 4 {
        1usize << (4 * width)
    } else {
        samples
    };
    for index in 0..total {
        let (a, b, accumulator) = if width <= 4 {
            (
                index as u64 & operand_mask,
                (index as u64 >> width) & operand_mask,
                (index as u64 >> (2 * width)) & accumulator_mask,
            )
        } else {
            (
                rng.next_u64() & operand_mask,
                rng.next_u64() & operand_mask,
                rng.next_u64() & accumulator_mask,
            )
        };
        let got = net.eval_with_aux_u128(a, b, accumulator);
        let want = expected(width, a, b, accumulator);
        if got != want {
            return Err(format!(
                "MAC mismatch width={width} a={a} b={b} accumulator={accumulator}: got={got} want={want}"
            ));
        }
    }
    Ok(())
}

pub fn seed_genomes() -> Vec<MacGenome> {
    let mut seeds = Vec::new();
    for (_, arithmetic) in reference_corpus() {
        for accumulator_carry in [AccumulatorCarry::Ripple, AccumulatorCarry::Prefix] {
            for fuse_accumulator in [false, true] {
                seeds.push(MacGenome {
                    arithmetic: arithmetic.clone(),
                    requant: RequantPlan::default(),
                    accumulator_carry,
                    fuse_accumulator,
                });
            }
        }
    }
    seeds
}

#[derive(Clone, Debug)]
pub struct MacElite {
    pub id: usize,
    pub genome: MacGenome,
    pub depth16: usize,
    pub gates16: usize,
    pub routing16: usize,
    pub generation: usize,
    pub operator: &'static str,
}

fn offer(
    archive: &mut Vec<MacElite>,
    genome: MacGenome,
    generation: usize,
    operator: &'static str,
    next_id: &mut usize,
    seed: u64,
) {
    let net16 = develop_mac(&genome, 16);
    let counts = net16.counts();
    let score = (
        counts.gate_depth,
        counts.simple_gates,
        net16.routing_proxy().score,
    );
    if archive.iter().any(|elite| {
        elite.depth16 <= score.0 && elite.gates16 <= score.1 && elite.routing16 <= score.2
    }) {
        return;
    }
    if verify_mac(&develop_mac(&genome, 4), 0, seed).is_err()
        || verify_mac(&develop_mac(&genome, 8), 512, seed ^ 8).is_err()
        || verify_mac(&net16, 512, seed ^ 16).is_err()
    {
        return;
    }
    archive.retain(|elite| {
        !(score.0 <= elite.depth16
            && score.1 <= elite.gates16
            && score.2 <= elite.routing16
            && (score.0 < elite.depth16 || score.1 < elite.gates16 || score.2 < elite.routing16))
    });
    archive.push(MacElite {
        id: *next_id,
        genome,
        depth16: score.0,
        gates16: score.1,
        routing16: score.2,
        generation,
        operator,
    });
    *next_id += 1;
}

pub fn evolve_mac(generations: usize, seed: u64, log: impl FnMut(&str)) -> Vec<MacElite> {
    evolve_mac_internal(generations, seed, true, log)
}

pub fn evolve_mac_postadd(generations: usize, seed: u64, log: impl FnMut(&str)) -> Vec<MacElite> {
    evolve_mac_internal(generations, seed, false, log)
}

/// Retain one strong member of each structural/semantic niche instead of
/// deleting everything outside the cheap-objective Pareto front. This archive
/// is intended for a second-stage physical mapping tournament: the first MAC
/// campaign showed that proxy Pareto selection misses most of the flattened
/// SKY130 frontier.
pub fn evolve_mac_diverse(
    generations: usize,
    seed: u64,
    mut log: impl FnMut(&str),
) -> Vec<MacElite> {
    let mut archive = BTreeMap::new();
    let mut next_id = 0;
    let mut rng = Rng::new(seed);
    for genome in seed_genomes() {
        offer_diverse(&mut archive, genome, 0, "control_seed", &mut next_id, seed);
    }
    for generation in 1..=generations {
        let keys: Vec<_> = archive.keys().cloned().collect();
        let parent = archive[&keys[rng.below(keys.len())]].genome.clone();
        let (child, operator) = if archive.len() > 1 && rng.below(3) == 0 {
            let other = archive[&keys[rng.below(keys.len())]].genome.clone();
            let crossed = parent.crossover(&other, &mut rng);
            (crossed.mutate_named(&mut rng).0, "mac_crossover")
        } else {
            parent.mutate_named(&mut rng)
        };
        offer_diverse(
            &mut archive,
            child,
            generation,
            operator,
            &mut next_id,
            seed ^ generation as u64,
        );
        if generation % 1_000 == 0 {
            log(&format!(
                "generation {generation}: {} exact MAC diversity niches",
                archive.len()
            ));
        }
    }
    archive.into_values().collect()
}

fn diversity_niche(genome: &MacGenome, net8: &Netlist, net16: &Netlist) -> String {
    let counts8 = net8.counts();
    let counts16 = net16.counts();
    format!(
        "f{}-ac{:?}-d{:?}-p{:?}-c{}-a{:?}-fa{:?}-r{}-sr{:?}-zr{:?}-rc{:?}-si{:?}-sh{}-d8b{}-g8b{}-d16b{}-g16b{}",
        usize::from(genome.fuse_accumulator),
        genome.accumulator_carry,
        genome.arithmetic.default_action,
        genome.arithmetic.default_pick,
        usize::from(genome.arithmetic.default_carry_same_stage),
        genome.arithmetic.adder,
        genome.arithmetic.fa_impl,
        genome.arithmetic.rules.len(),
        genome.requant.sticky_reduction,
        genome.requant.saturation_reduction,
        genome.requant.round_carry,
        genome.requant.saturation,
        usize::from(genome.requant.share_saturation_predicate),
        counts8.gate_depth / 4,
        counts8.simple_gates / 32,
        counts16.gate_depth / 4,
        counts16.simple_gates / 64,
    )
}

fn offer_diverse(
    archive: &mut BTreeMap<String, MacElite>,
    genome: MacGenome,
    generation: usize,
    operator: &'static str,
    next_id: &mut usize,
    seed: u64,
) {
    if archive.values().any(|elite| elite.genome == genome) {
        return;
    }
    let net8 = develop_mac(&genome, 8);
    let net16 = develop_mac(&genome, 16);
    let counts8 = net8.counts();
    let counts = net16.counts();
    let niche = diversity_niche(&genome, &net8, &net16);
    let quality = (
        counts8.gate_depth * counts8.simple_gates + counts.gate_depth * counts.simple_gates,
        counts.gate_depth,
        counts.simple_gates,
    );
    if archive.get(&niche).is_some_and(|elite| {
        let incumbent8 = develop_mac(&elite.genome, 8).counts();
        let incumbent = (
            incumbent8.gate_depth * incumbent8.simple_gates + elite.depth16 * elite.gates16,
            elite.depth16,
            elite.gates16,
        );
        incumbent <= quality
    }) {
        return;
    }
    // Compressor development conserves the weighted sum by construction.
    // During exploration, sample the wider siblings and reserve exhaustive
    // W=4 verification for every retained elite at emission time.
    if verify_mac(&develop_mac(&genome, 8), 512, seed ^ 8).is_err()
        || verify_mac(&net16, 512, seed ^ 16).is_err()
    {
        return;
    }
    archive.insert(
        niche,
        MacElite {
            id: *next_id,
            genome,
            depth16: counts.gate_depth,
            gates16: counts.simple_gates,
            routing16: net16.routing_proxy().score,
            generation,
            operator,
        },
    );
    *next_id += 1;
}

fn evolve_mac_internal(
    generations: usize,
    seed: u64,
    allow_fusion: bool,
    mut log: impl FnMut(&str),
) -> Vec<MacElite> {
    let mut archive = Vec::new();
    let mut next_id = 0;
    let mut rng = Rng::new(seed);
    for genome in seed_genomes()
        .into_iter()
        .filter(|genome| allow_fusion || !genome.fuse_accumulator)
    {
        offer(&mut archive, genome, 0, "control_seed", &mut next_id, seed);
    }
    for generation in 1..=generations {
        let parent = archive[rng.below(archive.len())].genome.clone();
        let (mut child, operator) = if archive.len() > 1 && rng.below(3) == 0 {
            let other = archive[rng.below(archive.len())].genome.clone();
            let crossed = parent.crossover(&other, &mut rng);
            (crossed.mutate_named(&mut rng).0, "mac_crossover")
        } else {
            parent.mutate_named(&mut rng)
        };
        if !allow_fusion {
            child.fuse_accumulator = false;
        }
        offer(
            &mut archive,
            child,
            generation,
            operator,
            &mut next_id,
            seed ^ generation as u64,
        );
        if generation % 1_000 == 0 {
            log(&format!(
                "generation {generation}: {} exact MAC Pareto elites",
                archive.len()
            ));
        }
    }
    archive
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::Adder;

    #[test]
    fn frozen_genome_replays_and_rejects_invalid_policies() {
        let json = include_str!("../results/discovery/mac_diverse_pilot_20260905/mac_evo477.json");
        let genome = MacGenome::from_json(json).unwrap();
        let mut net = develop_mac(&genome, 8);
        net.label = "mac_requant_evo477".into();
        assert_eq!(
            net.to_verilog(),
            include_str!(
                "../results/discovery/mac_diverse_pilot_20260905/mul8_mac_requant_evo477.v"
            )
        );
        assert!(MacGenome::from_json(&json.replace("Balanced", "invalid")).is_err());
        assert!(MacGenome::from_json(&json.replace("\"drop16\":0", "\"drop16\":1")).is_err());
        assert!(MacGenome::from_json("{}").is_err());
    }

    #[test]
    fn wide_mac_verifies_high_roots_and_detects_corruption() {
        let genome = seed_genomes().remove(0);
        for width in [24, 32] {
            let mut net = develop_mac(&genome, width);
            verify_mac(&net, 128, 47).unwrap();
            let expected_status_bit = 3 * width + 2;
            assert!(net.eval_with_aux_u128(1, 1, 0) & (1u128 << expected_status_bit) != 0);
            net.outputs[expected_status_bit] = Wire::Const0;
            assert!(verify_mac(&net, 0, 47).is_err());
        }
    }

    #[test]
    fn ripple_and_prefix_mac_are_exhaustively_exact_at_four_bits() {
        for accumulator_carry in [AccumulatorCarry::Ripple, AccumulatorCarry::Prefix] {
            for fuse_accumulator in [false, true] {
                let genome = MacGenome {
                    arithmetic: Genome::dadda(Adder::BrentKung),
                    requant: RequantPlan::default(),
                    accumulator_carry,
                    fuse_accumulator,
                };
                verify_mac(&develop_mac(&genome, 4), 0, 7).unwrap();
            }
        }
    }

    #[test]
    fn mutation_preserves_the_mac_contract() {
        let mut rng = Rng::new(19);
        let mut genome = seed_genomes().remove(0);
        for _ in 0..12 {
            genome = genome.mutate_named(&mut rng).0;
            verify_mac(&develop_mac(&genome, 4), 0, 23).unwrap();
        }
    }

    #[test]
    fn diversity_search_retains_multiple_niches_without_routing_selection() {
        let archive = evolve_mac_diverse(32, 29, |_| {});
        assert!(archive.len() > 1);
        for elite in archive {
            verify_mac(&develop_mac(&elite.genome, 4), 0, 31).unwrap();
        }
    }
}
