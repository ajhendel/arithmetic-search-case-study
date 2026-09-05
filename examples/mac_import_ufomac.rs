use tinytapeout2_search::evo::FaImpl;
use tinytapeout2_search::gen::Adder;
use tinytapeout2_search::mac::{develop_mac, verify_mac, MacGenome};
use tinytapeout2_search::mac_import::import_mac;

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(
        args.len(),
        4,
        "genome.json graph.json fresh-output-directory"
    );
    let genome = MacGenome::from_json(&std::fs::read_to_string(&args[1]).unwrap()).unwrap();
    let graph = std::fs::read_to_string(&args[2]).unwrap();
    let out = std::path::Path::new(&args[3]);
    assert!(!out.exists());
    std::fs::create_dir_all(out).unwrap();
    // The root refactor must preserve the frozen candidate byte for byte.
    let mut original = develop_mac(&genome, 24);
    original.label = "mac_replay_candidate".to_string();
    let frozen = std::fs::read_to_string(
        "results/discovery/mac_crosswidth_20260905/evo608_heldout/mul24_mac_replay_candidate.v",
    )
    .unwrap();
    assert_eq!(original.to_verilog(), frozen);
    for fa in [FaImpl::Mux, FaImpl::XorMaj] {
        for adder in [Adder::Sklansky, Adder::BrentKung, Adder::KoggeStone] {
            let mut net = import_mac(&graph, &genome, adder, fa);
            net.label = format!("mac_ufomac_{}_{fa:?}", adder.name()).to_ascii_lowercase();
            verify_mac(&net, 4096, 0x7566_6f6d_6163_0024).unwrap();
            std::fs::write(
                out.join(format!("{}.v", net.module_name())),
                net.to_verilog(),
            )
            .unwrap();
        }
    }
    println!("Six imported controls verified; original candidate unchanged byte for byte");
}
