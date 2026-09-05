use tinytapeout2_search::{
    baseline_mixed, baseline_nibble_xor, bdd_synthesize, circuit_from_yosys_json, input_signatures,
    named_target, target_mixed, target_nibble_xor, Circuit, Rng, INPUTS, ROWS, TARGET_NAMES,
};

#[test]
fn every_input_signature_is_balanced() {
    for signature in input_signatures() {
        assert_eq!(
            signature.iter().map(|word| word.count_ones()).sum::<u32>(),
            (ROWS / 2) as u32
        );
    }
}

#[test]
fn known_mixed_baseline_is_exact_and_uses_twelve_nodes() {
    let score = baseline_mixed(32).score(&target_mixed());
    assert!(score.exact());
    assert_eq!(score.active_nodes, 12);
    assert_eq!(score.depth, 3);
}

#[test]
fn bit_parallel_and_independent_scalar_evaluators_agree() {
    let mut rng = Rng::new(42);
    let input_signatures = input_signatures();
    for _ in 0..100 {
        let circuit = Circuit::random(32, &mut rng);
        let parallel = circuit.signatures(&input_signatures);
        for row in 0..ROWS {
            let inputs = std::array::from_fn::<_, INPUTS, _>(|i| ((row >> i) & 1) != 0);
            let scalar = circuit.evaluate_row(inputs);
            for output in 0..scalar.len() {
                let parallel_bit = ((parallel[output][row / 64] >> (row % 64)) & 1) != 0;
                assert_eq!(parallel_bit, scalar[output]);
            }
        }
    }
}

#[test]
fn canonical_key_ignores_unused_nodes_and_commutative_input_order() {
    let a = baseline_nibble_xor(24);
    let mut b = a.clone();
    b.nodes[0].a = 4;
    b.nodes[0].b = 0;
    b.nodes[23].op = tinytapeout2_search::Op::Mux;
    assert_eq!(a.canonical_key(), b.canonical_key());
}

#[test]
fn bdd_synthesis_is_exact_for_every_named_target() {
    for &name in TARGET_NAMES {
        let target = named_target(name).unwrap();
        assert!(bdd_synthesize(&target).score(&target).exact(), "{name}");
    }
}

#[test]
fn specialized_multiplier_targets_have_the_declared_semantics() {
    let rounded = named_target("mul_q4_round").unwrap();
    let saturated = named_target("mul_sat4").unwrap();
    let bit = |target: &[[u64; 4]; 4], row: usize, output: usize| {
        ((target[output][row / 64] >> (row % 64)) & 1) as u8
    };
    let value = |target: &[[u64; 4]; 4], row: usize| {
        (0..4)
            .map(|output| bit(target, row, output) << output)
            .sum::<u8>()
    };

    for row in 0..ROWS {
        let a = (row & 15) as u8;
        let b = ((row >> 4) & 15) as u8;
        let product = a as u16 * b as u16;
        assert_eq!(value(&rounded, row), ((product + 8) >> 4) as u8);
        assert_eq!(value(&saturated, row), product.min(15) as u8);
    }
}

#[test]
fn known_xor_baseline_is_exact_and_uses_four_nodes() {
    let score = baseline_nibble_xor(24).score(&target_nibble_xor());
    assert!(score.exact());
    assert_eq!(score.active_nodes, 4);
    assert_eq!(score.depth, 1);
}

#[test]
fn compact_fabric_mapper_preserves_all_rows() {
    for circuit in [baseline_nibble_xor(24), baseline_mixed(32)] {
        for mapped in [
            circuit
                .map_to_compact_fabric()
                .expect("control must fit greedy 6x8 fabric"),
            circuit
                .map_to_compact_fabric_scheduled(100, 42)
                .expect("control must fit scheduled 6x8 fabric"),
        ] {
            for row in 0..ROWS {
                let inputs = std::array::from_fn::<_, INPUTS, _>(|i| ((row >> i) & 1) != 0);
                assert_eq!(mapped.evaluate_row(inputs), circuit.evaluate_row(inputs));
            }
        }
    }
}

#[test]
fn post_fabric_constraint_is_four_by_four() {
    let circuit = baseline_nibble_xor(4);
    let mapped = circuit
        .map_to_post_fabric(100, 42)
        .expect("four independent XORs must fit the post fabric");
    assert_eq!(mapped.layers.len(), 4);
    assert!(mapped.layers.iter().all(|layer| layer.len() == 4));
}

#[test]
fn zero_node_postprocessor_is_a_valid_terminal_seed() {
    use tinytapeout2_search::{named_target, refine_for_fabric_shape, Circuit};

    let circuit = Circuit {
        nodes: Vec::new(),
        outputs: [0, 1, 2, 3],
    };
    let target = named_target("post_lo").unwrap();
    let (_, score, mapped, accepted, rejected) =
        refine_for_fabric_shape(&circuit, &target, 100, 42, 4, 4);

    assert!(score.exact());
    assert_eq!(mapped.useful_nodes + mapped.pass_nodes, 0);
    assert_eq!(accepted, 0);
    assert_eq!(rejected, 0);
}

#[test]
fn imports_a_yosys_gate_netlist() {
    let json = r#"{
      "modules":{"top":{
        "ports":{
          "x":{"bits":[2,3,4,5,6,7,8,9]},
          "y":{"bits":[10,10,10,10]}
        },
        "cells":{"gate":{
          "type":"$_AND_",
          "connections":{"A":[2],"B":[3],"Y":[10]}
        }}
      }}
    }"#;
    let circuit = circuit_from_yosys_json(json, "top").unwrap();
    for row in 0..ROWS {
        let inputs = std::array::from_fn::<_, INPUTS, _>(|i| ((row >> i) & 1) != 0);
        assert_eq!(circuit.evaluate_row(inputs), [inputs[0] & inputs[1]; 4]);
    }
}
