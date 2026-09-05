use std::collections::{BTreeMap, HashSet};
use std::process::Command;
use std::time::Instant;
use tinytapeout2_search::{
    baseline_mixed, baseline_nibble_xor, bdd_synthesize, circuit_from_yosys_json, evolve,
    named_target, refine_exact, refine_for_compact_fabric, refine_for_fabric_shape,
    target_nibble_xor, Circuit, Rng, TARGET_NAMES,
};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    match args.get(1).map(String::as_str) {
        Some("benchmark") => benchmark(parse(&args, 2, 1_000_000)),
        Some("search") => search(
            args.get(2).map(String::as_str).unwrap_or("xor"),
            parse(&args, 3, 200_000),
            parse(&args, 4, 24),
            parse_u64(&args, 5, 0xc1ac017),
        ),
        Some("experiment") => experiment(
            args.get(2).map(String::as_str).unwrap_or("mixed"),
            parse(&args, 3, 20),
            parse(&args, 4, 1_000_000),
            parse(&args, 5, 32),
        ),
        Some("archive") => archive(
            args.get(2).map(String::as_str).unwrap_or("mixed"),
            parse(&args, 3, 100_000_000),
            parse(&args, 4, 32),
        ),
        Some("corpus") => corpus(parse(&args, 2, 200_000), parse(&args, 3, 32)),
        Some("bdd") => bdd(args.get(2).map(String::as_str)),
        Some("refine") => refine(
            args.get(2).map(String::as_str).unwrap_or("mixed"),
            parse(&args, 3, 1_000_000),
            parse_u64(&args, 4, 0xdecafbad),
        ),
        Some("map") => map_candidate(
            args.get(2).map(String::as_str).unwrap_or("mixed"),
            parse(&args, 3, 1_000_000),
            parse_u64(&args, 4, 0xdecafbad),
        ),
        Some("fabric-search") => fabric_search(
            args.get(2).map(String::as_str).unwrap_or("mixed"),
            parse(&args, 3, 10_000_000),
            parse_u64(&args, 4, 0xfa8c1c),
        ),
        Some("yosys-seed") => yosys_seed(
            args.get(2).map(String::as_str).unwrap_or("mul_lo"),
            parse(&args, 3, 10_000),
            args.get(4).and_then(|value| value.parse().ok()),
        ),
        Some("post-search") => post_search(
            args.get(2).map(String::as_str).unwrap_or("post_q4_round"),
            parse(&args, 3, 1_000_000),
            parse_u64(&args, 4, 0x5057),
        ),
        Some("gen-cores") => gen_cores(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/generated"),
            &args[3.min(args.len())..],
        ),
        Some("evolve-cores") => evolve_cores(
            parse(&args, 2, 20_000),
            parse_u64(&args, 3, 0xe701),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("benchmarks/evolved"),
            parse(&args, 5, 24),
        ),
        Some("evolve-low") => evolve_specialized_cores(
            parse(&args, 2, 20_000),
            parse_u64(&args, 3, 0x10_e701),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/low_search"),
            parse(&args, 5, 24),
            "low",
        ),
        Some("evolve-rounded") => evolve_specialized_cores(
            parse(&args, 2, 20_000),
            parse_u64(&args, 3, 0x0a11_ce55),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/rounded_search"),
            parse(&args, 5, 24),
            "rounded",
        ),
        Some("evolve-saturating") => evolve_specialized_cores(
            parse(&args, 2, 20_000),
            parse_u64(&args, 3, 0x5a7_e701),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/saturating_search"),
            parse(&args, 5, 24),
            "saturating",
        ),
        Some("evolve-low-sat-family") => evolve_specialized_cores(
            parse(&args, 2, 20_000),
            parse_u64(&args, 3, 0xfa11_e701),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/low_sat_family_search"),
            parse(&args, 5, 24),
            "low_sat_family",
        ),
        Some("evolve-operation-families") => evolve_operation_family_search(
            parse(&args, 2, 31_000),
            parse_u64(&args, 3, 0x0f_fa11),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/joint_operation_search"),
        ),
        Some("gen-operation-family-controls") => gen_operation_family_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/operation_family_controls"),
            &args[3.min(args.len())..],
        ),
        Some("evolve-multi-root") => evolve_multi_root_search(
            parse(&args, 2, 31_000),
            parse_u64(&args, 3, 0x6d75_6c74),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/multi_root_search"),
        ),
        Some("gen-multi-root-controls") => gen_multi_root_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/multi_root_controls"),
        ),
        Some("evolve-requant") => evolve_requant_search(
            parse(&args, 2, 100_000),
            parse_u64(&args, 3, 0x7265_7175),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/requant_search"),
            false,
        ),
        Some("evolve-requant-routing") => evolve_requant_search(
            parse(&args, 2, 100_000),
            parse_u64(&args, 3, 0x726f_7574),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/requant_routing_search"),
            true,
        ),
        Some("evolve-mac-requant") => evolve_mac_search(
            parse(&args, 2, 10_000),
            parse_u64(&args, 3, 0x6d61_6351),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/mac_requant_search"),
            true,
            false,
        ),
        Some("evolve-mac-requant-diverse") => evolve_mac_search(
            parse(&args, 2, 10_000),
            parse_u64(&args, 3, 0x6d61_6344),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/mac_requant_diverse_search"),
            true,
            true,
        ),
        Some("evolve-mac-requant-postadd") => evolve_mac_search(
            parse(&args, 2, 10_000),
            parse_u64(&args, 3, 0x6d61_6351),
            args.get(4)
                .map(String::as_str)
                .unwrap_or("results/discovery/mac_requant_postadd_search"),
            false,
            false,
        ),
        Some("gen-mac-candidate") => gen_mac_candidate(
            args.get(2).expect("genome JSON path"),
            args.get(3).expect("output directory"),
            &args[4.min(args.len())..],
        ),
        Some("gen-mac-controls") => gen_mac_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/mac_controls"),
        ),
        Some("gen-requant-controls") => gen_requant_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/requant_controls"),
        ),
        Some("gen-dev-controls") => gen_dev_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/dev_controls"),
            &args[3.min(args.len())..],
        ),
        Some("ablate-genome") => ablate_genome(
            args.get(2).expect("genome JSON path"),
            args.get(3)
                .map(String::as_str)
                .unwrap_or("results/ablation"),
            &args[4.min(args.len())..],
        ),
        Some("gen-low-product") => gen_low_product(
            args.get(2).expect("genome JSON path"),
            args.get(3).expect("output Verilog path"),
            parse(&args, 4, 8),
        ),
        Some("gen-low-controls") => gen_low_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/low_controls"),
            &args[3.min(args.len())..],
        ),
        Some("gen-rounded-controls") => gen_rounded_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/rounded_controls"),
            &args[3.min(args.len())..],
        ),
        Some("gen-saturating-controls") => gen_saturating_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/saturating_controls"),
            &args[3.min(args.len())..],
        ),
        Some("gen-low-sat-family-controls") => gen_low_sat_family_controls(
            args.get(2)
                .map(String::as_str)
                .unwrap_or("benchmarks/low_sat_family_controls"),
            &args[3.min(args.len())..],
        ),
        _ => usage(),
    }
}

fn parse(args: &[String], index: usize, default: usize) -> usize {
    args.get(index)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn parse_u64(args: &[String], index: usize, default: u64) -> u64 {
    args.get(index)
        .and_then(|v| v.parse().ok())
        .unwrap_or(default)
}

fn usage() {
    eprintln!(
        "usage:\n  cargo run --release -- benchmark [candidates]\n  cargo run --release -- search [target] [generations] [nodes] [seed]\n  cargo run --release -- experiment [target] [runs] [generations] [nodes]\n  cargo run --release -- archive [target] [candidate_budget] [nodes]\n  cargo run --release -- corpus [generations_per_target] [nodes]\n  cargo run --release -- bdd [target|all]\n  cargo run --release -- refine [target] [attempts] [seed]\n  cargo run --release -- map [target] [refinement_attempts] [seed]\n  cargo run --release -- fabric-search [target] [attempts] [seed]\n  cargo run --release -- yosys-seed [target] [scheduler_attempts] [ABC_delay_target]\n  cargo run --release -- post-search [post_target] [attempts] [seed]\n  cargo run --release -- gen-cores [outdir] [widths...]\n  cargo run --release -- gen-dev-controls [outdir] [widths...]\n  cargo run --release -- ablate-genome <genome.json> [outdir] [widths...]\n  cargo run --release -- gen-low-product <genome.json> <output.v> [width]\n  cargo run --release -- gen-low-controls [outdir] [widths...]\n  cargo run --release -- gen-rounded-controls [outdir] [widths...]\n  cargo run --release -- gen-saturating-controls [outdir] [widths...]\n  cargo run --release -- gen-low-sat-family-controls [outdir] [widths...]\n  cargo run --release -- evolve-cores [generations] [seed] [outdir] [emit_count]\n  cargo run --release -- evolve-low [generations] [seed] [outdir] [emit_count]\n  cargo run --release -- evolve-rounded [generations] [seed] [outdir] [emit_count]\n  cargo run --release -- evolve-saturating [generations] [seed] [outdir] [emit_count]\n  cargo run --release -- evolve-low-sat-family [generations] [seed] [outdir] [emit_count]\n  cargo run --release -- evolve-operation-families [generations] [seed] [outdir]"
    );
}

fn gen_low_product(genome_path: &str, output_path: &str, width: usize) {
    let json = std::fs::read_to_string(genome_path).expect("read genome JSON");
    let genome = tinytapeout2_search::evo::Genome::from_json(&json).expect("parse genome JSON");
    let (net, _) = tinytapeout2_search::evo::develop_low_product(&genome, width);
    tinytapeout2_search::evo::verify_low_product(&net, 200_000, 0x10_9a0d)
        .expect("verify exact low product");
    let module = net.module_name();
    std::fs::write(output_path, net.to_verilog()).expect("write low-product Verilog");
    let counts = net.counts();
    println!(
        "{module}: {} cells, {} simple gates, depth {}, verified",
        counts.cells, counts.simple_gates, counts.gate_depth
    );
}

fn gen_low_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{develop_low_product, reference_corpus, verify_low_product};
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tcontrol\tadder\tfa_impl\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for &width in &widths {
        for (control, genome) in reference_corpus() {
            let (mut net, _) = develop_low_product(&genome, width);
            net.label = format!("lo_ctrl_{}", control.to_ascii_lowercase());
            verify_low_product(&net, 200_000, 0x10_c017 ^ width as u64)
                .expect("low-product control failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            let c = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\t{control}\t{}\t{:?}\t{}\t{}\t{}\t{}\n",
                genome.adder.name(),
                genome.fa_impl,
                c.cells,
                c.simple_gates,
                c.depth,
                c.gate_depth
            ));
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote exact low-product controls at widths {widths:?} to {outdir}");
}

fn gen_rounded_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{
        develop_rounded_fractional, reference_corpus, verify_rounded_fractional,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tcontrol\tadder\tfa_impl\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for &width in &widths {
        for (control, genome) in reference_corpus() {
            let (mut net, _) = develop_rounded_fractional(&genome, width);
            net.label = format!("qround_ctrl_{}", control.to_ascii_lowercase());
            verify_rounded_fractional(&net, 200_000, 0x0a11_ce55 ^ width as u64)
                .expect("rounded control failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            let c = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\t{control}\t{}\t{:?}\t{}\t{}\t{}\t{}\n",
                genome.adder.name(),
                genome.fa_impl,
                c.cells,
                c.simple_gates,
                c.depth,
                c.gate_depth
            ));
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote exact rounded controls at widths {widths:?} to {outdir}");
}

fn gen_saturating_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{develop_saturating, reference_corpus, verify_saturating};
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tcontrol\tadder\tfa_impl\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for &width in &widths {
        for (control, genome) in reference_corpus() {
            let (mut net, _) = develop_saturating(&genome, width);
            net.label = format!("sat_ctrl_{}", control.to_ascii_lowercase());
            verify_saturating(&net, 200_000, 0x5a7_c017 ^ width as u64)
                .expect("saturating control failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            let c = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\t{control}\t{}\t{:?}\t{}\t{}\t{}\t{}\n",
                genome.adder.name(),
                genome.fa_impl,
                c.cells,
                c.simple_gates,
                c.depth,
                c.gate_depth
            ));
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote exact saturating controls at widths {widths:?} to {outdir}");
}

fn gen_low_sat_family_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{
        develop_low_product, develop_low_saturating_family, develop_saturating, reference_corpus,
        verify_low_saturating_family,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tcontrol\tadder\tfused_gates\tseparate_gates\tstructural_saving_pct\tcell_depth\tgate_depth\n",
    );
    for &width in &widths {
        for (control, genome) in reference_corpus() {
            let (mut family, _) = develop_low_saturating_family(&genome, width);
            family.label = format!("low_sat_family_ctrl_{}", control.to_ascii_lowercase());
            verify_low_saturating_family(&family, 200_000, 0xfa11_c017 ^ width as u64)
                .expect("low/saturating family control failed verification");
            let name = family.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), family.to_verilog())
                .expect("write family core");
            let fused = family.counts();
            let separate = develop_low_product(&genome, width).0.counts().simple_gates
                + develop_saturating(&genome, width).0.counts().simple_gates;
            let saving = 100.0 * (separate - fused.simple_gates) as f64 / separate as f64;
            manifest.push_str(&format!(
                "{name}\t{width}\t{control}\t{}\t{}\t{}\t{saving:.2}\t{}\t{}\n",
                genome.adder.name(),
                fused.simple_gates,
                separate,
                fused.depth,
                fused.gate_depth
            ));
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote exact shared low/saturating controls at widths {widths:?} to {outdir}");
}

fn benchmark(candidates: usize) {
    let target = target_nibble_xor();
    let mut rng = Rng::new(0x5eed);
    let circuits: Vec<_> = (0..1024).map(|_| Circuit::random(32, &mut rng)).collect();
    let start = Instant::now();
    let mut checksum = 0u64;
    for i in 0..candidates {
        checksum += circuits[i % circuits.len()].score(&target).correct_bits as u64;
    }
    let elapsed = start.elapsed();
    println!("candidates={candidates}");
    println!("seconds={:.6}", elapsed.as_secs_f64());
    println!(
        "candidates_per_second={:.0}",
        candidates as f64 / elapsed.as_secs_f64()
    );
    println!("truth_table_rows_checked={}", candidates * 256);
    println!("checksum={checksum}");
}

fn search(target_name: &str, generations: usize, nodes: usize, seed: u64) {
    let target = require_target(target_name);
    let baseline = if target_name == "xor" {
        baseline_nibble_xor(nodes.max(4))
    } else {
        baseline_mixed(nodes.max(12))
    };
    println!("known_baseline={:?}", baseline.score(&target));
    let start = Instant::now();
    let (circuit, score, evaluated) = evolve(&target, nodes, generations, 8, seed);
    println!("target={target_name}");
    println!("score={score:?}");
    println!("evaluated={evaluated}");
    println!("seconds={:.6}", start.elapsed().as_secs_f64());
    println!("exact={}", score.exact());
    if score.exact() {
        println!("circuit={circuit:#?}");
    }
}

fn experiment(target_name: &str, runs: usize, generations: usize, nodes: usize) {
    let target = require_target(target_name);
    let baseline = if target_name == "xor" {
        baseline_nibble_xor(nodes.max(4))
    } else {
        baseline_mixed(nodes.max(12))
    };
    println!("target,run,exact,correct_bits,active_nodes,depth,max_fanout,evaluated,seconds");
    let mut exact = 0;
    let mut total_evaluated = 0usize;
    let start_all = Instant::now();
    for run in 0..runs {
        let start = Instant::now();
        let (_, score, evaluated) = evolve(
            &target,
            nodes,
            generations,
            8,
            0xc1ac017_u64.wrapping_add(run as u64 * 0x9e3779b9),
        );
        exact += usize::from(score.exact());
        total_evaluated += evaluated;
        println!(
            "{target_name},{run},{},{},{},{},{},{},{:.6}",
            score.exact(),
            score.correct_bits,
            score.active_nodes,
            score.depth,
            score.max_fanout,
            evaluated,
            start.elapsed().as_secs_f64()
        );
    }
    eprintln!("baseline={:?}", baseline.score(&target));
    eprintln!("exact_runs={exact}/{runs}");
    eprintln!("total_evaluated={total_evaluated}");
    eprintln!("total_seconds={:.6}", start_all.elapsed().as_secs_f64());
}

fn archive(target_name: &str, budget: usize, nodes: usize) {
    let target = require_target(target_name);
    let start = Instant::now();
    let mut evaluated = 0usize;
    let mut searches = 0usize;
    let mut exact_survivors = 0usize;
    let mut structures = HashSet::new();
    let mut size_histogram = BTreeMap::<usize, usize>::new();
    let mut best: Option<(Circuit, tinytapeout2_search::Score, u64)> = None;

    while budget.saturating_sub(evaluated) >= 9 {
        let remaining = budget - evaluated;
        let generations = (remaining - 1) / 8;
        let seed = 0xa11ce5eed_u64.wrapping_add(searches as u64 * 0x9e3779b97f4a7c15);
        let (circuit, score, used) = evolve(&target, nodes, generations, 8, seed);
        evaluated += used;
        searches += 1;
        if score.exact() {
            exact_survivors += 1;
            structures.insert(circuit.canonical_key());
            *size_histogram.entry(score.active_nodes).or_default() += 1;
            if best
                .as_ref()
                .map(|(_, old, _)| score.better_than(*old))
                .unwrap_or(true)
            {
                best = Some((circuit, score, seed));
            }
        }
    }

    let mut rng = Rng::new(0xf111_u64);
    while evaluated < budget {
        let circuit = Circuit::random(nodes, &mut rng);
        let score = circuit.score(&target);
        evaluated += 1;
        if score.exact() {
            exact_survivors += 1;
            structures.insert(circuit.canonical_key());
            *size_histogram.entry(score.active_nodes).or_default() += 1;
            if best
                .as_ref()
                .map(|(_, old, _)| score.better_than(*old))
                .unwrap_or(true)
            {
                best = Some((circuit, score, 0xf111_u64));
            }
        }
    }

    println!("target={target_name}");
    println!("candidate_budget={budget}");
    println!("evaluated={evaluated}");
    println!("independent_searches={searches}");
    println!("exact_survivors={exact_survivors}");
    println!("distinct_structures={}", structures.len());
    println!("best_score={:?}", best.as_ref().map(|(_, score, _)| score));
    println!("best_seed={:?}", best.as_ref().map(|(_, _, seed)| seed));
    if let Some((circuit, _, _)) = best {
        println!("best_circuit={circuit:#?}");
    }
    println!("active_node_histogram={size_histogram:?}");
    println!("seconds={:.6}", start.elapsed().as_secs_f64());
}

fn corpus(generations: usize, nodes: usize) {
    println!("target,exact,correct_bits,active_nodes,depth,max_fanout,evaluated,seconds");
    for (index, &name) in TARGET_NAMES.iter().enumerate() {
        let target = require_target(name);
        let start = Instant::now();
        let (_, score, evaluated) = evolve(
            &target,
            nodes,
            generations,
            8,
            0xc0ffee_u64.wrapping_add(index as u64 * 0x9e3779b9),
        );
        println!(
            "{name},{},{},{},{},{},{},{:.6}",
            score.exact(),
            score.correct_bits,
            score.active_nodes,
            score.depth,
            score.max_fanout,
            evaluated,
            start.elapsed().as_secs_f64()
        );
    }
}

fn require_target(name: &str) -> [tinytapeout2_search::Signature; 4] {
    named_target(name).unwrap_or_else(|| {
        eprintln!(
            "unknown target {name:?}; choices: {}",
            TARGET_NAMES.join(", ")
        );
        std::process::exit(2);
    })
}

fn bdd(target_name: Option<&str>) {
    println!("target,exact,total_nodes,active_nodes,depth,max_fanout,fits32,fits64");
    let names: Vec<&str> = match target_name.unwrap_or("all") {
        "all" => TARGET_NAMES.to_vec(),
        name => vec![name],
    };
    for name in names {
        let target = require_target(name);
        let circuit = bdd_synthesize(&target);
        let score = circuit.score(&target);
        println!(
            "{name},{},{},{},{},{},{},{}",
            score.exact(),
            circuit.nodes.len(),
            score.active_nodes,
            score.depth,
            score.max_fanout,
            score.active_nodes <= 32,
            score.active_nodes <= 64
        );
    }
}

fn refine(target_name: &str, attempts: usize, seed: u64) {
    let target = require_target(target_name);
    let initial = bdd_synthesize(&target);
    let initial_score = initial.score(&target);
    let start = Instant::now();
    let (circuit, score, accepted) = refine_exact(&initial, &target, attempts, seed);
    println!("target={target_name}");
    println!("initial={initial_score:?}");
    println!("attempts={attempts}");
    println!("accepted_exact_changes={accepted}");
    println!("final={score:?}");
    println!("seconds={:.6}", start.elapsed().as_secs_f64());
    println!("circuit={circuit:#?}");
}

fn map_candidate(target_name: &str, attempts: usize, seed: u64) {
    let target = require_target(target_name);
    let initial = bdd_synthesize(&target);
    let (circuit, score, _) = refine_exact(&initial, &target, attempts, seed);
    println!("target={target_name}");
    println!("abstract_score={score:?}");
    match circuit.map_to_compact_fabric() {
        Ok(mapped) => {
            let exact = (0..256).all(|row| {
                let inputs = std::array::from_fn(|i| ((row >> i) & 1) != 0);
                mapped.evaluate_row(inputs) == circuit.evaluate_row(inputs)
            });
            println!("mapped=true");
            println!("mapped_exact={exact}");
            println!("useful_nodes={}", mapped.useful_nodes);
            println!("pass_nodes={}", mapped.pass_nodes);
            println!("occupied_nodes={}", mapped.useful_nodes + mapped.pass_nodes);
        }
        Err(error) => {
            println!("mapped=false");
            println!("map_error={error:?}");
        }
    }
}

fn fabric_search(target_name: &str, attempts: usize, seed: u64) {
    let target = require_target(target_name);
    let initial = bdd_synthesize(&target);
    let start = Instant::now();
    let (circuit, score, rank, accepted) =
        refine_for_compact_fabric(&initial, &target, attempts, seed);
    println!("target={target_name}");
    println!("attempts={attempts}");
    println!("accepted_exact_changes={accepted}");
    println!("score={score:?}");
    println!("fabric_rank={rank:?}");
    println!("mapped={}", rank.unmappable == 0);
    println!("seconds={:.6}", start.elapsed().as_secs_f64());
    println!("circuit={circuit:#?}");
}

fn yosys_seed(target_name: &str, scheduler_attempts: usize, delay_target: Option<usize>) {
    let circuit = synthesize_yosys_seed(target_name, delay_target);
    let target = require_target(target_name);
    let score = circuit.score(&target);
    println!("target={target_name}");
    println!("abc_delay_target={delay_target:?}");
    println!("score={score:?}");
    println!("exact={}", score.exact());
    println!("fabric_rank={:?}", circuit.compact_fabric_rank());
    println!("map_result={:?}", circuit.map_to_compact_fabric());
    println!(
        "scheduled_map_result={:?}",
        circuit.map_to_compact_fabric_scheduled(scheduler_attempts, 0x5cedu64)
    );
    if target_name.starts_with("post_") {
        let post = circuit.map_to_post_fabric(scheduler_attempts, 0x5cedu64);
        println!("required_post_shape=4x4");
        println!("post_map_result={post:?}");
        if post.is_err() {
            eprintln!("candidate rejected: does not fit mandatory 4x4 post fabric");
            std::process::exit(3);
        }
    }
    for (layers, width) in [(6, 8), (8, 6), (10, 5), (12, 4), (8, 8), (10, 8), (12, 8)] {
        match circuit.map_to_fabric_scheduled(layers, width, scheduler_attempts, 0x5cedu64) {
            Ok(mapped) => println!(
                "shape={layers}x{width} mapped=true useful={} pass={} occupied={}",
                mapped.useful_nodes,
                mapped.pass_nodes,
                mapped.useful_nodes + mapped.pass_nodes
            ),
            Err(error) => println!("shape={layers}x{width} mapped=false error={error:?}"),
        }
    }
}

fn module_for_target(target_name: &str) -> &'static str {
    let module = match target_name {
        "add4" => "add4_target",
        "mul_lo" => "mul_lo_target",
        "mul_hi" => "mul_hi_target",
        "mul_q4_round" => "mul_q4_round_target",
        "mul_sat4" => "mul_sat4_target",
        "post_lo" => "post_lo_target",
        "post_hi" => "post_hi_target",
        "post_q4_round" => "post_q4_round_target",
        "post_sat4" => "post_sat4_target",
        "popcount" => "popcount_target",
        "priority" => "priority_target",
        "compare" => "compare_target",
        "crc4" => "crc4_target",
        _ => {
            eprintln!("no Yosys benchmark module for {target_name:?}");
            std::process::exit(2);
        }
    };
    module
}

fn synthesize_yosys_seed(target_name: &str, delay_target: Option<usize>) -> Circuit {
    let module = module_for_target(target_name);
    let path =
        std::env::temp_dir().join(format!("tinytapeout2-{module}-{}.json", std::process::id()));
    let abc = delay_target
        .map(|delay| format!("abc -g simple -D {delay}"))
        .unwrap_or_else(|| "abc -g simple".to_string());
    let script = format!(
        "read_verilog benchmarks/targets.v; hierarchy -top {module}; proc; flatten; opt; techmap; opt; {abc}; write_json {}",
        path.display()
    );
    let status = Command::new("yosys")
        .args(["-Q", "-q", "-p", &script])
        .status()
        .expect("failed to execute yosys");
    if !status.success() {
        std::process::exit(status.code().unwrap_or(1));
    }
    let json = std::fs::read_to_string(&path).expect("failed to read Yosys JSON");
    let _ = std::fs::remove_file(&path);
    circuit_from_yosys_json(&json, module).expect("failed to import Yosys JSON")
}

fn post_search(target_name: &str, attempts: usize, seed: u64) {
    if !target_name.starts_with("post_") {
        eprintln!("post-search requires a post_* target");
        std::process::exit(2);
    }
    let target = require_target(target_name);
    let initial = synthesize_yosys_seed(target_name, None);
    let initial_score = initial.score(&target);
    let initial_map = initial
        .map_to_post_fabric(100_000, seed ^ 0x51ced)
        .expect("synthesis seed must fit mandatory 4x4 post fabric");
    let start = Instant::now();
    let (circuit, score, mapped, accepted, rejected_unmappable) =
        refine_for_fabric_shape(&initial, &target, attempts, seed, 4, 4);
    println!("target={target_name}");
    println!("required_shape=4x4");
    println!("attempts={attempts}");
    println!("initial_score={initial_score:?}");
    println!(
        "initial_occupied={}",
        initial_map.useful_nodes + initial_map.pass_nodes
    );
    println!("final_score={score:?}");
    println!("final_useful={}", mapped.useful_nodes);
    println!("final_pass={}", mapped.pass_nodes);
    println!("final_occupied={}", mapped.useful_nodes + mapped.pass_nodes);
    println!("accepted_exact_mappable={accepted}");
    println!("rejected_exact_unmappable={rejected_unmappable}");
    println!("seconds={:.6}", start.elapsed().as_secs_f64());
    println!("circuit={circuit:#?}");
}

/// Emit every (reduction, final adder) core at each requested width, verify
/// each netlist in software, and write testbenches, equivalence scripts and a
/// manifest with cell counts.
fn gen_cores(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::gen::{
        equivalence_script, generate, testbench_verilog, verify, ADDERS, CELLS_VERILOG, REDUCTIONS,
    };
    let widths: Vec<usize> = if widths.is_empty() {
        vec![4, 8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\treduction\tadder\tcells\tsimple_gates\tcell_depth\tgate_depth\tand2\tha\tfa\txor2\txor3\tpg\tblack\tgray\n",
    );
    for &width in &widths {
        let mut modules = Vec::new();
        for &reduction in &REDUCTIONS {
            for &adder in &ADDERS {
                let net = generate(width, reduction, adder);
                verify(&net, 200_000, 0x9e3779b97f4a7c15 ^ width as u64)
                    .expect("generated core failed software verification");
                let name = net.module_name();
                std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
                std::fs::write(
                    format!("{outdir}/equiv_{name}.ys"),
                    equivalence_script(width, &name, outdir),
                )
                .expect("write equivalence script");
                let c = net.counts();
                manifest.push_str(&format!(
                    "{name}\t{width}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
                    reduction.name(),
                    adder.name(),
                    c.cells,
                    c.simple_gates,
                    c.depth,
                    c.gate_depth,
                    c.and2,
                    c.ha,
                    c.fa,
                    c.xor2,
                    c.xor3,
                    c.pg,
                    c.black,
                    c.gray
                ));
                println!(
                    "{name}: {} cells, {} simple gates, depth {} cells / {} gates, verified in software",
                    c.cells, c.simple_gates, c.depth, c.gate_depth
                );
                modules.push(name);
            }
        }
        std::fs::write(
            format!("{outdir}/tb_mul{width}.sv"),
            testbench_verilog(width, &modules),
        )
        .expect("write testbench");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote {outdir}");
}

/// Emit the complete developmental control corpus. Unlike `gen-cores`, this
/// crosses each textbook reduction with both physical full-adder styles, so
/// evolved mux-carry genomes always have an apples-to-apples control.
fn gen_dev_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{emit, reference_corpus};
    use tinytapeout2_search::gen::{equivalence_script, testbench_verilog, verify, CELLS_VERILOG};
    let widths: Vec<usize> = if widths.is_empty() {
        vec![4, 8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\treduction\tadder\tfa_impl\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for &width in &widths {
        let mut modules = Vec::new();
        for (control, genome) in reference_corpus() {
            let label = format!("ctrl_{control}").to_ascii_lowercase();
            let (net, _) = emit(&genome, width, &label);
            verify(&net, 200_000, 0xc017_2015 ^ width as u64)
                .expect("developmental control failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            if width <= 8 {
                std::fs::write(
                    format!("{outdir}/equiv_{name}.ys"),
                    equivalence_script(width, &name, outdir),
                )
                .expect("write equivalence script");
            }
            let counts = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\tdevelopmental\t{}\t{:?}\t{}\t{}\t{}\t{}\n",
                genome.adder.name(),
                genome.fa_impl,
                counts.cells,
                counts.simple_gates,
                counts.depth,
                counts.gate_depth
            ));
            modules.push(name);
        }
        std::fs::write(
            format!("{outdir}/tb_mul{width}.sv"),
            testbench_verilog(width, &modules),
        )
        .expect("write testbench");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!(
        "wrote {} matched developmental controls to {outdir}",
        modules_count(&widths)
    );
}

fn modules_count(widths: &[usize]) -> usize {
    tinytapeout2_search::evo::reference_corpus().len() * widths.len()
}

/// Emit the original genome and every single-rule deletion. This identifies
/// whether a physical result belongs to a compact causal rule or to an
/// accidental interaction among inactive genes.
fn ablate_genome(path: &str, outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{describe, emit, Genome};
    use tinytapeout2_search::gen::{equivalence_script, testbench_verilog, verify, CELLS_VERILOG};
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths.iter().map(|w| w.parse().expect("width")).collect()
    };
    let original = Genome::from_json(&std::fs::read_to_string(path).expect("read genome"))
        .expect("parse genome");
    let mut variants = Vec::new();
    if original.rules.len() <= 6 {
        for mask in 0..(1usize << original.rules.len()) {
            let mut genome = original.clone();
            genome.rules = original
                .rules
                .iter()
                .enumerate()
                .filter(|(index, _)| mask & (1 << index) != 0)
                .map(|(_, rule)| *rule)
                .collect();
            variants.push((
                format!("rules_{mask:0width$b}", width = original.rules.len()),
                genome,
            ));
        }
    } else {
        variants.push(("base".to_string(), original.clone()));
        for index in 0..original.rules.len() {
            let mut genome = original.clone();
            genome.rules.remove(index);
            variants.push((format!("drop_rule{index}"), genome));
        }
    }
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\treduction\tadder\tvariant\tcells\tsimple_gates\tcell_depth\tgate_depth\tdescriptors\n",
    );
    for &width in &widths {
        let mut modules = Vec::new();
        for (label, genome) in &variants {
            let (net, descriptors) = emit(genome, width, &format!("ablate_{label}"));
            verify(&net, 200_000, 0x00ab_1a7e ^ width as u64)
                .expect("ablation failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            if width <= 8 {
                std::fs::write(
                    format!("{outdir}/equiv_{name}.ys"),
                    equivalence_script(width, &name, outdir),
                )
                .expect("write equivalence script");
            }
            let counts = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\tablation\t{}\t{label}\t{}\t{}\t{}\t{}\t{}\n",
                genome.adder.name(),
                counts.cells,
                counts.simple_gates,
                counts.depth,
                counts.gate_depth,
                describe(&descriptors)
            ));
            modules.push(name);
        }
        std::fs::write(
            format!("{outdir}/tb_mul{width}.sv"),
            testbench_verilog(width, &modules),
        )
        .expect("write testbench");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote {} ablations to {outdir}", variants.len());
}

/// Run the developmental search, write both archives with lineage, and emit
/// the exact Pareto front plus the most prior-art-distant elites as Verilog
/// at every trained, held-out and scaling width, with SAT scripts for 4 and 8.
fn evolve_cores(generations: usize, seed: u64, outdir: &str, emit_count: usize) {
    use tinytapeout2_search::evo::{
        archive_tsv, emit, evolve, HELD_OUT_WIDTHS, SCALING_WIDTH, TRAINED_WIDTHS,
    };
    use tinytapeout2_search::gen::{equivalence_script, testbench_verilog, CELLS_VERILOG};
    let start = Instant::now();
    let run = evolve(generations, seed, |line| println!("{line}"));
    println!(
        "search finished in {:.1}s: exact {} niches from {} evaluations ({} inexact rejected), approx {} niches",
        start.elapsed().as_secs_f64(),
        run.exact.elites.len(),
        run.exact.evaluated,
        run.exact.rejected_inexact,
        run.approx.elites.len()
    );
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(
        format!("{outdir}/exact_archive.tsv"),
        archive_tsv(&run.exact),
    )
    .expect("write");
    std::fs::write(
        format!("{outdir}/approx_archive.tsv"),
        archive_tsv(&run.approx),
    )
    .expect("write");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "seed={seed}\ngenerations={generations}\ntrained={:?}\nheld_out={:?}\nscaling={}\n",
            TRAINED_WIDTHS, HELD_OUT_WIDTHS, SCALING_WIDTH
        ),
    )
    .expect("write");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");

    // Selection for emission: the exact Pareto front, then the elites most
    // distant from the reference corpus, up to emit_count.
    let mut chosen: Vec<usize> = run.exact.pareto().iter().map(|e| e.id).collect();
    if let Some(best) = run.exact.best() {
        if !chosen.contains(&best.id) {
            chosen.push(best.id);
        }
    }
    let mut by_distance: Vec<_> = run.exact.elites.values().collect();
    by_distance.sort_by(|a, b| {
        b.prior_art_distance
            .partial_cmp(&a.prior_art_distance)
            .unwrap()
    });
    for e in by_distance {
        if chosen.len() >= emit_count {
            break;
        }
        if !chosen.contains(&e.id) {
            chosen.push(e.id);
        }
    }
    let mut manifest = String::from("module\twidth\treduction\tadder\tcells\tsimple_gates\tcell_depth\tgate_depth\tid\tniche\tprior_art_distance\n");
    let mut widths: Vec<usize> = TRAINED_WIDTHS.to_vec();
    widths.extend(HELD_OUT_WIDTHS);
    widths.push(SCALING_WIDTH);
    let mut modules_by_width: BTreeMap<usize, Vec<String>> = BTreeMap::new();
    for e in run.exact.elites.values().filter(|e| chosen.contains(&e.id)) {
        let label = format!("evo{}", e.id);
        std::fs::write(format!("{outdir}/{label}.json"), e.genome.to_json()).expect("write genome");
        for &w in &widths {
            let (net, _) = emit(&e.genome, w, &label);
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            if w <= 8 {
                std::fs::write(
                    format!("{outdir}/equiv_{name}.ys"),
                    equivalence_script(w, &name, outdir),
                )
                .expect("write");
            }
            let c = net.counts();
            manifest.push_str(&format!(
                "{name}\t{w}\tevo\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\n",
                e.genome.adder.name(),
                c.cells,
                c.simple_gates,
                c.depth,
                c.gate_depth,
                e.id,
                e.niche,
                e.prior_art_distance
            ));
            modules_by_width.entry(w).or_default().push(name);
        }
    }
    for (w, modules) in &modules_by_width {
        std::fs::write(
            format!("{outdir}/tb_mul{w}.sv"),
            testbench_verilog(*w, modules),
        )
        .expect("write tb");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!(
        "emitted {} exact elites at widths {:?} to {outdir}",
        chosen.len(),
        widths
    );
    if let Some(best) = run.exact.best() {
        println!(
            "best exact: id {} niche {} score {:?} distance {:.3} op {} gen {}",
            best.id,
            best.niche,
            best.score,
            best.prior_art_distance,
            best.operator,
            best.generation
        );
    }
    for e in run.exact.pareto() {
        println!(
            "pareto: id {} depth {} gates {} slope {} niche {} distance {:.3}",
            e.id,
            e.score.depth_top,
            e.score.gates_top,
            e.depth_slope_milli,
            e.niche,
            e.prior_art_distance
        );
    }
}

fn operation_mask_name(mask: u8) -> String {
    [
        (1, "lo"),
        (2, "hi"),
        (4, "round"),
        (8, "sat"),
        (16, "overflow"),
    ]
    .iter()
    .filter_map(|&(bit, name)| (mask & bit != 0).then_some(name))
    .collect::<Vec<_>>()
    .join("_")
}

fn gen_operation_family_controls(outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::{
        develop_operation_family, reference_corpus, verify_operation_family, OP_ALL,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let widths: Vec<usize> = if widths.is_empty() {
        vec![8, 16]
    } else {
        widths
            .iter()
            .map(|width| width.parse().expect("integer width"))
            .collect()
    };
    std::fs::create_dir_all(outdir).expect("create operation-family control directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tmask\toperations\treduction\tadder\tfa_impl\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for mask in (1..=OP_ALL).filter(|mask: &u8| mask.count_ones() >= 3) {
        let operations = operation_mask_name(mask);
        for (control, genome) in reference_corpus() {
            let mut fields = control.split('_');
            let reduction = fields.next().expect("control reduction");
            let adder = fields.next().expect("control adder");
            let fa_impl = fields.next().expect("control full-adder implementation");
            for &width in &widths {
                let (mut net, _) = develop_operation_family(&genome, width, mask);
                net.label = format!("family_{operations}_ctrl_{control}").to_ascii_lowercase();
                verify_operation_family(
                    &net,
                    mask,
                    200_000,
                    0x0c01_7001_u64 ^ mask as u64 ^ width as u64,
                )
                .expect("operation-family control failed verification");
                let module = net.module_name();
                let counts = net.counts();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write operation-family control");
                manifest.push_str(&format!(
                    "{module}\t{width}\t0x{mask:02x}\t{operations}\t{reduction}\t{adder}\t{fa_impl}\t{}\t{}\t{}\t{}\n",
                    counts.cells, counts.simple_gates, counts.depth, counts.gate_depth
                ));
            }
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest)
        .expect("write operation-family control manifest");
    println!(
        "wrote all {} textbook operation-family controls at widths {widths:?} to {outdir}",
        (1..=OP_ALL)
            .filter(|mask: &u8| mask.count_ones() >= 3)
            .count()
            * reference_corpus().len()
    );
}

fn evolve_multi_root_search(generations: usize, seed: u64, outdir: &str) {
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::multiroot::{
        develop_family, evolve_multi_root_families, verify_family,
    };
    let start = Instant::now();
    let islands = evolve_multi_root_families(generations, seed, |line| println!("{line}"));
    std::fs::create_dir_all(outdir).expect("create multi-root output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut summary = String::from(
        "mask\toperations\tevaluated\trejected\tpareto_elites\tbest_id\tdepth16\tgates16\tgeneration\toperator\n",
    );
    let mut manifest = String::from(
        "module\twidth\tmask\toperations\trole\tid\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for island in &islands {
        let operations = operation_mask_name(island.mask);
        let best = island.best().expect("seeded multi-root island");
        summary.push_str(&format!(
            "0x{:02x}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\n",
            island.mask,
            operations,
            island.evaluated,
            island.rejected,
            island.elites.len(),
            best.id,
            best.depth16,
            best.gates16,
            best.generation,
            best.operator
        ));
        std::fs::write(
            format!("{outdir}/family_{:02x}_best.json", island.mask),
            serde_json::to_string_pretty(&best.genome.to_json()).expect("serialize family genome"),
        )
        .expect("write family genome");
        for width in [8usize, 16] {
            let mut net = develop_family(&best.genome, width);
            net.label = format!("multiroot_{operations}_evo{}", best.id);
            verify_family(
                &net,
                island.mask,
                200_000,
                seed ^ island.mask as u64 ^ width as u64,
            )
            .expect("emitted multi-root family failed joint verification");
            let module = net.module_name();
            let counts = net.counts();
            std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                .expect("write multi-root Verilog");
            manifest.push_str(&format!(
                "{module}\t{width}\t0x{:02x}\t{operations}\tevolved\t{}\t{}\t{}\t{}\t{}\n",
                island.mask,
                best.id,
                counts.cells,
                counts.simple_gates,
                counts.depth,
                counts.gate_depth
            ));
        }
    }
    std::fs::write(format!("{outdir}/summary.tsv"), summary).expect("write summary");
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "seed={seed}\ngenerations={generations}\nsemantic_islands={}\nminimum_outputs=3\none_genome=true\none_union_pruned_circuit=true\nearly_stop=false\nelapsed_seconds={:.3}\n",
            islands.len(),
            start.elapsed().as_secs_f64()
        ),
    )
    .expect("write run record");
    println!(
        "multi-root search finished in {:.1}s across {} fixed 3+ output contracts",
        start.elapsed().as_secs_f64(),
        islands.len()
    );
}

fn gen_multi_root_controls(outdir: &str) {
    use tinytapeout2_search::evo::{
        reference_corpus, OP_HIGH, OP_LOW, OP_OVERFLOW, OP_ROUND, OP_SATURATE,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::multiroot::{
        develop_family, verify_family, FamilyGenome, ReductionShape, RootPlan, RoundCarry,
        SaturationImpl,
    };
    let roots = RootPlan {
        overflow: ReductionShape::Balanced,
        round_carry: RoundCarry::Prefix,
        saturation: SaturationImpl::OrMask,
        share_predicate: true,
    };
    let masks = [
        OP_HIGH | OP_ROUND | OP_SATURATE,
        OP_LOW | OP_HIGH | OP_ROUND | OP_SATURATE,
        OP_LOW | OP_ROUND | OP_SATURATE | OP_OVERFLOW,
    ];
    std::fs::create_dir_all(outdir).expect("create multi-root control directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from("module\twidth\treduction\tadder\tmask\toperations\tfa_impl\n");
    for mask in masks {
        let operations = operation_mask_name(mask);
        for (control, arithmetic) in reference_corpus() {
            let mut fields = control.split('_');
            let reduction = fields.next().expect("reduction");
            let adder = fields.next().expect("adder");
            let fa_impl = fields.next().expect("full-adder implementation");
            let family = FamilyGenome::new(arithmetic, mask, roots);
            for width in [8usize, 16] {
                let mut net = develop_family(&family, width);
                net.label = format!("multiroot_{operations}_ctrl_{control}").to_ascii_lowercase();
                verify_family(
                    &net,
                    mask,
                    200_000,
                    0x6d72_c017 ^ mask as u64 ^ width as u64,
                )
                .expect("multi-root control failed verification");
                let module = net.module_name();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write multi-root control");
                manifest.push_str(&format!(
                    "{module}\t{width}\t{reduction}\t{adder}\t0x{mask:02x}\t{operations}\t{fa_impl}\n"
                ));
            }
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote strongest-root-plan controls to {outdir}");
}

fn evolve_requant_search(generations: usize, seed: u64, outdir: &str, routing_aware: bool) {
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::workload::{
        develop_requant, evolve_requant, evolve_requant_routing, verify_requant,
    };
    let start = Instant::now();
    let archive = if routing_aware {
        evolve_requant_routing(generations, seed, |line| println!("{line}"))
    } else {
        evolve_requant(generations, seed, |line| println!("{line}"))
    };
    std::fs::create_dir_all(outdir).expect("create requant output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\trole\tid\tdepth16\tgates16\trouting16\tgeneration\toperator\tsticky_reduction\tsaturation_reduction\tround_carry\tsaturation_impl\tshare_saturation\n",
    );
    let mut pareto = archive.clone();
    pareto.sort_by_key(|elite| (elite.depth16, elite.gates16));
    for elite in &pareto {
        for width in [8usize, 16] {
            let mut net = develop_requant(&elite.genome, width);
            net.label = format!("requant_evo{}", elite.id);
            verify_requant(&net, 200_000, seed ^ width as u64 ^ elite.id as u64)
                .expect("requant elite failed independent verification");
            let module = net.module_name();
            std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                .expect("write requant elite");
            manifest.push_str(&format!(
                "{module}\t{width}\tevolved\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{:?}\t{:?}\t{:?}\t{}\n",
                elite.id,
                elite.depth16,
                elite.gates16,
                elite.routing16,
                elite.generation,
                elite.operator,
                elite.genome.plan.sticky_reduction,
                elite.genome.plan.saturation_reduction,
                elite.genome.plan.round_carry,
                elite.genome.plan.saturation,
                elite.genome.plan.share_saturation_predicate
            ));
        }
        std::fs::write(
            format!("{outdir}/requant_evo{}.json", elite.id),
            format!(
                "{{\"arithmetic\":{},\"sticky_reduction\":\"{:?}\",\"saturation_reduction\":\"{:?}\",\"round_carry\":\"{:?}\",\"saturation_impl\":\"{:?}\",\"share_saturation\":{}}}",
                elite.genome.arithmetic.to_json(),
                elite.genome.plan.sticky_reduction,
                elite.genome.plan.saturation_reduction,
                elite.genome.plan.round_carry,
                elite.genome.plan.saturation,
                elite.genome.plan.share_saturation_predicate
            ),
        )
        .expect("write requant genome");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "contract=unsigned_scaled_mul_round_half_up_sat_status\nshift=W/2\noutputs=result,saturated,inexact\none_genome=true\none_circuit=true\nobjectives={}\nseed={seed}\ngenerations={generations}\npareto_elites={}\nelapsed_seconds={:.3}\n",
            if routing_aware {
                "depth,gates,routing_proxy"
            } else {
                "depth,gates"
            },
            archive.len(),
            start.elapsed().as_secs_f64()
        ),
    )
    .expect("write run record");
    println!(
        "requant search finished in {:.1}s with {} exact Pareto elites",
        start.elapsed().as_secs_f64(),
        archive.len()
    );
}

fn evolve_mac_search(
    generations: usize,
    seed: u64,
    outdir: &str,
    allow_fusion: bool,
    diverse: bool,
) {
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::mac::{
        develop_mac, evolve_mac, evolve_mac_diverse, evolve_mac_postadd, verify_mac,
    };
    let start = Instant::now();
    let archive = if diverse {
        evolve_mac_diverse(generations, seed, |line| println!("{line}"))
    } else if allow_fusion {
        evolve_mac(generations, seed, |line| println!("{line}"))
    } else {
        evolve_mac_postadd(generations, seed, |line| println!("{line}"))
    };
    let archive_size = archive.len();
    let archive = if diverse {
        select_mac_physical_tournament(archive, 128)
    } else {
        archive
    };
    std::fs::create_dir_all(outdir).expect("create MAC output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from(
        "module\twidth\tid\tdepth16\tgates16\trouting16\tgeneration\toperator\taccumulator_carry\tfuse_accumulator\tsticky_reduction\tsaturation_reduction\tround_carry\tsaturation_impl\tshare_saturation\tvariant\n",
    );
    let mut elites = archive.clone();
    elites.sort_by_key(|elite| (elite.depth16, elite.gates16, elite.routing16));
    for elite in &elites {
        for width in [4usize, 8, 16] {
            let mut net = develop_mac(&elite.genome, width);
            net.label = format!("mac_requant_evo{}", elite.id);
            verify_mac(
                &net,
                if width == 4 { 0 } else { 200_000 },
                seed ^ width as u64 ^ elite.id as u64,
            )
            .expect("MAC elite failed independent verification");
            let module = net.module_name();
            std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                .expect("write MAC elite");
            manifest.push_str(&format!(
                "{module}\t{width}\t{}\t{}\t{}\t{}\t{}\t{}\t{:?}\t{}\t{:?}\t{:?}\t{:?}\t{:?}\t{}\toriginal\n",
                elite.id,
                elite.depth16,
                elite.gates16,
                elite.routing16,
                elite.generation,
                elite.operator,
                elite.genome.accumulator_carry,
                elite.genome.fuse_accumulator,
                elite.genome.requant.sticky_reduction,
                elite.genome.requant.saturation_reduction,
                elite.genome.requant.round_carry,
                elite.genome.requant.saturation,
                elite.genome.requant.share_saturation_predicate,
            ));
        }
        if diverse && !elite.genome.arithmetic.rules.is_empty() {
            let mut ablated = elite.genome.clone();
            ablated.arithmetic.rules.clear();
            let ablated16 = develop_mac(&ablated, 16);
            let counts = ablated16.counts();
            let routing = ablated16.routing_proxy().score;
            for width in [4usize, 8, 16] {
                let mut net = develop_mac(&ablated, width);
                net.label = format!("mac_requant_evo{}_norules", elite.id);
                verify_mac(
                    &net,
                    if width == 4 { 0 } else { 200_000 },
                    seed ^ width as u64 ^ elite.id as u64 ^ 0xab1a_7100,
                )
                .expect("MAC no-rule ablation failed independent verification");
                let module = net.module_name();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write MAC no-rule ablation");
                manifest.push_str(&format!(
                    "{module}\t{width}\t{}\t{}\t{}\t{}\t{}\tno_rules_ablation\t{:?}\t{}\t{:?}\t{:?}\t{:?}\t{:?}\t{}\tno_rules\n",
                    elite.id,
                    counts.gate_depth,
                    counts.simple_gates,
                    routing,
                    elite.generation,
                    ablated.accumulator_carry,
                    ablated.fuse_accumulator,
                    ablated.requant.sticky_reduction,
                    ablated.requant.saturation_reduction,
                    ablated.requant.round_carry,
                    ablated.requant.saturation,
                    ablated.requant.share_saturation_predicate,
                ));
            }
        }
        std::fs::write(
            format!("{outdir}/mac_evo{}.json", elite.id),
            format!(
                "{{\"arithmetic\":{},\"accumulator_carry\":\"{:?}\",\"fuse_accumulator\":{},\"sticky_reduction\":\"{:?}\",\"saturation_reduction\":\"{:?}\",\"round_carry\":\"{:?}\",\"saturation_impl\":\"{:?}\",\"share_saturation\":{}}}",
                elite.genome.arithmetic.to_json(),
                elite.genome.accumulator_carry,
                elite.genome.fuse_accumulator,
                elite.genome.requant.sticky_reduction,
                elite.genome.requant.saturation_reduction,
                elite.genome.requant.round_carry,
                elite.genome.requant.saturation,
                elite.genome.requant.share_saturation_predicate,
            ),
        )
        .expect("write MAC genome");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "contract=unsigned_mac_requant\noutputs=accumulator,result,saturated,status\none_genome=true\none_circuit=true\naccumulator_fusion={}\nselection={}\nobjectives={}\nverification=exhaustive_w4,sampled_w8_w16\nseed={seed}\ngenerations={generations}\narchive_elites={archive_size}\nphysical_tournament_elites={}\nelapsed_seconds={:.3}\n",
            allow_fusion,
            if diverse { "map_elites" } else { "pareto" },
            if diverse { "depth,gates,topology_diversity;physical_mapping_deferred" } else { "depth,gates,routing_proxy" },
            archive.len(),
            start.elapsed().as_secs_f64()
        ),
    )
    .expect("write run record");
    println!(
        "MAC/requant search finished in {:.1}s with {} exact physical-tournament elites",
        start.elapsed().as_secs_f64(),
        archive.len()
    );
}

fn select_mac_physical_tournament(
    mut archive: Vec<tinytapeout2_search::mac::MacElite>,
    limit: usize,
) -> Vec<tinytapeout2_search::mac::MacElite> {
    use std::collections::BTreeSet;
    if archive.len() <= limit {
        return archive;
    }

    // Never discard a depth/gate Pareto point. The routing proxy is
    // deliberately absent: calibration showed that minimizing it selects in
    // the wrong direction for timing on this workload.
    let mut selected = BTreeSet::new();
    for candidate in &archive {
        let dominated = archive.iter().any(|other| {
            other.depth16 <= candidate.depth16
                && other.gates16 <= candidate.gates16
                && (other.depth16 < candidate.depth16 || other.gates16 < candidate.gates16)
        });
        if !dominated {
            selected.insert(candidate.id);
        }
    }

    // Fill the remaining budget by evenly sampling the stable, fully expanded
    // genome ordering. This preserves distant topology/output-policy families
    // instead of taking only neighbors of the cheapest graph.
    archive.sort_by_key(|elite| format!("{:?}", elite.genome));
    let remaining: Vec<_> = archive
        .iter()
        .filter(|elite| !selected.contains(&elite.id))
        .map(|elite| elite.id)
        .collect();
    let slots = limit.saturating_sub(selected.len()).min(remaining.len());
    for index in 0..slots {
        selected.insert(remaining[index * remaining.len() / slots]);
    }
    archive
        .into_iter()
        .filter(|elite| selected.contains(&elite.id))
        .collect()
}

fn gen_mac_controls(outdir: &str) {
    use tinytapeout2_search::evo::reference_corpus;
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::mac::{develop_mac, verify_mac, AccumulatorCarry, MacGenome};
    use tinytapeout2_search::multiroot::{ReductionShape, RoundCarry, SaturationImpl};
    use tinytapeout2_search::workload::RequantPlan;
    std::fs::create_dir_all(outdir).expect("create MAC control directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let plans = [
        (
            "fused_fast_shared",
            true,
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Balanced,
                round_carry: RoundCarry::Prefix,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: true,
            },
        ),
        (
            "fused_fast_split",
            true,
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Balanced,
                round_carry: RoundCarry::Prefix,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: false,
            },
        ),
        (
            "fused_small",
            true,
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Linear,
                round_carry: RoundCarry::Ripple,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: true,
            },
        ),
        (
            "post_small_split",
            false,
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Linear,
                round_carry: RoundCarry::Ripple,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: false,
            },
        ),
    ];
    let mut manifest = String::from(
        "module\twidth\treduction\tadder\tkind\tplan\tfused\tdepth_proxy\tgates_proxy\trouting_proxy\n",
    );
    for (control, arithmetic) in reference_corpus() {
        let mut fields = control.split('_');
        let reduction = fields.next().expect("control reduction");
        let adder = fields.next().expect("control adder");
        for (plan_name, fuse_accumulator, requant) in plans {
            let genome = MacGenome {
                arithmetic: arithmetic.clone(),
                requant,
                accumulator_carry: AccumulatorCarry::Ripple,
                fuse_accumulator,
            };
            for width in [8usize, 16] {
                let mut net = develop_mac(&genome, width);
                net.label = format!("mac_{plan_name}_ctrl_{control}").to_ascii_lowercase();
                verify_mac(&net, 1_000, 0x6d61_6363 ^ width as u64)
                    .expect("MAC control failed verification");
                let module = net.module_name();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write MAC control");
                let counts = net.counts();
                manifest.push_str(&format!(
                    "{module}\t{width}\t{reduction}\t{adder}\ttextbook_complete\t{plan_name}\t{fuse_accumulator}\t{}\t{}\t{}\n",
                    counts.gate_depth,
                    counts.simple_gates,
                    net.routing_proxy().score
                ));
            }
        }
    }
    for width in [8usize, 16] {
        let shift = width / 2;
        let module = format!("mac{width}_requant_behavioral");
        let verilog = format!(
            "module {module}(input wire [{wm1}:0] a, input wire [{wm1}:0] b, input wire [{twom1}:0] aux, output wire [{outm1}:0] y);\n  wire [{two}:0] sum = a * b + aux;\n  wire [{two}:0] rounded = sum + {ext}'d{bias};\n  wire [{two}:0] scaled = rounded >> {shift};\n  wire sat = |scaled[{two}:{width}];\n  wire [{wm1}:0] result = sat ? {{{width}{{1'b1}}}} : scaled[{wm1}:0];\n  wire status = sat | sum[{two}] | (|sum[{sm1}:0]);\n  assign y = {{status, sat, result, sum}};\nendmodule\n",
            wm1 = width - 1,
            twom1 = 2 * width - 1,
            two = 2 * width,
            outm1 = 3 * width + 2,
            ext = 2 * width + 1,
            bias = 1usize << (shift - 1),
            sm1 = shift - 1,
        );
        std::fs::write(format!("{outdir}/{module}.v"), verilog).expect("write MAC behavioral");
        manifest.push_str(&format!(
            "{module}\t{width}\tbehavioral\tshared\tbehavioral\t-\t-\t-\t-\t-\n"
        ));
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote complete MAC controls for four shortlisted plans to {outdir}");
}

fn gen_requant_controls(outdir: &str) {
    use tinytapeout2_search::evo::reference_corpus;
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::multiroot::{ReductionShape, RoundCarry, SaturationImpl};
    use tinytapeout2_search::workload::{
        develop_requant, verify_requant, RequantGenome, RequantPlan,
    };
    std::fs::create_dir_all(outdir).expect("create requant control directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut manifest = String::from("module\twidth\treduction\tadder\tkind\tplan\trouting_proxy\n");
    let plans = [
        (
            "fast",
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Balanced,
                round_carry: RoundCarry::Prefix,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: true,
            },
        ),
        (
            "mid",
            RequantPlan {
                sticky_reduction: ReductionShape::Balanced,
                saturation_reduction: ReductionShape::Linear,
                round_carry: RoundCarry::Ripple,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: true,
            },
        ),
        (
            "small",
            RequantPlan {
                sticky_reduction: ReductionShape::Linear,
                saturation_reduction: ReductionShape::Linear,
                round_carry: RoundCarry::Ripple,
                saturation: SaturationImpl::OrMask,
                share_saturation_predicate: true,
            },
        ),
    ];
    for (control, arithmetic) in reference_corpus() {
        let mut fields = control.split('_');
        let reduction = fields.next().expect("control reduction");
        let adder = fields.next().expect("control adder");
        for (plan_name, plan) in plans {
            let genome = RequantGenome {
                arithmetic: arithmetic.clone(),
                plan,
            };
            for width in [8usize, 16] {
                let mut net = develop_requant(&genome, width);
                net.label = format!("requant_{plan_name}_ctrl_{control}").to_ascii_lowercase();
                verify_requant(
                    &net,
                    200_000,
                    0x7265_7163 ^ width as u64 ^ plan_name.len() as u64,
                )
                .expect("requant control failed verification");
                let module = net.module_name();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write requant control");
                manifest.push_str(&format!(
                    "{module}\t{width}\t{reduction}\t{adder}\ttextbook_complete\t{plan_name}\t{}\n",
                    net.routing_proxy().score
                ));
            }
        }
    }
    for width in [8usize, 16] {
        let shift = width / 2;
        let module = format!("requant{width}_behavioral");
        let verilog = format!(
            "module {module}(input wire [{wm1}:0] a, input wire [{wm1}:0] b, output wire [{wp1}:0] y);\n  wire [{twom1}:0] p = a * b;\n  wire [{two}:0] rounded = {{1'b0, p}} + {ext}'d{bias};\n  wire [{two}:0] scaled = rounded >> {shift};\n  wire sat = |scaled[{two}:{width}];\n  wire [{wm1}:0] result = sat ? {{{width}{{1'b1}}}} : scaled[{wm1}:0];\n  wire sticky = |p[{sm1}:0];\n  assign y = {{sat | sticky, sat, result}};\nendmodule\n",
            wm1 = width - 1,
            wp1 = width + 1,
            twom1 = 2 * width - 1,
            two = 2 * width,
            ext = 2 * width + 1,
            bias = 1usize << (shift - 1),
            sm1 = shift - 1,
        );
        std::fs::write(format!("{outdir}/{module}.v"), verilog).expect("write behavioral control");
        manifest.push_str(&format!(
            "{module}\t{width}\tbehavioral\tshared\tbehavioral\t-\t-\n"
        ));
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("wrote complete requant controls for all shortlisted root plans to {outdir}");
}

fn evolve_operation_family_search(generations: usize, seed: u64, outdir: &str) {
    use tinytapeout2_search::evo::{
        archive_tsv, develop_operation_family, evolve_operation_families, verify_operation_family,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let start = Instant::now();
    let archives = evolve_operation_families(generations, seed, |line| println!("{line}"));
    std::fs::create_dir_all(outdir).expect("create joint-search output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    let mut summary = String::from(
        "mask\toperations\tevaluated\tniches\tbest_id\tdepth16\tgates16\tcontrol_id\tcontrol_depth16\tcontrol_gates16\tprior_art_distance\n",
    );
    let mut manifest = String::from(
        "module\twidth\tmask\toperations\trole\tid\tcells\tsimple_gates\tcell_depth\tgate_depth\n",
    );
    for archive in &archives {
        let mask = archive.operation_mask.expect("operation-family mask");
        let name = operation_mask_name(mask);
        let mask_dir = format!("{outdir}/mask_{mask:02x}");
        std::fs::create_dir_all(&mask_dir).expect("create semantic-island directory");
        std::fs::write(format!("{mask_dir}/archive.tsv"), archive_tsv(archive))
            .expect("write semantic archive");
        let best = archive.best().expect("seeded semantic island");
        let control = archive
            .elites
            .values()
            .filter(|elite| elite.generation == 0)
            .min_by_key(|elite| elite.score)
            .expect("seeded textbook control");
        std::fs::write(
            format!("{mask_dir}/family_evo{}.json", best.id),
            best.genome.to_json(),
        )
        .expect("write family genome");
        summary.push_str(&format!(
            "0x{mask:02x}\t{name}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\n",
            archive.evaluated,
            archive.elites.len(),
            best.id,
            best.score.depth_top,
            best.score.gates_top,
            control.id,
            control.score.depth_top,
            control.score.gates_top,
            best.prior_art_distance
        ));
        for (role, elite) in [("evolved", best), ("control", control)] {
            for width in [8usize, 16] {
                let (mut net, _) = develop_operation_family(&elite.genome, width, mask);
                net.label = format!("family_{name}_{role}{}", elite.id);
                verify_operation_family(&net, mask, 200_000, seed ^ mask as u64 ^ width as u64)
                    .expect("joint-search output failed verification");
                let module = net.module_name();
                std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                    .expect("write joint-search output");
                let counts = net.counts();
                manifest.push_str(&format!(
                    "{module}\t{width}\t0x{mask:02x}\t{name}\t{role}\t{}\t{}\t{}\t{}\t{}\n",
                    elite.id, counts.cells, counts.simple_gates, counts.depth, counts.gate_depth
                ));
            }
        }
    }
    std::fs::write(format!("{outdir}/summary.tsv"), summary).expect("write joint summary");
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write joint manifest");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "seed={seed}\ngenerations={generations}\nsemantic_islands={}\nminimum_outputs=3\nearly_stop=false\nelapsed_seconds={:.3}\n",
            archives.len(),
            start.elapsed().as_secs_f64()
        ),
    )
    .expect("write joint run manifest");
    println!(
        "joint operation/topology search finished in {:.1}s across {} persistent semantic islands (3+ outputs)",
        start.elapsed().as_secs_f64(),
        archives.len()
    );
}

/// Search and emit exact wrapping-low multipliers. Unlike `evolve-cores`, all
/// selection happens after the upper arithmetic cone has been pruned.
fn evolve_specialized_cores(
    generations: usize,
    seed: u64,
    outdir: &str,
    emit_count: usize,
    mode: &str,
) {
    use tinytapeout2_search::evo::{
        archive_tsv, develop_low_product, develop_low_saturating_family,
        develop_rounded_fractional, develop_saturating, evolve_low, evolve_low_sat_family,
        evolve_rounded, evolve_saturating, verify_low_product, verify_low_saturating_family,
        verify_rounded_fractional, verify_saturating, HELD_OUT_WIDTHS, SCALING_WIDTH,
        TRAINED_WIDTHS,
    };
    use tinytapeout2_search::gen::CELLS_VERILOG;
    let start = Instant::now();
    let archive = match mode {
        "low" => evolve_low(generations, seed, |line| println!("{line}")),
        "rounded" => evolve_rounded(generations, seed, |line| println!("{line}")),
        "saturating" => evolve_saturating(generations, seed, |line| println!("{line}")),
        "low_sat_family" => evolve_low_sat_family(generations, seed, |line| println!("{line}")),
        _ => panic!("unknown specialized mode {mode}"),
    };
    println!(
        "{mode} search finished in {:.1}s: {} niches from {} evaluations, best {:?}",
        start.elapsed().as_secs_f64(),
        archive.elites.len(),
        archive.evaluated,
        archive.best().map(|elite| elite.score)
    );
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/archive.tsv"), archive_tsv(&archive)).expect("write archive");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    std::fs::write(
        format!("{outdir}/run.txt"),
        format!(
            "mode={mode}\nseed={seed}\ngenerations={generations}\ntrained={:?}\nheld_out={:?}\nscaling={}\n",
            TRAINED_WIDTHS, HELD_OUT_WIDTHS, SCALING_WIDTH,
        ),
    )
    .expect("write run manifest");

    let mut chosen: Vec<usize> = archive.pareto().iter().map(|elite| elite.id).collect();
    if let Some(best) = archive.best() {
        if !chosen.contains(&best.id) {
            chosen.push(best.id);
        }
    }
    let mut by_distance: Vec<_> = archive.elites.values().collect();
    by_distance.sort_by(|a, b| {
        b.prior_art_distance
            .partial_cmp(&a.prior_art_distance)
            .unwrap()
    });
    for elite in by_distance {
        if chosen.len() >= emit_count {
            break;
        }
        if !chosen.contains(&elite.id) {
            chosen.push(elite.id);
        }
    }
    let mut manifest = String::from(
        "module\twidth\tid\tcells\tsimple_gates\tcell_depth\tgate_depth\tniche\tprior_art_distance\n",
    );
    let mut widths = TRAINED_WIDTHS.to_vec();
    widths.extend(HELD_OUT_WIDTHS);
    widths.push(SCALING_WIDTH);
    for elite in archive
        .elites
        .values()
        .filter(|elite| chosen.contains(&elite.id))
    {
        let label = format!("{mode}_evo{}", elite.id);
        std::fs::write(format!("{outdir}/{label}.json"), elite.genome.to_json())
            .expect("write genome");
        for &width in &widths {
            let (mut net, _) = match mode {
                "low" => develop_low_product(&elite.genome, width),
                "rounded" => develop_rounded_fractional(&elite.genome, width),
                "saturating" => develop_saturating(&elite.genome, width),
                "low_sat_family" => develop_low_saturating_family(&elite.genome, width),
                _ => unreachable!(),
            };
            net.label = label.clone();
            let verdict = match mode {
                "low" => verify_low_product(&net, 200_000, seed ^ width as u64),
                "rounded" => verify_rounded_fractional(&net, 200_000, seed ^ width as u64),
                "saturating" => verify_saturating(&net, 200_000, seed ^ width as u64),
                "low_sat_family" => {
                    verify_low_saturating_family(&net, 200_000, seed ^ width as u64)
                }
                _ => unreachable!(),
            };
            verdict.expect("emitted specialized elite failed verification");
            let name = net.module_name();
            std::fs::write(format!("{outdir}/{name}.v"), net.to_verilog()).expect("write core");
            let counts = net.counts();
            manifest.push_str(&format!(
                "{name}\t{width}\t{}\t{}\t{}\t{}\t{}\t{}\t{:.3}\n",
                elite.id,
                counts.cells,
                counts.simple_gates,
                counts.depth,
                counts.gate_depth,
                elite.niche,
                elite.prior_art_distance
            ));
        }
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
    println!("emitted {} {mode} elites to {outdir}", chosen.len());
}

/// Freeze a candidate, its no-rule sibling, and all trunks with the same root plan.
fn gen_mac_candidate(path: &str, outdir: &str, widths: &[String]) {
    use tinytapeout2_search::evo::reference_corpus;
    use tinytapeout2_search::gen::CELLS_VERILOG;
    use tinytapeout2_search::mac::{develop_mac, verify_mac, MacGenome};
    let json = std::fs::read_to_string(path).expect("read MAC genome");
    let genome = MacGenome::from_json(&json).expect("parse MAC genome");
    let widths: Vec<usize> = if widths.is_empty() {
        vec![12, 24]
    } else {
        widths
            .iter()
            .map(|w| w.parse().expect("integer width"))
            .collect()
    };
    assert!(
        widths
            .iter()
            .all(|w| (2..=32).contains(w) && w.is_multiple_of(2)),
        "widths must be even and in 2..=32"
    );
    std::fs::create_dir_all(outdir).expect("create output directory");
    std::fs::write(format!("{outdir}/cells.v"), CELLS_VERILOG).expect("write cells");
    std::fs::write(format!("{outdir}/genome.json"), json).expect("freeze genome");
    let mut ablation = genome.clone();
    ablation.arithmetic.rules.clear();
    let mut forms = vec![
        ("candidate".to_string(), "candidate", genome.clone()),
        ("norules".to_string(), "ablation", ablation),
    ];
    for (name, arithmetic) in reference_corpus() {
        let mut control = genome.clone();
        control.arithmetic = arithmetic;
        forms.push((
            format!("ctrl_{name}").to_ascii_lowercase(),
            "control",
            control,
        ));
    }
    let mut manifest = String::from("module\twidth\tclass\tsamples\tseed\n");
    for width in widths {
        for (label, class, form) in &forms {
            let mut net = develop_mac(form, width);
            net.label = format!("mac_replay_{label}");
            let samples = if *class == "control" { 1_000 } else { 200_000 };
            let seed = 0x686f_6c64 ^ width as u64;
            verify_mac(&net, samples, seed).expect("MAC replay verification");
            let module = net.module_name();
            std::fs::write(format!("{outdir}/{module}.v"), net.to_verilog())
                .expect("write netlist");
            manifest.push_str(&format!("{module}\t{width}\t{class}\t{samples}\t{seed}\n"));
        }
        println!("verified and emitted MAC replay and 30 matched trunks at W={width}");
    }
    std::fs::write(format!("{outdir}/manifest.tsv"), manifest).expect("write manifest");
}
