//! Developmental multiplier genomes, a column-state compressor engine that is
//! arithmetically correct by construction, aligned (homologous) crossover,
//! and a MAP-Elites archive.
//!
//! The genome is not a circuit. It is a small rule program that grows a
//! circuit: at every reduction stage, every weighted column looks at its
//! height, its position and the stage number, finds the first matching rule,
//! and applies that rule's compression motif to its bits in the rule's chosen
//! order. The same program develops a 4-, 8-, 16- or 32-bit multiplier. A
//! mutation edits a rule and therefore changes many columns coherently.
//!
//! Every compressor consumes bits of one weight and emits bits of that weight
//! and the next, so the weighted sum is conserved at every step and every
//! developed circuit computes the product. Correctness is nevertheless
//! re-checked exhaustively for every child (to 8 bits) and by SAT for the
//! elites, because "by construction" is a claim about the engine and the
//! evaluator is the judge.

use crate::gen::{final_prefix, final_ripple, split_rows, Adder, Netlist, Reduction, Wire, ADDERS};
use crate::Rng;
use std::collections::BTreeMap;
use std::fmt::Write as _;

/// How a column orders its bits before feeding compressors.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Pick {
    /// Earliest-arriving bits first (delay-aware, in the spirit of
    /// three-greedy compressor scheduling).
    Earliest,
    /// Latest-arriving bits first (keeps early bits for the final adder).
    Latest,
    /// Positional order as generated.
    Positional,
    /// Alternate ends.
    Alternate,
}

pub const PICKS: [Pick; 4] = [
    Pick::Earliest,
    Pick::Latest,
    Pick::Positional,
    Pick::Alternate,
];

/// Compression motif applied to one column at one stage.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Action {
    /// As many full adders as possible, then a half adder on a remainder of
    /// two (Wallace).
    Full,
    /// As many full adders as possible, remainder passes (no half adders).
    FullNoHa,
    /// Only enough adders to reach the next Dadda height.
    Dadda,
    /// One half adder, rest passes.
    HalfOne,
    /// 4:2 compressors (two chained full adders with a horizontal carry into
    /// the next column at the same stage), remainder passes.
    C42,
    /// Do nothing this stage.
    Defer,
    /// (5;3) counters: five bits of one weight into one bit each at weights
    /// w, w+1, w+2 (two full adders and a half adder), remainder passes.
    Counter53,
    /// (7;3) counters: seven bits into w, w+1, w+2 (four full adders).
    Counter73,
}

pub const ACTIONS: [Action; 8] = [
    Action::Full,
    Action::FullNoHa,
    Action::Dadda,
    Action::HalfOne,
    Action::C42,
    Action::Defer,
    Action::Counter53,
    Action::Counter73,
];

/// Physical realization of the full-adder relation.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FaImpl {
    /// XOR3 sum, majority carry.
    XorMaj,
    /// XOR sum, mux carry.
    Mux,
}

pub const FA_IMPLS: [FaImpl; 2] = [FaImpl::XorMaj, FaImpl::Mux];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Rule {
    /// Column height range (inclusive) the rule applies to.
    pub h_lo: u8,
    pub h_hi: u8,
    /// Column position range in sixteenths of the full 2W width (inclusive).
    pub col_lo: u8,
    pub col_hi: u8,
    /// Stage range (inclusive).
    pub stage_lo: u8,
    pub stage_hi: u8,
    pub action: Action,
    pub pick: Pick,
    /// Carries out of this column's full adders enter the next column in the
    /// same stage (array style) rather than the next stage (tree style).
    pub carry_same_stage: bool,
    /// Offset applied to the Dadda target height for this rule.
    pub threshold: i8,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Genome {
    pub rules: Vec<Rule>,
    /// Applied when no rule matches.
    pub default_action: Action,
    pub default_pick: Pick,
    pub default_carry_same_stage: bool,
    pub adder: Adder,
    pub fa_impl: FaImpl,
    /// Columns below this many sixteenths of the full 2W width are not
    /// generated at all. Zero means an exact multiplier; anything else is an
    /// approximate multiplier that belongs in the approximate archive.
    pub drop16: u8,
}

impl Genome {
    pub const MAX_RULES: usize = 8;

    /// The classic constructions as genomes, used as seeds and as a check
    /// that the engine reproduces the named generators.
    pub fn array(adder: Adder) -> Self {
        Genome {
            rules: Vec::new(),
            default_action: Action::Full,
            default_pick: Pick::Positional,
            default_carry_same_stage: true,
            adder,
            fa_impl: FaImpl::XorMaj,
            drop16: 0,
        }
    }

    pub fn wallace(adder: Adder) -> Self {
        Genome {
            rules: Vec::new(),
            default_action: Action::Full,
            default_pick: Pick::Positional,
            default_carry_same_stage: false,
            adder,
            fa_impl: FaImpl::XorMaj,
            drop16: 0,
        }
    }

    pub fn dadda(adder: Adder) -> Self {
        Genome {
            rules: Vec::new(),
            default_action: Action::Dadda,
            default_pick: Pick::Positional,
            default_carry_same_stage: false,
            adder,
            fa_impl: FaImpl::XorMaj,
            drop16: 0,
        }
    }

    pub fn random(rng: &mut Rng) -> Self {
        let mut genome = Genome {
            rules: Vec::new(),
            default_action: ACTIONS[rng.below(ACTIONS.len() - 1)], // never Defer by default
            default_pick: PICKS[rng.below(PICKS.len())],
            default_carry_same_stage: rng.below(4) == 0,
            adder: ADDERS[rng.below(ADDERS.len())],
            fa_impl: FA_IMPLS[rng.below(FA_IMPLS.len())],
            drop16: 0,
        };
        for _ in 0..rng.below(4) {
            genome.rules.push(random_rule(rng));
        }
        genome
    }

    /// Small and large mutations. The name of the operator is returned for
    /// the lineage record.
    pub fn mutate_named(&self, rng: &mut Rng) -> (Self, &'static str) {
        let mut child = self.clone();
        let op = match rng.below(16) {
            0 => {
                child.default_action = ACTIONS[rng.below(ACTIONS.len())];
                if child.default_action == Action::Defer {
                    child.default_action = Action::Dadda;
                }
                "default_action"
            }
            1 => {
                child.default_pick = PICKS[rng.below(PICKS.len())];
                "default_pick"
            }
            2 => {
                child.default_carry_same_stage = !child.default_carry_same_stage;
                "default_carry"
            }
            3 => {
                child.adder = ADDERS[rng.below(ADDERS.len())];
                "adder"
            }
            4 if child.rules.len() < Self::MAX_RULES => {
                let at = rng.below(child.rules.len() + 1);
                child.rules.insert(at, random_rule(rng));
                "rule_insert"
            }
            5 if !child.rules.is_empty() => {
                let at = rng.below(child.rules.len());
                child.rules.remove(at);
                "rule_remove"
            }
            6 if child.rules.len() >= 2 => {
                let a = rng.below(child.rules.len());
                let b = rng.below(child.rules.len());
                child.rules.swap(a, b);
                "rule_swap"
            }
            7 => {
                child.fa_impl = FA_IMPLS[rng.below(FA_IMPLS.len())];
                "fa_impl"
            }
            // Large operators.
            8 => {
                // Replace every rule touching a column region with one fresh
                // rule that governs exactly that region.
                let lo = rng.below(16) as u8;
                let hi = (lo + 1 + rng.below(8) as u8).min(16);
                child.rules.retain(|r| r.col_hi <= lo || r.col_lo >= hi);
                let mut fresh = random_rule(rng);
                fresh.col_lo = lo;
                fresh.col_hi = hi;
                fresh.h_lo = 2;
                fresh.h_hi = 40;
                if child.rules.len() < Self::MAX_RULES {
                    child.rules.insert(0, fresh);
                }
                "region_replace"
            }
            9 => {
                // Replace the whole default reduction policy.
                let policy = rng.below(3);
                let base = match policy {
                    0 => Genome::array(child.adder),
                    1 => Genome::wallace(child.adder),
                    _ => Genome::dadda(child.adder),
                };
                child.default_action = base.default_action;
                child.default_pick = PICKS[rng.below(PICKS.len())];
                child.default_carry_same_stage = base.default_carry_same_stage;
                "policy_replace"
            }
            10 => {
                // Shift the compression threshold of a whole region.
                let delta: i8 = if rng.below(2) == 0 { -1 } else { 1 };
                let lo = rng.below(16) as u8;
                for r in child.rules.iter_mut() {
                    if r.col_lo >= lo {
                        r.threshold = (r.threshold + delta).clamp(-4, 4);
                    }
                }
                if child.rules.is_empty() || rng.below(3) == 0 {
                    let mut fresh = random_rule(rng);
                    fresh.action = Action::Dadda;
                    fresh.threshold = delta;
                    fresh.col_lo = lo;
                    fresh.col_hi = 16;
                    fresh.h_lo = 3;
                    fresh.h_hi = 40;
                    fresh.stage_lo = 0;
                    fresh.stage_hi = 20;
                    if child.rules.len() < Self::MAX_RULES {
                        child.rules.push(fresh);
                    }
                }
                "threshold_shift"
            }
            11 => {
                // Insert a deferral stage: everything at stage k in a region
                // waits, and later rules move one stage later.
                let k = rng.below(4) as u8;
                for r in child.rules.iter_mut() {
                    if r.stage_lo >= k {
                        r.stage_lo = (r.stage_lo + 1).min(20);
                        r.stage_hi = (r.stage_hi + 1).min(20);
                    }
                }
                let mut fresh = random_rule(rng);
                fresh.action = Action::Defer;
                fresh.stage_lo = k;
                fresh.stage_hi = k;
                fresh.h_lo = 3;
                fresh.h_hi = 40;
                child.rules.insert(0, fresh);
                child.rules.truncate(Self::MAX_RULES);
                "stage_insert"
            }
            12 if !child.rules.is_empty() => {
                // Remove a stage: rules at stage k are dropped, later ones move up.
                let k = rng.below(4) as u8;
                child
                    .rules
                    .retain(|r| !(r.stage_lo == k && r.stage_hi == k));
                for r in child.rules.iter_mut() {
                    if r.stage_lo > k {
                        r.stage_lo -= 1;
                        r.stage_hi = r.stage_hi.max(r.stage_lo);
                    }
                }
                "stage_remove"
            }
            13 if !child.rules.is_empty() && child.rules.len() < Self::MAX_RULES => {
                // Duplication with divergence: copy a rule to a shifted region.
                let at = rng.below(child.rules.len());
                let mut copy = child.rules[at];
                let span = copy.col_hi - copy.col_lo;
                let shift = rng.below(16) as u8;
                copy.col_lo = shift.min(15);
                copy.col_hi = (copy.col_lo + span.max(1)).min(16);
                copy = mutate_rule(copy, rng);
                child.rules.push(copy);
                "duplicate_shift"
            }
            14 => {
                // Enter or leave the approximate archive by dropping low columns.
                child.drop16 = if child.drop16 == 0 || rng.below(3) == 0 {
                    1 + rng.below(7) as u8
                } else {
                    0
                };
                "drop_columns"
            }
            _ if !child.rules.is_empty() => {
                let at = rng.below(child.rules.len());
                child.rules[at] = mutate_rule(child.rules[at], rng);
                "rule_param"
            }
            _ => {
                child.rules.push(random_rule(rng));
                "rule_insert"
            }
        };
        (child, op)
    }

    pub fn mutate(&self, rng: &mut Rng) -> Self {
        self.mutate_named(rng).0
    }

    /// Homologous crossover: rules are aligned by the column region they
    /// govern, and the child takes each region's rules from one parent.
    pub fn crossover(&self, other: &Self, rng: &mut Rng) -> Self {
        let cut = rng.below(17) as u8; // column sixteenth
        let mut rules: Vec<Rule> = Vec::new();
        for rule in &self.rules {
            if rule.col_lo < cut {
                rules.push(*rule);
            }
        }
        for rule in &other.rules {
            if rule.col_lo >= cut {
                rules.push(*rule);
            }
        }
        rules.truncate(Self::MAX_RULES);
        let take_other = rng.below(2) == 0;
        let base = if take_other { other } else { self };
        Genome {
            rules,
            default_action: base.default_action,
            default_pick: base.default_pick,
            default_carry_same_stage: base.default_carry_same_stage,
            adder: if rng.below(2) == 0 {
                self.adder
            } else {
                other.adder
            },
            fa_impl: base.fa_impl,
            drop16: base.drop16,
        }
    }

    pub fn to_json(&self) -> String {
        let rules: Vec<String> = self
            .rules
            .iter()
            .map(|r| {
                format!(
                    "{{\"h\":[{},{}],\"col16\":[{},{}],\"stage\":[{},{}],\"action\":\"{:?}\",\"pick\":\"{:?}\",\"carry_same_stage\":{},\"threshold\":{}}}",
                    r.h_lo, r.h_hi, r.col_lo, r.col_hi, r.stage_lo, r.stage_hi, r.action, r.pick, r.carry_same_stage, r.threshold
                )
            })
            .collect();
        format!(
            "{{\"rules\":[{}],\"default_action\":\"{:?}\",\"default_pick\":\"{:?}\",\"default_carry_same_stage\":{},\"adder\":\"{}\",\"fa_impl\":\"{:?}\",\"drop16\":{}}}",
            rules.join(","),
            self.default_action,
            self.default_pick,
            self.default_carry_same_stage,
            self.adder.name(),
            self.fa_impl,
            self.drop16
        )
    }

    pub fn from_json(text: &str) -> Result<Self, String> {
        let value: serde_json::Value = serde_json::from_str(text).map_err(|e| e.to_string())?;
        let string = |key: &str| -> Result<&str, String> {
            value[key]
                .as_str()
                .ok_or_else(|| format!("missing string {key}"))
        };
        let action = |name: &str| match name {
            "Full" => Ok(Action::Full),
            "FullNoHa" => Ok(Action::FullNoHa),
            "Dadda" => Ok(Action::Dadda),
            "HalfOne" => Ok(Action::HalfOne),
            "C42" => Ok(Action::C42),
            "Defer" => Ok(Action::Defer),
            "Counter53" => Ok(Action::Counter53),
            "Counter73" => Ok(Action::Counter73),
            _ => Err(format!("unknown action {name}")),
        };
        let pick = |name: &str| match name {
            "Earliest" => Ok(Pick::Earliest),
            "Latest" => Ok(Pick::Latest),
            "Positional" => Ok(Pick::Positional),
            "Alternate" => Ok(Pick::Alternate),
            _ => Err(format!("unknown pick {name}")),
        };
        let adder = match string("adder")? {
            "ripple" => Adder::Ripple,
            "koggestone" => Adder::KoggeStone,
            "sklansky" => Adder::Sklansky,
            "brentkung" => Adder::BrentKung,
            "hancarlson" => Adder::HanCarlson,
            name => return Err(format!("unknown adder {name}")),
        };
        let fa_impl = match string("fa_impl")? {
            "XorMaj" => FaImpl::XorMaj,
            "Mux" => FaImpl::Mux,
            name => return Err(format!("unknown full-adder implementation {name}")),
        };
        let mut rules = Vec::new();
        for item in value["rules"]
            .as_array()
            .ok_or_else(|| "missing rules array".to_string())?
        {
            let pair = |key: &str| -> Result<(u8, u8), String> {
                let xs = item[key]
                    .as_array()
                    .ok_or_else(|| format!("missing {key} pair"))?;
                if xs.len() != 2 {
                    return Err(format!("{key} must contain two values"));
                }
                Ok((
                    xs[0].as_u64().ok_or_else(|| format!("invalid {key}"))? as u8,
                    xs[1].as_u64().ok_or_else(|| format!("invalid {key}"))? as u8,
                ))
            };
            let (h_lo, h_hi) = pair("h")?;
            let (col_lo, col_hi) = pair("col16")?;
            let (stage_lo, stage_hi) = pair("stage")?;
            rules.push(Rule {
                h_lo,
                h_hi,
                col_lo,
                col_hi,
                stage_lo,
                stage_hi,
                action: action(
                    item["action"]
                        .as_str()
                        .ok_or_else(|| "missing rule action".to_string())?,
                )?,
                pick: pick(
                    item["pick"]
                        .as_str()
                        .ok_or_else(|| "missing rule pick".to_string())?,
                )?,
                carry_same_stage: item["carry_same_stage"]
                    .as_bool()
                    .ok_or_else(|| "missing rule carry_same_stage".to_string())?,
                threshold: item["threshold"]
                    .as_i64()
                    .ok_or_else(|| "missing rule threshold".to_string())?
                    as i8,
            });
        }
        Ok(Genome {
            rules,
            default_action: action(string("default_action")?)?,
            default_pick: pick(string("default_pick")?)?,
            default_carry_same_stage: value["default_carry_same_stage"]
                .as_bool()
                .ok_or_else(|| "missing default_carry_same_stage".to_string())?,
            adder,
            fa_impl,
            drop16: value["drop16"]
                .as_u64()
                .ok_or_else(|| "missing drop16".to_string())? as u8,
        })
    }
}

fn random_rule(rng: &mut Rng) -> Rule {
    let h_lo = 2 + rng.below(6) as u8;
    let col_lo = rng.below(16) as u8;
    let stage_lo = rng.below(4) as u8;
    Rule {
        h_lo,
        h_hi: h_lo + rng.below(8) as u8,
        col_lo,
        col_hi: (col_lo + 1 + rng.below(16) as u8).min(16),
        stage_lo,
        stage_hi: stage_lo + rng.below(8) as u8,
        action: ACTIONS[rng.below(ACTIONS.len())],
        pick: PICKS[rng.below(PICKS.len())],
        carry_same_stage: rng.below(3) == 0,
        threshold: 0,
    }
}

fn mutate_rule(rule: Rule, rng: &mut Rng) -> Rule {
    let mut r = rule;
    match rng.below(10) {
        0 => r.h_lo = (r.h_lo as i32 + rng.below(3) as i32 - 1).clamp(2, 12) as u8,
        1 => r.h_hi = (r.h_hi as i32 + rng.below(3) as i32 - 1).clamp(2, 40) as u8,
        2 => r.col_lo = (r.col_lo as i32 + rng.below(5) as i32 - 2).clamp(0, 15) as u8,
        3 => r.col_hi = (r.col_hi as i32 + rng.below(5) as i32 - 2).clamp(1, 16) as u8,
        4 => r.stage_lo = (r.stage_lo as i32 + rng.below(3) as i32 - 1).clamp(0, 12) as u8,
        5 => r.stage_hi = (r.stage_hi as i32 + rng.below(3) as i32 - 1).clamp(0, 20) as u8,
        6 => r.action = ACTIONS[rng.below(ACTIONS.len())],
        7 => r.pick = PICKS[rng.below(PICKS.len())],
        8 => r.threshold = (r.threshold + if rng.below(2) == 0 { -1 } else { 1 }).clamp(-4, 4),
        _ => r.carry_same_stage = !r.carry_same_stage,
    }
    if r.h_hi < r.h_lo {
        r.h_hi = r.h_lo;
    }
    if r.col_hi <= r.col_lo {
        r.col_hi = r.col_lo + 1;
    }
    if r.stage_hi < r.stage_lo {
        r.stage_hi = r.stage_lo;
    }
    r
}

#[derive(Clone, Copy, Debug)]
struct Bit {
    wire: Wire,
    /// Estimated arrival in the simple-gate depth model used by `counts()`.
    arrival: u32,
}

/// Structural descriptors of a developed circuit, used as MAP-Elites niches.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Descriptors {
    pub stages: usize,
    pub full_adders: usize,
    pub half_adders: usize,
    pub c42: usize,
    pub counters: usize,
    /// Full adders whose carry entered the same stage.
    pub same_stage_carries: usize,
    pub forced_finish: bool,
    pub adder: Adder,
    pub fa_impl: FaImpl,
    pub dropped_columns: usize,
}

/// Grow a multiplier from a genome. Always returns a netlist; `forced_finish`
/// in the descriptors says whether the rules failed to terminate within the
/// stage cap and a Dadda finish was appended.
pub fn develop(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    develop_internal(genome, width, false, false, false, false, false)
}

fn develop_internal(
    genome: &Genome,
    width: usize,
    inject_rounding_bit: bool,
    fuse_saturation: bool,
    expose_saturation_low: bool,
    expose_saturation_overflow: bool,
    inject_accumulator: bool,
) -> (Netlist, Descriptors) {
    let mut net = Netlist::new(width, Reduction::Dadda, genome.adder);
    let columns = 2 * width + usize::from(inject_accumulator);
    let dropped = (genome.drop16 as usize * columns) / 16;
    let mut cols: Vec<Vec<Bit>> = vec![Vec::new(); columns];
    let mut direct_overflow = Vec::new();
    for i in 0..width {
        for j in 0..width {
            if i + j < dropped {
                continue;
            }
            let wire = net.and2(Wire::A(j), Wire::B(i));
            if fuse_saturation && i + j >= width {
                // Unsigned partial products cannot cancel. Any asserted term
                // already at weight W or above proves saturation, so do not
                // spend a compressor tree constructing its exact value.
                direct_overflow.push(wire);
            } else {
                cols[i + j].push(Bit { wire, arrival: 1 });
            }
        }
    }
    if inject_accumulator {
        net.aux_width = 2 * width;
        for (column, bits) in cols.iter_mut().enumerate().take(2 * width) {
            bits.push(Bit {
                wire: Wire::Aux(column),
                arrival: 0,
            });
        }
    }
    if inject_rounding_bit {
        cols[width - 1].push(Bit {
            wire: Wire::Const1,
            arrival: 0,
        });
    }
    let mut desc = Descriptors {
        stages: 0,
        full_adders: 0,
        half_adders: 0,
        c42: 0,
        counters: 0,
        same_stage_carries: 0,
        forced_finish: false,
        adder: genome.adder,
        fa_impl: genome.fa_impl,
        dropped_columns: dropped,
    };
    let fa_impl = genome.fa_impl;
    let fa_cell = |net: &mut Netlist, a: Wire, b: Wire, c: Wire| -> (Wire, Wire) {
        match fa_impl {
            FaImpl::XorMaj => net.fa(a, b, c),
            FaImpl::Mux => net.fa_mux(a, b, c),
        }
    };

    let max_height = cols.iter().map(Vec::len).max().unwrap_or(0);
    let mut dadda_heights = vec![2usize];
    while *dadda_heights.last().unwrap() < max_height {
        let last = *dadda_heights.last().unwrap();
        dadda_heights.push(last * 3 / 2);
    }

    let stage_cap = 6 * width;
    let mut stage = 0usize;
    let mut progress_guard = 0usize;
    while cols.iter().any(|c| c.len() > 2) && stage < stage_cap {
        let current_max = cols.iter().map(Vec::len).max().unwrap_or(0);
        let dadda_target = dadda_heights
            .iter()
            .rev()
            .find(|&&h| h < current_max)
            .copied()
            .unwrap_or(2);
        let mut next: Vec<Vec<Bit>> = vec![Vec::new(); columns];
        let mut same: Vec<Vec<Bit>> = vec![Vec::new(); columns];
        let mut c42_cin: Vec<Vec<Bit>> = vec![Vec::new(); columns];
        let mut any_adder = false;
        for c in 0..columns {
            let mut bits: Vec<Bit> = std::mem::take(&mut cols[c]);
            bits.extend(std::mem::take(&mut same[c]));
            // Horizontal 4:2 carry-in offered by the previous column in this
            // stage. It is used horizontally only by a 4:2 here; otherwise it
            // is an ordinary bit of this column.
            let mut cins: Vec<Bit> = std::mem::take(&mut c42_cin[c]);
            let height = bits.len() + cins.len();
            if height <= 2 {
                bits.append(&mut cins);
                next[c].extend(bits);
                continue;
            }
            let sixteenth = ((c * 16) / columns) as u8;
            let rule = genome.rules.iter().find(|r| {
                (r.h_lo as usize) <= height
                    && height <= r.h_hi as usize
                    && r.col_lo <= sixteenth
                    && sixteenth < r.col_hi
                    && (r.stage_lo as usize) <= stage
                    && stage <= r.stage_hi as usize
            });
            let (action, pick, same_stage, threshold) = match rule {
                Some(r) => (r.action, r.pick, r.carry_same_stage, r.threshold),
                None => (
                    genome.default_action,
                    genome.default_pick,
                    genome.default_carry_same_stage,
                    0,
                ),
            };
            if action != Action::C42 {
                bits.append(&mut cins);
            }
            order_bits(&mut bits, pick);
            let mut queue: std::collections::VecDeque<Bit> = bits.into_iter().collect();

            let target = match action {
                Action::Full | Action::FullNoHa | Action::C42 => 2usize,
                Action::Counter53 | Action::Counter73 => 2usize,
                Action::Dadda => (dadda_target as i64 + threshold as i64).max(2) as usize,
                Action::HalfOne | Action::Defer => usize::MAX,
            };
            loop {
                let live = queue.len() + next[c].len() + cins.len();
                if live <= target && action != Action::HalfOne {
                    break;
                }
                if action == Action::Defer {
                    break;
                }
                if action == Action::HalfOne {
                    if queue.len() >= 2 {
                        let a = queue.pop_front().unwrap();
                        let b = queue.pop_front().unwrap();
                        let (s, co) = net.ha(a.wire, b.wire);
                        let arrival = a.arrival.max(b.arrival) + 2;
                        next[c].push(Bit { wire: s, arrival });
                        push_carry(
                            &mut next,
                            &mut same,
                            c,
                            Bit { wire: co, arrival },
                            same_stage,
                            &mut desc,
                        );
                        desc.half_adders += 1;
                        any_adder = true;
                    }
                    break;
                }
                if action == Action::Counter73 && queue.len() >= 7 {
                    let x: Vec<Bit> = (0..7).map(|_| queue.pop_front().unwrap()).collect();
                    let (s1, c1) = fa_cell(&mut net, x[0].wire, x[1].wire, x[2].wire);
                    let (s2, c2) = fa_cell(&mut net, x[3].wire, x[4].wire, x[5].wire);
                    let a1 = x.iter().map(|b| b.arrival).max().unwrap() + 3;
                    let (sum, c3) = fa_cell(&mut net, s1, s2, x[6].wire);
                    let (w1, w2) = fa_cell(&mut net, c1, c2, c3);
                    next[c].push(Bit {
                        wire: sum,
                        arrival: a1 + 3,
                    });
                    if c + 1 < columns {
                        next[c + 1].push(Bit {
                            wire: w1,
                            arrival: a1 + 3,
                        });
                    }
                    if c + 2 < columns {
                        next[c + 2].push(Bit {
                            wire: w2,
                            arrival: a1 + 3,
                        });
                    }
                    desc.full_adders += 4;
                    desc.counters += 1;
                    any_adder = true;
                    continue;
                }
                if action == Action::Counter53 && queue.len() >= 5 {
                    let x: Vec<Bit> = (0..5).map(|_| queue.pop_front().unwrap()).collect();
                    let (s1, c1) = fa_cell(&mut net, x[0].wire, x[1].wire, x[2].wire);
                    let a1 = x.iter().map(|b| b.arrival).max().unwrap() + 3;
                    let (sum, c2) = fa_cell(&mut net, s1, x[3].wire, x[4].wire);
                    let (w1, w2) = net.ha(c1, c2);
                    next[c].push(Bit {
                        wire: sum,
                        arrival: a1 + 3,
                    });
                    if c + 1 < columns {
                        next[c + 1].push(Bit {
                            wire: w1,
                            arrival: a1 + 3,
                        });
                    }
                    if c + 2 < columns {
                        next[c + 2].push(Bit {
                            wire: w2,
                            arrival: a1 + 3,
                        });
                    }
                    desc.full_adders += 2;
                    desc.half_adders += 1;
                    desc.counters += 1;
                    any_adder = true;
                    continue;
                }
                if action == Action::C42 && queue.len() >= 4 {
                    let x1 = queue.pop_front().unwrap();
                    let x2 = queue.pop_front().unwrap();
                    let x3 = queue.pop_front().unwrap();
                    let x4 = queue.pop_front().unwrap();
                    let (s1, cout) = fa_cell(&mut net, x1.wire, x2.wire, x3.wire);
                    let a1 = x1.arrival.max(x2.arrival).max(x3.arrival) + 3;
                    let cin_bit = cins.pop().unwrap_or(Bit {
                        wire: Wire::Const0,
                        arrival: 0,
                    });
                    let (s, carry) = if cin_bit.wire == Wire::Const0 {
                        let (s, carry) = net.ha(s1, x4.wire);
                        desc.half_adders += 1;
                        (s, carry)
                    } else {
                        let (s, carry) = fa_cell(&mut net, s1, x4.wire, cin_bit.wire);
                        desc.full_adders += 1;
                        (s, carry)
                    };
                    let a2 = a1.max(x4.arrival).max(cin_bit.arrival) + 3;
                    next[c].push(Bit {
                        wire: s,
                        arrival: a2,
                    });
                    if c + 1 < columns {
                        next[c + 1].push(Bit {
                            wire: carry,
                            arrival: a2,
                        });
                        // Horizontal carry: offered to the next column's 4:2 in
                        // this stage, otherwise it lands in the next stage.
                        c42_cin[c + 1].push(Bit {
                            wire: cout,
                            arrival: a1,
                        });
                    }
                    desc.full_adders += 1;
                    desc.c42 += 1;
                    any_adder = true;
                    continue;
                }
                let excess = live.saturating_sub(target);
                if queue.len() >= 3 && (excess >= 2 || action != Action::Dadda) {
                    let a = queue.pop_front().unwrap();
                    let b = queue.pop_front().unwrap();
                    let d = queue.pop_front().unwrap();
                    let (s, co) = fa_cell(&mut net, a.wire, b.wire, d.wire);
                    let arrival = a.arrival.max(b.arrival).max(d.arrival) + 3;
                    next[c].push(Bit { wire: s, arrival });
                    push_carry(
                        &mut next,
                        &mut same,
                        c,
                        Bit { wire: co, arrival },
                        same_stage,
                        &mut desc,
                    );
                    desc.full_adders += 1;
                    any_adder = true;
                } else if queue.len() >= 2 && action != Action::FullNoHa {
                    let a = queue.pop_front().unwrap();
                    let b = queue.pop_front().unwrap();
                    let (s, co) = net.ha(a.wire, b.wire);
                    let arrival = a.arrival.max(b.arrival) + 2;
                    next[c].push(Bit { wire: s, arrival });
                    push_carry(
                        &mut next,
                        &mut same,
                        c,
                        Bit { wire: co, arrival },
                        same_stage,
                        &mut desc,
                    );
                    desc.half_adders += 1;
                    any_adder = true;
                } else {
                    break;
                }
            }
            // A horizontal carry-in that no 4:2 consumed is an ordinary bit
            // of this column in the next stage.
            next[c].append(&mut cins);
            next[c].extend(queue);
        }
        // Same-stage carries that reached a column already processed this
        // stage (column index above the last processed one is impossible, so
        // this only happens at the top column) fall through to the next stage.
        for c in 0..columns {
            next[c].extend(std::mem::take(&mut same[c]));
        }
        cols = next;
        stage += 1;
        desc.stages += 1;
        if !any_adder {
            progress_guard += 1;
            if progress_guard > 2 {
                break;
            }
        }
    }

    if cols.iter().any(|c| c.len() > 2) {
        // Rules did not finish; append a Dadda finish so the genome still
        // yields a valid multiplier, and record that it needed help.
        desc.forced_finish = true;
        let finish = Genome::dadda(genome.adder);
        while cols.iter().any(|c| c.len() > 2) {
            let current_max = cols.iter().map(Vec::len).max().unwrap_or(0);
            let target = dadda_heights
                .iter()
                .rev()
                .find(|&&h| h < current_max)
                .copied()
                .unwrap_or(2);
            let mut next: Vec<Vec<Bit>> = vec![Vec::new(); columns];
            for c in 0..columns {
                let mut bits = std::mem::take(&mut cols[c]);
                order_bits(&mut bits, finish.default_pick);
                let mut queue: std::collections::VecDeque<Bit> = bits.into_iter().collect();
                loop {
                    let live = queue.len() + next[c].len();
                    if live <= target {
                        break;
                    }
                    if live - target >= 2 && queue.len() >= 3 {
                        let a = queue.pop_front().unwrap();
                        let b = queue.pop_front().unwrap();
                        let d = queue.pop_front().unwrap();
                        let (s, co) = fa_cell(&mut net, a.wire, b.wire, d.wire);
                        let arrival = a.arrival.max(b.arrival).max(d.arrival) + 3;
                        next[c].push(Bit { wire: s, arrival });
                        if c + 1 < columns {
                            next[c + 1].push(Bit { wire: co, arrival });
                        }
                        desc.full_adders += 1;
                    } else if queue.len() >= 2 {
                        let a = queue.pop_front().unwrap();
                        let b = queue.pop_front().unwrap();
                        let (s, co) = net.ha(a.wire, b.wire);
                        let arrival = a.arrival.max(b.arrival) + 2;
                        next[c].push(Bit { wire: s, arrival });
                        if c + 1 < columns {
                            next[c + 1].push(Bit { wire: co, arrival });
                        }
                        desc.half_adders += 1;
                    } else {
                        break;
                    }
                }
                next[c].extend(queue);
            }
            cols = next;
            desc.stages += 1;
        }
    }

    let wire_cols: Vec<Vec<Wire>> = cols
        .into_iter()
        .map(|c| c.into_iter().map(|b| b.wire).collect())
        .collect();
    let (rows, carries) = split_rows(wire_cols);
    let outputs = match genome.adder {
        Adder::Ripple => final_ripple(&mut net, &rows, &carries),
        prefix => final_prefix(&mut net, &rows, &carries, prefix),
    };
    if fuse_saturation {
        // Lower-weight partial products can still carry across the W-bit
        // boundary. OR every resulting upper bit with the direct high terms;
        // together these are exactly the predicate a*b >= 2^W.
        direct_overflow.extend_from_slice(&outputs[width..]);
        let mut level = direct_overflow;
        while level.len() > 1 {
            let mut next = Vec::with_capacity(level.len().div_ceil(2));
            for pair in level.chunks(2) {
                next.push(if pair.len() == 2 {
                    net.or2(pair[0], pair[1])
                } else {
                    pair[0]
                });
            }
            level = next;
        }
        let overflow = level.first().copied().unwrap_or(Wire::Const0);
        let low = outputs[..width].to_vec();
        let saturated: Vec<Wire> = low
            .iter()
            .map(|&bit| net.mux2(overflow, bit, Wire::Const1))
            .collect();
        net.outputs = saturated;
        if expose_saturation_low {
            net.outputs.extend(low);
        }
        if expose_saturation_overflow {
            net.outputs.push(overflow);
        }
    } else {
        net.outputs = outputs;
    }
    net.prune();
    (net, desc)
}

/// Develop a true fused multiply-accumulate tree: accumulator bits enter the
/// same weighted columns as partial products before compressor scheduling.
pub fn develop_fused_mac(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    develop_internal(genome, width, false, false, false, false, true)
}

/// Develop an exact wrapping `W x W -> W` multiplier.
///
/// The low half of a product is independent of all carries and partial
/// products at weights `W` and above. Truncating the observable outputs and
/// pruning therefore removes the entire upper cone, rather than paying for a
/// full product and selecting pins afterward. This is the baseline object for
/// specialized low-product search.
pub fn develop_low_product(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    let (mut net, desc) = develop(genome, width);
    net.outputs.truncate(width);
    net.label = "lo_specialized".to_string();
    net.prune();
    (net, desc)
}

/// Develop unsigned Q0.W multiplication rounded to the nearest W-bit Q0.W
/// result. The exact integer specification is `(a*b + 2^(W-1)) >> W`.
pub fn develop_rounded_fractional(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    let (mut net, desc) = develop_internal(genome, width, true, false, false, false, false);
    net.outputs = net.outputs[width..].to_vec();
    net.label = "q_round_specialized".to_string();
    net.prune();
    (net, desc)
}

/// Develop exact unsigned saturating multiplication: `min(a*b, 2^W-1)`.
/// High partial products feed the overflow predicate directly instead of an
/// exact discarded upper product; only lower-column carries are resolved.
pub fn develop_saturating(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    let (mut net, desc) = develop_internal(genome, width, false, true, false, false, false);
    net.label = "sat_specialized".to_string();
    (net, desc)
}

/// Joint exact arithmetic family. Outputs `[W-1:0]` are saturating product;
/// outputs `[2W-1:W]` are wrapping-low product. Both share partial products,
/// lower-column reduction, and final carry logic.
pub fn develop_low_saturating_family(genome: &Genome, width: usize) -> (Netlist, Descriptors) {
    let (mut net, desc) = develop_internal(genome, width, false, true, true, false, false);
    net.label = "low_sat_family".to_string();
    (net, desc)
}

pub const OP_LOW: u8 = 1;
pub const OP_HIGH: u8 = 2;
pub const OP_ROUND: u8 = 4;
pub const OP_SATURATE: u8 = 8;
pub const OP_OVERFLOW: u8 = 16;
pub const OP_ALL: u8 = OP_LOW | OP_HIGH | OP_ROUND | OP_SATURATE | OP_OVERFLOW;

fn balanced_or(net: &mut Netlist, mut level: Vec<Wire>) -> Wire {
    if level.is_empty() {
        return Wire::Const0;
    }
    while level.len() > 1 {
        let mut next = Vec::with_capacity(level.len().div_ceil(2));
        for pair in level.chunks(2) {
            next.push(if pair.len() == 2 {
                net.or2(pair[0], pair[1])
            } else {
                pair[0]
            });
        }
        level = next;
    }
    level[0]
}

/// Develop any nonempty subset of five related multiply results in canonical
/// low/high/round/saturate/overflow order. Families needing only the low value
/// and predicates use the fused saturation lowering; families needing exact
/// high or rounded values share one full product and derive all outputs from it.
pub fn develop_operation_family(genome: &Genome, width: usize, mask: u8) -> (Netlist, Descriptors) {
    assert!(mask > 0 && mask & !OP_ALL == 0, "invalid operation mask");
    let predicate_only =
        mask & (OP_HIGH | OP_ROUND) == 0 && mask & (OP_SATURATE | OP_OVERFLOW) != 0;
    if predicate_only {
        let need_low = mask & OP_LOW != 0;
        let need_sat = mask & OP_SATURATE != 0;
        let need_overflow = mask & OP_OVERFLOW != 0;
        let (mut net, desc) =
            develop_internal(genome, width, false, true, need_low, need_overflow, false);
        // Internal order is saturated, optional low, optional overflow. Repack
        // to the canonical semantic order and drop unrequested saturation.
        let internal = net.outputs.clone();
        let mut outputs = Vec::new();
        let mut index = width;
        if need_low {
            outputs.extend_from_slice(&internal[index..index + width]);
            index += width;
        }
        if need_sat {
            outputs.extend_from_slice(&internal[..width]);
        }
        if need_overflow {
            outputs.push(internal[index]);
        }
        net.outputs = outputs;
        net.label = format!("ops_{mask:02x}");
        net.prune();
        return (net, desc);
    }

    let (mut net, desc) = develop(genome, width);
    let product = net.outputs.clone();
    let low = product[..width].to_vec();
    let high = product[width..].to_vec();
    let overflow = balanced_or(&mut net, high.clone());
    let mut rounded = Vec::with_capacity(width);
    let mut carry = product[width - 1];
    for &bit in &high {
        rounded.push(net.xor2(bit, carry));
        carry = net.and2(bit, carry);
    }
    let saturated: Vec<_> = low
        .iter()
        .map(|&bit| net.mux2(overflow, bit, Wire::Const1))
        .collect();
    net.outputs.clear();
    if mask & OP_LOW != 0 {
        net.outputs.extend_from_slice(&low);
    }
    if mask & OP_HIGH != 0 {
        net.outputs.extend_from_slice(&high);
    }
    if mask & OP_ROUND != 0 {
        net.outputs.extend_from_slice(&rounded);
    }
    if mask & OP_SATURATE != 0 {
        net.outputs.extend_from_slice(&saturated);
    }
    if mask & OP_OVERFLOW != 0 {
        net.outputs.push(overflow);
    }
    net.label = format!("ops_{mask:02x}");
    net.prune();
    (net, desc)
}

/// Verify exact wrapping multiplication. Widths through eight are exhaustive;
/// wider instances receive fixed corner cases and a deterministic random set.
pub fn verify_low_product(net: &Netlist, random_samples: usize, seed: u64) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    verify_specialized(net, random_samples, seed, |a, b| a.wrapping_mul(b) & mask)
}

pub fn verify_rounded_fractional(
    net: &Netlist,
    random_samples: usize,
    seed: u64,
) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    verify_specialized(net, random_samples, seed, |a, b| {
        ((a.wrapping_mul(b) + (1u64 << (width - 1))) >> width) & mask
    })
}

pub fn verify_saturating(net: &Netlist, random_samples: usize, seed: u64) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    verify_specialized(net, random_samples, seed, |a, b| {
        a.wrapping_mul(b).min(mask)
    })
}

pub fn verify_low_saturating_family(
    net: &Netlist,
    random_samples: usize,
    seed: u64,
) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    verify_specialized(net, random_samples, seed, |a, b| {
        let product = a.wrapping_mul(b);
        product.min(mask) | ((product & mask) << width)
    })
}

fn operation_family_expected(width: usize, mask: u8, a: u64, b: u64) -> u128 {
    let value_mask = (1u64 << width) - 1;
    let product = a.wrapping_mul(b);
    let low = product & value_mask;
    let high = (product >> width) & value_mask;
    let rounded = product
        .wrapping_add(1u64 << (width - 1))
        .wrapping_shr(width as u32)
        & value_mask;
    let overflow = u64::from(high != 0);
    let saturated = if overflow != 0 { value_mask } else { low };
    let mut result = 0u128;
    let mut shift = 0usize;
    for (bit, value, bits) in [
        (OP_LOW, low, width),
        (OP_HIGH, high, width),
        (OP_ROUND, rounded, width),
        (OP_SATURATE, saturated, width),
        (OP_OVERFLOW, overflow, 1),
    ] {
        if mask & bit != 0 {
            result |= (value as u128) << shift;
            shift += bits;
        }
    }
    result
}

pub fn verify_operation_family(
    net: &Netlist,
    mask: u8,
    random_samples: usize,
    seed: u64,
) -> Result<(), String> {
    let width = net.width;
    for (chunk_index, outputs) in net.outputs.chunks(64).enumerate() {
        let mut chunk = net.clone();
        chunk.outputs = outputs.to_vec();
        chunk.prune();
        let shift = 64 * chunk_index;
        verify_specialized(&chunk, random_samples, seed ^ shift as u64, |a, b| {
            (operation_family_expected(width, mask, a, b) >> shift) as u64
        })?;
    }
    Ok(())
}

fn verify_specialized(
    net: &Netlist,
    random_samples: usize,
    seed: u64,
    expected_value: impl Fn(u64, u64) -> u64,
) -> Result<(), String> {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    let mut rng = crate::Rng::new(seed);
    let total = if width <= 8 {
        1usize << (2 * width)
    } else {
        random_samples
    };
    for base in (0..total).step_by(64) {
        let lanes = (total - base).min(64);
        let mut a_bits = vec![0u64; width];
        let mut b_bits = vec![0u64; width];
        let mut expected = vec![0u64; width];
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
            let expected_value = expected_value(a, b);
            for bit in 0..width {
                a_bits[bit] |= ((a >> bit) & 1) << lane;
                b_bits[bit] |= ((b >> bit) & 1) << lane;
                expected[bit] |= ((expected_value >> bit) & 1) << lane;
            }
        }
        let got = net.eval_batch64(&a_bits, &b_bits);
        if let Some(bit) = got.iter().zip(&expected).position(|(x, y)| x != y) {
            let differing = got[bit] ^ expected[bit];
            let lane = differing.trailing_zeros() as usize;
            let (a, b) = pairs[lane];
            return Err(format!(
                "low-product mismatch at width {width}: {a} * {b}, output bit {bit}"
            ));
        }
    }
    if width > 8 {
        for &a in &[0, 1, 2, mask >> 1, mask - 1, mask] {
            for &b in &[0, 1, 2, mask >> 1, mask - 1, mask] {
                let got = net.eval(a, b);
                let expected = expected_value(a, b);
                if got != expected {
                    return Err(format!(
                        "low-product corner mismatch at width {width}: {a} * {b}"
                    ));
                }
            }
        }
    }
    Ok(())
}

fn push_carry(
    next: &mut [Vec<Bit>],
    same: &mut [Vec<Bit>],
    c: usize,
    carry: Bit,
    same_stage: bool,
    desc: &mut Descriptors,
) {
    if c + 1 >= next.len() {
        return;
    }
    if same_stage {
        same[c + 1].push(carry);
        desc.same_stage_carries += 1;
    } else {
        next[c + 1].push(carry);
    }
}

fn order_bits(bits: &mut [Bit], pick: Pick) {
    match pick {
        Pick::Earliest => bits.sort_by_key(|b| b.arrival),
        Pick::Latest => bits.sort_by_key(|b| std::cmp::Reverse(b.arrival)),
        Pick::Positional => {}
        Pick::Alternate => {
            let mut sorted: Vec<Bit> = bits.to_vec();
            sorted.sort_by_key(|b| b.arrival);
            let mut out = Vec::with_capacity(sorted.len());
            let (mut lo, mut hi) = (0usize, sorted.len());
            let mut front = true;
            while lo < hi {
                if front {
                    out.push(sorted[lo]);
                    lo += 1;
                } else {
                    hi -= 1;
                    out.push(sorted[hi]);
                }
                front = !front;
            }
            bits.copy_from_slice(&out);
        }
    }
}

/// Widths that guide selection, widths that are only reported, and the one
/// evaluated occasionally for scaling.
pub const TRAINED_WIDTHS: [usize; 3] = [4, 8, 16];
pub const HELD_OUT_WIDTHS: [usize; 2] = [12, 24];
pub const SCALING_WIDTH: usize = 32;

/// Per-width structural metrics of a developed circuit.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WidthMetrics {
    pub width: usize,
    pub gate_depth: usize,
    pub simple_gates: usize,
    pub cells: usize,
    pub stages: usize,
    pub exact: bool,
}

/// Selection score. Lexicographic: depth at the widest trained width, then
/// the sum of depths at the other trained widths, then gates at the widest
/// width. Lower is better. The fitted depth-versus-width slope is recorded
/// for every elite but is not a selection key: as a key it rewarded genomes
/// that were merely worse at small widths.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Score {
    pub depth_top: usize,
    pub depth_rest: usize,
    pub gates_top: usize,
}

fn slope_milli(points: &[(usize, usize)]) -> i64 {
    // Least-squares slope of ln(y) against ln(x).
    let n = points.len() as f64;
    if points.len() < 2 {
        return 0;
    }
    let xs: Vec<f64> = points.iter().map(|(x, _)| (*x as f64).ln()).collect();
    let ys: Vec<f64> = points
        .iter()
        .map(|(_, y)| (*y as f64).max(1.0).ln())
        .collect();
    let mx = xs.iter().sum::<f64>() / n;
    let my = ys.iter().sum::<f64>() / n;
    let num: f64 = xs.iter().zip(&ys).map(|(x, y)| (x - mx) * (y - my)).sum();
    let den: f64 = xs.iter().map(|x| (x - mx) * (x - mx)).sum();
    if den == 0.0 {
        0
    } else {
        (num / den * 1000.0).round() as i64
    }
}

/// Error statistics of an approximate multiplier at one width, exhaustive.
#[derive(Clone, Debug, PartialEq)]
pub struct ErrorStats {
    pub width: usize,
    pub max_abs: u64,
    pub mean_abs: f64,
    pub error_rate: f64,
    pub bias: f64,
    /// Worst error as a fraction of the full-scale product.
    pub max_rel_full_scale: f64,
}

pub fn error_stats(net: &Netlist) -> ErrorStats {
    let width = net.width;
    let mask = (1u64 << width) - 1;
    let mut max_abs = 0u64;
    let mut sum_abs = 0f64;
    let mut sum_signed = 0f64;
    let mut wrong = 0u64;
    let mut n = 0u64;
    for a in 0..=mask {
        for b in 0..=mask {
            let expected = a * b;
            let got = net.eval(a, b);
            let diff = got as i128 - expected as i128;
            let abs = diff.unsigned_abs() as u64;
            if abs > 0 {
                wrong += 1;
            }
            max_abs = max_abs.max(abs);
            sum_abs += abs as f64;
            sum_signed += diff as f64;
            n += 1;
        }
    }
    let full_scale = ((1u128 << (2 * width)) - 1) as f64;
    ErrorStats {
        width,
        max_abs,
        mean_abs: sum_abs / n as f64,
        error_rate: wrong as f64 / n as f64,
        bias: sum_signed / n as f64,
        max_rel_full_scale: max_abs as f64 / full_scale,
    }
}

#[derive(Clone, Debug)]
pub struct Elite {
    pub id: usize,
    pub genome: Genome,
    pub score: Score,
    pub depth_slope_milli: i64,
    pub descriptors: Descriptors,
    pub trained: Vec<WidthMetrics>,
    pub held_out: Vec<WidthMetrics>,
    pub scaling: Option<WidthMetrics>,
    pub niche: String,
    pub generation: usize,
    pub parents: Vec<String>,
    pub operator: &'static str,
    pub prior_art_distance: f64,
    pub errors: Option<ErrorStats>,
}

/// MAP-Elites niche key from descriptors at the widest trained width.
pub fn niche_key(desc: &Descriptors, width: usize) -> String {
    let adders = desc.full_adders + desc.half_adders;
    let fa_share = desc
        .full_adders
        .checked_mul(4)
        .and_then(|n| n.checked_div(adders))
        .unwrap_or(0)
        .min(3);
    let same = if desc.same_stage_carries == 0 {
        0
    } else if desc.same_stage_carries * 2 < desc.full_adders.max(1) {
        1
    } else {
        2
    };
    let stage_bucket = (desc.stages * 4 / (width.max(2) * 2)).min(7);
    format!(
        "s{}_fa{}_same{}_c42{}_cnt{}_{}_{:?}{}",
        stage_bucket,
        fa_share,
        same,
        usize::from(desc.c42 > 0),
        usize::from(desc.counters > 0),
        desc.adder.name(),
        desc.fa_impl,
        if desc.forced_finish { "_forced" } else { "" }
    )
}

/// Feature vector for prior-art distance: normalized by width so the corpus
/// and the candidates compare at the same width.
pub fn features(desc: &Descriptors, metrics: &WidthMetrics) -> Vec<f64> {
    let w = metrics.width as f64;
    let adders = (desc.full_adders + desc.half_adders).max(1) as f64;
    vec![
        desc.stages as f64 / w,
        desc.full_adders as f64 / (w * w),
        desc.half_adders as f64 / (w * w),
        desc.c42 as f64 / (w * w),
        desc.counters as f64 / (w * w),
        desc.same_stage_carries as f64 / adders,
        metrics.gate_depth as f64 / w,
        metrics.simple_gates as f64 / (w * w),
        match desc.adder {
            Adder::Ripple => 0.0,
            Adder::KoggeStone => 1.0,
            Adder::Sklansky => 2.0,
            Adder::BrentKung => 3.0,
            Adder::HanCarlson => 4.0,
        } / 4.0,
        if desc.fa_impl == FaImpl::Mux {
            1.0
        } else {
            0.0
        },
    ]
}

fn distance(a: &[f64], b: &[f64]) -> f64 {
    a.iter()
        .zip(b)
        .map(|(x, y)| (x - y) * (x - y))
        .sum::<f64>()
        .sqrt()
}

fn develop_metrics(
    genome: &Genome,
    width: usize,
    exact_required: bool,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop(genome, width);
    let counts = net.counts();
    let exact = if width <= 8 {
        crate::gen::verify(&net, 0, 1).is_ok()
    } else {
        crate::gen::verify(&net, 2048, rng_seed).is_ok()
    };
    let _ = exact_required;
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

fn develop_low_metrics(
    genome: &Genome,
    width: usize,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop_low_product(genome, width);
    let counts = net.counts();
    let exact = verify_low_product(&net, 2048, rng_seed).is_ok();
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

fn develop_rounded_metrics(
    genome: &Genome,
    width: usize,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop_rounded_fractional(genome, width);
    let counts = net.counts();
    let exact = verify_rounded_fractional(&net, 2048, rng_seed).is_ok();
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

fn develop_saturating_metrics(
    genome: &Genome,
    width: usize,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop_saturating(genome, width);
    let counts = net.counts();
    let exact = verify_saturating(&net, 2048, rng_seed).is_ok();
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

fn develop_low_sat_family_metrics(
    genome: &Genome,
    width: usize,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop_low_saturating_family(genome, width);
    let counts = net.counts();
    let exact = verify_low_saturating_family(&net, 2048, rng_seed).is_ok();
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

fn develop_operation_family_metrics(
    genome: &Genome,
    width: usize,
    mask: u8,
    rng_seed: u64,
) -> (Netlist, Descriptors, WidthMetrics) {
    let (net, desc) = develop_operation_family(genome, width, mask);
    let counts = net.counts();
    let exact = verify_operation_family(&net, mask, 2048, rng_seed).is_ok();
    let metrics = WidthMetrics {
        width,
        gate_depth: counts.gate_depth,
        simple_gates: counts.simple_gates,
        cells: counts.cells,
        stages: desc.stages,
        exact,
    };
    (net, desc, metrics)
}

pub struct Archive {
    /// "exact" or "approx"; the approximate archive keys niches by error
    /// budget as well as structure.
    pub kind: &'static str,
    pub operation_mask: Option<u8>,
    pub elites: BTreeMap<String, Elite>,
    pub evaluated: usize,
    pub rejected_inexact: usize,
    pub improvements: usize,
    pub next_id: usize,
    pub corpus: Vec<Vec<f64>>,
}

impl Archive {
    pub fn new(kind: &'static str) -> Self {
        Archive {
            kind,
            operation_mask: None,
            elites: BTreeMap::new(),
            evaluated: 0,
            rejected_inexact: 0,
            improvements: 0,
            next_id: 0,
            corpus: Vec::new(),
        }
    }

    pub fn new_operation_family(mask: u8) -> Self {
        let mut archive = Self::new("operation_family");
        archive.operation_mask = Some(mask);
        archive
    }

    /// Develop at the trained widths, verify, score, and insert if the niche
    /// improves. Held-out and scaling widths are developed only for elites
    /// that enter the archive and never influence the decision.
    pub fn offer(
        &mut self,
        genome: Genome,
        generation: usize,
        parents: Vec<String>,
        operator: &'static str,
        seed: u64,
    ) -> Option<Score> {
        self.evaluated += 1;
        let approximate = genome.drop16 > 0;
        if matches!(
            self.kind,
            "low" | "rounded" | "saturating" | "low_sat_family" | "operation_family"
        ) {
            if approximate {
                return None;
            }
        } else if approximate != (self.kind == "approx") {
            return None;
        }
        let mut trained = Vec::new();
        let mut top_desc = None;
        let mut top_net = None;
        for &w in &TRAINED_WIDTHS {
            let (net, desc, m) = match self.kind {
                "low" => develop_low_metrics(&genome, w, seed ^ w as u64),
                "rounded" => develop_rounded_metrics(&genome, w, seed ^ w as u64),
                "saturating" => develop_saturating_metrics(&genome, w, seed ^ w as u64),
                "low_sat_family" => develop_low_sat_family_metrics(&genome, w, seed ^ w as u64),
                "operation_family" => develop_operation_family_metrics(
                    &genome,
                    w,
                    self.operation_mask.expect("operation-family mask"),
                    seed ^ w as u64,
                ),
                _ => develop_metrics(&genome, w, !approximate, seed ^ w as u64),
            };
            if !approximate && !m.exact {
                self.rejected_inexact += 1;
                return None;
            }
            if w == *TRAINED_WIDTHS.last().unwrap() {
                top_desc = Some(desc);
                top_net = Some(net);
            }
            trained.push(m);
        }
        let top = trained.last().unwrap();
        let score = Score {
            depth_top: top.gate_depth,
            depth_rest: trained
                .iter()
                .take(trained.len() - 1)
                .map(|m| m.gate_depth)
                .sum(),
            gates_top: top.simple_gates,
        };
        let depth_slope_milli = slope_milli(
            &trained
                .iter()
                .map(|m| (m.width, m.gate_depth))
                .collect::<Vec<_>>(),
        );
        let desc = top_desc.unwrap();
        let _ = top_net;
        let errors = if approximate {
            let (net8, _) = develop(&genome, 8);
            Some(error_stats(&net8))
        } else {
            None
        };
        let mut niche = niche_key(&desc, top.width);
        if let Some(e) = &errors {
            // Error budget bucket: bits of worst-case error at 8 bits.
            let bits = 64 - e.max_abs.leading_zeros();
            niche = format!("err{bits}_{niche}");
        }
        let better = match self.elites.get(&niche) {
            Some(existing) => score < existing.score,
            None => true,
        };
        if !better {
            return Some(score);
        }
        self.improvements += 1;
        let held_out: Vec<WidthMetrics> = HELD_OUT_WIDTHS
            .iter()
            .map(|&w| match self.kind {
                "low" => develop_low_metrics(&genome, w, seed ^ w as u64).2,
                "rounded" => develop_rounded_metrics(&genome, w, seed ^ w as u64).2,
                "saturating" => develop_saturating_metrics(&genome, w, seed ^ w as u64).2,
                "low_sat_family" => develop_low_sat_family_metrics(&genome, w, seed ^ w as u64).2,
                "operation_family" => {
                    develop_operation_family_metrics(
                        &genome,
                        w,
                        self.operation_mask.expect("operation-family mask"),
                        seed ^ w as u64,
                    )
                    .2
                }
                _ => develop_metrics(&genome, w, false, seed ^ w as u64).2,
            })
            .collect();
        let feats = features(&desc, top);
        let prior_art_distance = self
            .corpus
            .iter()
            .map(|c| distance(c, &feats))
            .fold(f64::INFINITY, f64::min);
        let id = self.next_id;
        self.next_id += 1;
        self.elites.insert(
            niche.clone(),
            Elite {
                id,
                genome,
                score,
                depth_slope_milli,
                descriptors: desc,
                trained,
                held_out,
                scaling: None,
                niche,
                generation,
                parents,
                operator,
                prior_art_distance,
                errors,
            },
        );
        Some(score)
    }

    pub fn best(&self) -> Option<&Elite> {
        self.elites.values().min_by_key(|e| e.score)
    }

    /// Non-dominated elites over (depth at top width, gates at top width).
    pub fn pareto(&self) -> Vec<&Elite> {
        let mut all: Vec<&Elite> = self.elites.values().collect();
        all.sort_by_key(|e| (e.score.depth_top, e.score.gates_top));
        let mut front = Vec::new();
        let mut best_gates = usize::MAX;
        for e in all {
            if e.score.gates_top < best_gates {
                best_gates = e.score.gates_top;
                front.push(e);
            }
        }
        front
    }

    /// Fill in the 32-bit scaling metrics for every elite (structural only).
    pub fn compute_scaling(&mut self, seed: u64) {
        for elite in self.elites.values_mut() {
            if elite.scaling.is_none() {
                elite.scaling = Some(match self.kind {
                    "low" => develop_low_metrics(&elite.genome, SCALING_WIDTH, seed).2,
                    "rounded" => develop_rounded_metrics(&elite.genome, SCALING_WIDTH, seed).2,
                    "saturating" => {
                        develop_saturating_metrics(&elite.genome, SCALING_WIDTH, seed).2
                    }
                    "low_sat_family" => {
                        develop_low_sat_family_metrics(&elite.genome, SCALING_WIDTH, seed).2
                    }
                    "operation_family" => {
                        develop_operation_family_metrics(
                            &elite.genome,
                            SCALING_WIDTH,
                            self.operation_mask.expect("operation-family mask"),
                            seed,
                        )
                        .2
                    }
                    _ => develop_metrics(&elite.genome, SCALING_WIDTH, false, seed).2,
                });
            }
        }
    }
}

pub struct Run {
    pub exact: Archive,
    pub approx: Archive,
    pub seed: u64,
    pub generations: usize,
}

/// The reference corpus: the classical constructions with every adder and
/// both full-adder realizations, at the widest trained width.
pub fn reference_corpus() -> Vec<(String, Genome)> {
    let mut corpus = Vec::new();
    for &adder in &ADDERS {
        for &impl_ in &FA_IMPLS {
            for (name, mut g) in [
                ("array", Genome::array(adder)),
                ("wallace", Genome::wallace(adder)),
                ("dadda", Genome::dadda(adder)),
            ] {
                g.fa_impl = impl_;
                corpus.push((format!("{name}_{}_{:?}", adder.name(), impl_), g));
            }
        }
    }
    corpus
}

/// Run the search. Deterministic for a given seed.
pub fn evolve(generations: usize, seed: u64, mut log: impl FnMut(&str)) -> Run {
    let mut rng = Rng::new(seed);
    let mut exact = Archive::new("exact");
    let mut approx = Archive::new("approx");
    let top = *TRAINED_WIDTHS.last().unwrap();
    let corpus = reference_corpus();
    for (_, g) in &corpus {
        let (_, desc, m) = develop_metrics(g, top, true, seed);
        exact.corpus.push(features(&desc, &m));
    }
    approx.corpus = exact.corpus.clone();
    for (name, g) in &corpus {
        exact.offer(g.clone(), 0, vec![name.clone()], "corpus", seed);
    }
    for _ in 0..60 {
        let g = Genome::random(&mut rng);
        exact.offer(g, 0, vec!["random".into()], "random", seed);
    }
    for _ in 0..20 {
        let mut g = Genome::random(&mut rng);
        g.drop16 = 1 + rng.below(6) as u8;
        approx.offer(g, 0, vec!["random".into()], "random", seed);
    }
    log(&format!(
        "seeded: exact {} niches (best {:?}), approx {} niches",
        exact.elites.len(),
        exact.best().map(|e| e.score),
        approx.elites.len()
    ));
    for generation in 1..=generations {
        let use_approx = rng.below(4) == 0 && !approx.elites.is_empty();
        let (parent_a, parent_b, niche_a, niche_b) = {
            let archive = if use_approx { &approx } else { &exact };
            let keys: Vec<&String> = archive.elites.keys().collect();
            let ka = keys[rng.below(keys.len())].clone();
            let kb = keys[rng.below(keys.len())].clone();
            (
                archive.elites[&ka].genome.clone(),
                archive.elites[&kb].genome.clone(),
                ka,
                kb,
            )
        };
        let (child, operator, parents) = if rng.below(3) == 0 {
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "crossover", vec![niche_a, niche_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            let child = if rng.below(2) == 0 {
                child.mutate(&mut rng)
            } else {
                child
            };
            (child, op, vec![niche_a])
        };
        if child.drop16 > 0 {
            approx.offer(
                child,
                generation,
                parents,
                operator,
                seed ^ generation as u64,
            );
        } else {
            exact.offer(
                child,
                generation,
                parents,
                operator,
                seed ^ generation as u64,
            );
        }
        if generation % 1000 == 0 {
            log(&format!(
                "generation {generation}: exact {} niches / {} improvements / {} inexact, best {:?}; approx {} niches, best {:?}",
                exact.elites.len(),
                exact.improvements,
                exact.rejected_inexact,
                exact.best().map(|e| e.score),
                approx.elites.len(),
                approx.best().map(|e| e.score)
            ));
        }
    }
    exact.compute_scaling(seed);
    approx.compute_scaling(seed);
    Run {
        exact,
        approx,
        seed,
        generations,
    }
}

/// Search exact wrapping-low multipliers. All selection metrics are computed
/// after upper-cone pruning, so a full-product winner receives no advantage.
pub fn evolve_low(generations: usize, seed: u64, mut log: impl FnMut(&str)) -> Archive {
    let mut rng = Rng::new(seed);
    let mut archive = Archive::new("low");
    let top = *TRAINED_WIDTHS.last().unwrap();
    let corpus = reference_corpus();
    for (_, genome) in &corpus {
        let (_, desc, metrics) = develop_low_metrics(genome, top, seed);
        archive.corpus.push(features(&desc, &metrics));
    }
    for (name, genome) in &corpus {
        archive.offer(genome.clone(), 0, vec![name.clone()], "corpus", seed);
    }
    for _ in 0..60 {
        archive.offer(
            Genome::random(&mut rng),
            0,
            vec!["random".into()],
            "random",
            seed,
        );
    }
    for generation in 1..=generations {
        let keys: Vec<&String> = archive.elites.keys().collect();
        let key_a = keys[rng.below(keys.len())].clone();
        let key_b = keys[rng.below(keys.len())].clone();
        let parent_a = archive.elites[&key_a].genome.clone();
        let parent_b = archive.elites[&key_b].genome.clone();
        let (mut child, operator, parents) = if rng.below(3) == 0 {
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "crossover", vec![key_a, key_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            (child, op, vec![key_a])
        };
        child.drop16 = 0;
        archive.offer(
            child,
            generation,
            parents,
            operator,
            seed ^ generation as u64,
        );
        if generation % 1000 == 0 {
            log(&format!(
                "generation {generation}: {} niches / {} improvements, best {:?}",
                archive.elites.len(),
                archive.improvements,
                archive.best().map(|elite| elite.score)
            ));
        }
    }
    archive.compute_scaling(seed);
    archive
}

/// Search exact rounded Q0.W multipliers with the rounding bit fused into the
/// compressor state before reduction.
pub fn evolve_rounded(generations: usize, seed: u64, mut log: impl FnMut(&str)) -> Archive {
    let mut rng = Rng::new(seed);
    let mut archive = Archive::new("rounded");
    let top = *TRAINED_WIDTHS.last().unwrap();
    let corpus = reference_corpus();
    for (_, genome) in &corpus {
        let (_, desc, metrics) = develop_rounded_metrics(genome, top, seed);
        archive.corpus.push(features(&desc, &metrics));
    }
    for (name, genome) in &corpus {
        archive.offer(genome.clone(), 0, vec![name.clone()], "corpus", seed);
    }
    for _ in 0..60 {
        archive.offer(
            Genome::random(&mut rng),
            0,
            vec!["random".into()],
            "random",
            seed,
        );
    }
    for generation in 1..=generations {
        let keys: Vec<&String> = archive.elites.keys().collect();
        let key_a = keys[rng.below(keys.len())].clone();
        let key_b = keys[rng.below(keys.len())].clone();
        let parent_a = archive.elites[&key_a].genome.clone();
        let parent_b = archive.elites[&key_b].genome.clone();
        let (mut child, operator, parents) = if rng.below(3) == 0 {
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "crossover", vec![key_a, key_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            (child, op, vec![key_a])
        };
        child.drop16 = 0;
        archive.offer(
            child,
            generation,
            parents,
            operator,
            seed ^ generation as u64,
        );
        if generation % 1000 == 0 {
            log(&format!(
                "generation {generation}: {} niches / {} improvements, best {:?}",
                archive.elites.len(),
                archive.improvements,
                archive.best().map(|elite| elite.score)
            ));
        }
    }
    archive.compute_scaling(seed);
    archive
}

/// Search exact unsigned saturating multipliers end to end.
pub fn evolve_saturating(generations: usize, seed: u64, mut log: impl FnMut(&str)) -> Archive {
    let mut rng = Rng::new(seed);
    let mut archive = Archive::new("saturating");
    let top = *TRAINED_WIDTHS.last().unwrap();
    let corpus = reference_corpus();
    for (_, genome) in &corpus {
        let (_, desc, metrics) = develop_saturating_metrics(genome, top, seed);
        archive.corpus.push(features(&desc, &metrics));
    }
    for (name, genome) in &corpus {
        archive.offer(genome.clone(), 0, vec![name.clone()], "corpus", seed);
    }
    for _ in 0..60 {
        archive.offer(
            Genome::random(&mut rng),
            0,
            vec!["random".into()],
            "random",
            seed,
        );
    }
    for generation in 1..=generations {
        let keys: Vec<&String> = archive.elites.keys().collect();
        let key_a = keys[rng.below(keys.len())].clone();
        let key_b = keys[rng.below(keys.len())].clone();
        let parent_a = archive.elites[&key_a].genome.clone();
        let parent_b = archive.elites[&key_b].genome.clone();
        let (mut child, operator, parents) = if rng.below(3) == 0 {
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "crossover", vec![key_a, key_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            (child, op, vec![key_a])
        };
        child.drop16 = 0;
        archive.offer(
            child,
            generation,
            parents,
            operator,
            seed ^ generation as u64,
        );
        if generation % 1000 == 0 {
            log(&format!(
                "generation {generation}: {} niches / {} improvements, best {:?}",
                archive.elites.len(),
                archive.improvements,
                archive.best().map(|elite| elite.score)
            ));
        }
    }
    archive.compute_scaling(seed);
    archive
}

/// Search a jointly generated wrapping-low plus saturating multiplier family.
pub fn evolve_low_sat_family(generations: usize, seed: u64, mut log: impl FnMut(&str)) -> Archive {
    let mut rng = Rng::new(seed);
    let mut archive = Archive::new("low_sat_family");
    let top = *TRAINED_WIDTHS.last().unwrap();
    let corpus = reference_corpus();
    for (_, genome) in &corpus {
        let (_, desc, metrics) = develop_low_sat_family_metrics(genome, top, seed);
        archive.corpus.push(features(&desc, &metrics));
    }
    for (name, genome) in &corpus {
        archive.offer(genome.clone(), 0, vec![name.clone()], "corpus", seed);
    }
    for _ in 0..60 {
        archive.offer(
            Genome::random(&mut rng),
            0,
            vec!["random".into()],
            "random",
            seed,
        );
    }
    for generation in 1..=generations {
        let keys: Vec<&String> = archive.elites.keys().collect();
        let key_a = keys[rng.below(keys.len())].clone();
        let key_b = keys[rng.below(keys.len())].clone();
        let parent_a = archive.elites[&key_a].genome.clone();
        let parent_b = archive.elites[&key_b].genome.clone();
        let (mut child, operator, parents) = if rng.below(3) == 0 {
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "crossover", vec![key_a, key_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            (child, op, vec![key_a])
        };
        child.drop16 = 0;
        archive.offer(
            child,
            generation,
            parents,
            operator,
            seed ^ generation as u64,
        );
        if generation % 1000 == 0 {
            log(&format!(
                "generation {generation}: {} niches / {} improvements, best {:?}",
                archive.elites.len(),
                archive.improvements,
                archive.best().map(|elite| elite.score)
            ));
        }
    }
    archive.compute_scaling(seed);
    archive
}

/// One fixed-budget evolutionary run over both semantics and structure.
/// Every family of three or more outputs is a persistent island, preventing
/// raw circuit size from eliminating richer families. One- and two-output
/// cases are intentionally excluded: they are exhaustively screened elsewhere
/// and cannot expose higher-order sharing.
pub fn evolve_operation_families(
    generations: usize,
    seed: u64,
    mut log: impl FnMut(&str),
) -> Vec<Archive> {
    let mut rng = Rng::new(seed);
    let corpus = reference_corpus();
    let top = *TRAINED_WIDTHS.last().unwrap();
    let masks: Vec<u8> = (1..=OP_ALL).filter(|mask| mask.count_ones() >= 3).collect();
    let mut archives: Vec<Archive> = masks
        .iter()
        .copied()
        .map(|mask| {
            let mut archive = Archive::new_operation_family(mask);
            for (_, genome) in &corpus {
                let (_, desc, metrics) =
                    develop_operation_family_metrics(genome, top, mask, seed ^ mask as u64);
                archive.corpus.push(features(&desc, &metrics));
            }
            for (name, genome) in &corpus {
                archive.offer(
                    genome.clone(),
                    0,
                    vec![name.clone()],
                    "corpus",
                    seed ^ mask as u64,
                );
            }
            archive
        })
        .collect();

    for generation in 1..=generations {
        let source_index = rng.below(archives.len());
        let source_mask = archives[source_index]
            .operation_mask
            .expect("operation-family mask");
        let source_keys: Vec<_> = archives[source_index].elites.keys().cloned().collect();
        let key_a = source_keys[rng.below(source_keys.len())].clone();
        let parent_a = archives[source_index].elites[&key_a].genome.clone();
        let (mut child, mut operator, mut parents) = if rng.below(3) == 0 {
            let other_index = rng.below(archives.len());
            let other_keys: Vec<_> = archives[other_index].elites.keys().cloned().collect();
            let key_b = other_keys[rng.below(other_keys.len())].clone();
            let parent_b = archives[other_index].elites[&key_b].genome.clone();
            let crossed = parent_a.crossover(&parent_b, &mut rng);
            let (child, _) = crossed.mutate_named(&mut rng);
            (child, "family_crossover", vec![key_a, key_b])
        } else {
            let (child, op) = parent_a.mutate_named(&mut rng);
            (child, op, vec![key_a])
        };
        child.drop16 = 0;

        let mut destination_mask = source_mask;
        if rng.below(5) == 0 {
            destination_mask ^= 1 << rng.below(5);
            if destination_mask.count_ones() < 3 {
                destination_mask = source_mask;
            } else {
                operator = "semantic_toggle";
                parents.push(format!("mask_{source_mask:02x}"));
            }
        }
        let destination_index = masks
            .iter()
            .position(|&mask| mask == destination_mask)
            .expect("persistent semantic island");
        archives[destination_index].offer(
            child,
            generation,
            parents,
            operator,
            seed ^ generation as u64 ^ destination_mask as u64,
        );
        if generation % 5000 == 0 {
            let occupied: usize = archives.iter().map(|a| a.elites.len()).sum();
            let evaluated: usize = archives.iter().map(|a| a.evaluated).sum();
            log(&format!(
                "generation {generation}: {} semantic islands (3+ outputs), {occupied} niches, {evaluated} evaluations",
                archives.len()
            ));
        }
    }
    archives
}

/// Emit an elite as Verilog at the given width with a stable label.
pub fn emit(genome: &Genome, width: usize, label: &str) -> (Netlist, Descriptors) {
    let (mut net, desc) = develop(genome, width);
    net.label = label.to_string();
    (net, desc)
}

pub fn describe(desc: &Descriptors) -> String {
    let mut s = String::new();
    let _ = write!(
        s,
        "stages={} fa={} ha={} c42={} counters={} same_stage_carries={} forced={} adder={} fa_impl={:?} dropped={}",
        desc.stages,
        desc.full_adders,
        desc.half_adders,
        desc.c42,
        desc.counters,
        desc.same_stage_carries,
        desc.forced_finish,
        desc.adder.name(),
        desc.fa_impl,
        desc.dropped_columns
    );
    s
}

/// One TSV line per elite with lineage and every recorded width.
pub fn archive_tsv(archive: &Archive) -> String {
    let mut out = String::from(
        "id\tniche\tgeneration\toperator\tparents\tdepth_top\tdepth_slope_milli\tgates_top\tprior_art_distance\ttrained\theld_out\tscaling32\terr_max_abs\terr_mean_abs\terr_rate\terr_bias\tdescriptors\tgenome\n",
    );
    let fmt_metrics = |ms: &[WidthMetrics]| -> String {
        ms.iter()
            .map(|m| {
                format!(
                    "w{}:d{}/g{}/c{}/s{}{}",
                    m.width,
                    m.gate_depth,
                    m.simple_gates,
                    m.cells,
                    m.stages,
                    if m.exact { "" } else { "/INEXACT" }
                )
            })
            .collect::<Vec<_>>()
            .join(" ")
    };
    let mut elites: Vec<&Elite> = archive.elites.values().collect();
    elites.sort_by_key(|e| e.score);
    for e in elites {
        let (emax, emean, erate, ebias) = match &e.errors {
            Some(x) => (
                x.max_abs.to_string(),
                format!("{:.3}", x.mean_abs),
                format!("{:.4}", x.error_rate),
                format!("{:.3}", x.bias),
            ),
            None => ("-".into(), "-".into(), "-".into(), "-".into()),
        };
        let _ = writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}",
            e.id,
            e.niche,
            e.generation,
            e.operator,
            e.parents.join("|"),
            e.score.depth_top,
            e.depth_slope_milli,
            e.score.gates_top,
            e.prior_art_distance,
            fmt_metrics(&e.trained),
            fmt_metrics(&e.held_out),
            e.scaling
                .as_ref()
                .map(|m| fmt_metrics(std::slice::from_ref(m)))
                .unwrap_or_default(),
            emax,
            emean,
            erate,
            ebias,
            describe(&e.descriptors),
            e.genome.to_json()
        );
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::gen::{generate, verify, Reduction};

    #[test]
    fn classical_genomes_reproduce_the_named_generators_cell_counts() {
        for &width in &[4usize, 8] {
            for &adder in &ADDERS {
                let (a, _) = develop(&Genome::wallace(adder), width);
                let g = generate(width, Reduction::Wallace, adder);
                assert_eq!(
                    a.counts().cells,
                    g.counts().cells,
                    "wallace {width} {adder:?}"
                );
                let (d, _) = develop(&Genome::dadda(adder), width);
                let g = generate(width, Reduction::Dadda, adder);
                assert_eq!(
                    d.counts().cells,
                    g.counts().cells,
                    "dadda {width} {adder:?}"
                );
            }
        }
    }

    #[test]
    fn every_random_genome_develops_an_exact_multiplier() {
        let mut rng = Rng::new(42);
        for _ in 0..300 {
            let genome = Genome::random(&mut rng);
            for &width in &[4usize, 8] {
                let (net, _) = develop(&genome, width);
                verify(&net, 0, 1).unwrap_or_else(|e| panic!("{e}\n{}", genome.to_json()));
            }
            let mutated = genome.mutate(&mut rng);
            if mutated.drop16 == 0 {
                let (net, _) = develop(&mutated, 5);
                verify(&net, 0, 1).unwrap();
            }
        }
    }

    #[test]
    fn genome_json_round_trips() {
        let mut rng = Rng::new(0xab1a7e);
        for _ in 0..100 {
            let genome = Genome::random(&mut rng).mutate(&mut rng);
            assert_eq!(Genome::from_json(&genome.to_json()).unwrap(), genome);
        }
    }

    #[test]
    fn crossover_children_are_exact() {
        let mut rng = Rng::new(7);
        for _ in 0..100 {
            let a = Genome::random(&mut rng);
            let b = Genome::random(&mut rng);
            let child = a.crossover(&b, &mut rng);
            let (net, _) = develop(&child, 6);
            verify(&net, 0, 1).unwrap();
        }
    }

    #[test]
    fn archive_keeps_the_better_of_two_in_a_niche() {
        let mut archive = Archive::new("exact");
        archive.offer(Genome::dadda(Adder::Ripple), 0, vec![], "corpus", 1);
        let before = archive.best().unwrap().score;
        archive.offer(Genome::dadda(Adder::KoggeStone), 0, vec![], "corpus", 1);
        assert!(archive.best().unwrap().score <= before);
        assert_eq!(archive.elites.len(), 2);
    }

    #[test]
    fn counters_and_mux_adders_are_exact_and_reachable() {
        let mut rng = Rng::new(99);
        let mut saw_counter = false;
        let mut saw_mux = false;
        for _ in 0..200 {
            let mut g = Genome::random(&mut rng);
            g.default_action = if rng.below(2) == 0 {
                Action::Counter53
            } else {
                Action::Counter73
            };
            g.fa_impl = FaImpl::Mux;
            let (net, desc) = develop(&g, 8);
            verify(&net, 0, 1).unwrap();
            saw_counter |= desc.counters > 0;
            saw_mux |= net.counts().famux > 0;
        }
        assert!(saw_counter && saw_mux);
    }

    #[test]
    fn approximate_genomes_report_errors_and_drop_columns() {
        let mut g = Genome::dadda(Adder::Sklansky);
        g.drop16 = 4; // lowest quarter of the columns
        let (net, desc) = develop(&g, 8);
        assert_eq!(desc.dropped_columns, 4);
        let e = error_stats(&net);
        assert!(e.max_abs > 0 && e.max_abs < (1 << 8));
        assert!(e.error_rate > 0.0 && e.error_rate < 1.0);
    }

    #[test]
    fn low_product_specialization_is_exact_and_prunes_the_upper_cone() {
        for width in [4usize, 8] {
            let genome = Genome::dadda(Adder::BrentKung);
            let (full, _) = develop(&genome, width);
            let (low, _) = develop_low_product(&genome, width);
            let mask = (1u64 << width) - 1;
            for a in 0..=mask {
                for b in 0..=mask {
                    assert_eq!(low.eval(a, b), (a * b) & mask);
                }
            }
            assert_eq!(low.outputs.len(), width);
            assert!(low.counts().simple_gates < full.counts().simple_gates);
        }
    }

    #[test]
    fn rounded_fractional_specialization_is_exact() {
        for width in [4usize, 8] {
            let genome = Genome::dadda(Adder::BrentKung);
            let (rounded, _) = develop_rounded_fractional(&genome, width);
            verify_rounded_fractional(&rounded, 0, 7).unwrap();
            assert_eq!(rounded.outputs.len(), width);
        }
    }

    #[test]
    fn saturating_specialization_is_exact() {
        for width in [4usize, 8] {
            let genome = Genome::dadda(Adder::BrentKung);
            let (saturated, _) = develop_saturating(&genome, width);
            verify_saturating(&saturated, 0, 11).unwrap();
            assert_eq!(saturated.outputs.len(), width);
        }
    }

    #[test]
    fn shared_low_saturating_family_is_exact() {
        for width in [4usize, 8] {
            let genome = Genome::dadda(Adder::BrentKung);
            let (family, _) = develop_low_saturating_family(&genome, width);
            verify_low_saturating_family(&family, 0, 13).unwrap();
            assert_eq!(family.outputs.len(), 2 * width);
        }
    }

    #[test]
    fn every_operation_family_mask_is_exact() {
        let genome = Genome::dadda(Adder::BrentKung);
        for mask in 1..=OP_ALL {
            let (family, _) = develop_operation_family(&genome, 4, mask);
            verify_operation_family(&family, mask, 0, 17).unwrap();
        }
        let (widest_output, _) = develop_operation_family(&genome, 16, OP_ALL);
        assert_eq!(widest_output.outputs.len(), 65);
        verify_operation_family(&widest_output, OP_ALL, 1024, 19).unwrap();
    }

    #[test]
    fn large_mutations_keep_children_exact() {
        let mut rng = Rng::new(5);
        let mut ops = std::collections::BTreeSet::new();
        for _ in 0..400 {
            let g = Genome::random(&mut rng);
            let (child, op) = g.mutate_named(&mut rng);
            ops.insert(op);
            if child.drop16 == 0 {
                let (net, _) = develop(&child, 6);
                verify(&net, 0, 1).unwrap();
            }
        }
        for required in [
            "region_replace",
            "policy_replace",
            "threshold_shift",
            "stage_insert",
            "duplicate_shift",
        ] {
            assert!(ops.contains(required), "operator {required} never fired");
        }
    }

    #[test]
    fn slope_is_positive_for_growing_depth() {
        assert!(slope_milli(&[(4, 10), (8, 20), (16, 40)]) > 900);
        assert_eq!(slope_milli(&[(4, 10), (8, 10), (16, 10)]), 0);
    }
}
