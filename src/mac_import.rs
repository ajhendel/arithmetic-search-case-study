//! Import a frozen external compressor graph while sharing our CPA and MAC roots.
use crate::evo::FaImpl;
use crate::gen::{final_prefix, split_rows, Adder, Netlist, Reduction, Wire};
use crate::mac::{finish_mac_roots, MacGenome};
use serde_json::Value;
use std::collections::HashMap;

/// Structural correctness must additionally pass the independent composed proof.
/// Names are resolved in order; malformed, duplicate, and forward drivers fail.
pub fn import_mac(text: &str, genome: &MacGenome, adder: Adder, fa: FaImpl) -> Netlist {
    let graph: Value = serde_json::from_str(text).expect("graph JSON");
    let width = graph["width"].as_u64().expect("width") as usize;
    assert!((4..=32).contains(&width) && width.is_multiple_of(2));
    assert!(genome.fuse_accumulator);
    assert_ne!(adder, Adder::Ripple);
    let mut net = Netlist::new(width, Reduction::Dadda, adder);
    net.aux_width = 2 * width;
    let mut wires = HashMap::new();
    for i in 0..width {
        wires.insert(format!("a{i}"), Wire::A(i));
        wires.insert(format!("b{i}"), Wire::B(i));
    }
    for i in 0..2 * width {
        wires.insert(format!("aux{i}"), Wire::Aux(i));
    }
    for cell in graph["cells"].as_array().expect("cells") {
        let inputs: Vec<_> = cell["inputs"]
            .as_array()
            .expect("inputs")
            .iter()
            .map(|n| wires[n.as_str().expect("input name")])
            .collect();
        let outputs = match (cell["kind"].as_str().unwrap(), inputs.as_slice()) {
            ("and", [a, b]) => vec![net.and2(*a, *b)],
            ("ha", [a, b]) => {
                let (s, c) = net.ha(*a, *b);
                vec![s, c]
            }
            ("fa", [a, b, c]) => {
                let (s, c) = match fa {
                    FaImpl::Mux => net.fa_mux(*a, *b, *c),
                    FaImpl::XorMaj => net.fa(*a, *b, *c),
                };
                vec![s, c]
            }
            _ => panic!("unsupported compressor or arity"),
        };
        let names = cell["outputs"].as_array().expect("outputs");
        assert_eq!(outputs.len(), names.len());
        for (name, wire) in names.iter().zip(outputs) {
            assert!(wires
                .insert(name.as_str().unwrap().to_string(), wire)
                .is_none());
        }
    }
    let rows: Vec<Vec<Wire>> = graph["rows"]
        .as_array()
        .expect("rows")
        .iter()
        .map(|col| {
            col.as_array()
                .unwrap()
                .iter()
                .map(|n| wires[n.as_str().unwrap()])
                .collect()
        })
        .collect();
    assert_eq!(rows.len(), 2 * width + 1);
    let (a, b) = split_rows(rows);
    let sum = final_prefix(&mut net, &a, &b, adder);
    finish_mac_roots(net, sum, genome)
}
