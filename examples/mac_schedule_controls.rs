//! Bounded no-rule arrival-aware controls; no evolutionary search.
use std::fmt::Write as _;
use tinytapeout2_search::evo::{Action, FaImpl, Pick};
use tinytapeout2_search::gen::Adder;
use tinytapeout2_search::mac::{develop_mac, verify_mac, MacGenome};

fn main() {
    let args: Vec<_> = std::env::args().collect();
    assert_eq!(args.len(), 3, "genome.json output-directory");
    let json = std::fs::read_to_string(&args[1]).unwrap();
    let genome = MacGenome::from_json(&json).unwrap();
    let saved: serde_json::Value = serde_json::from_str(&json).unwrap();
    let out = std::path::Path::new(&args[2]);
    assert!(!out.exists(), "preserve existing control outputs");
    std::fs::create_dir_all(out).unwrap();
    let mut manifest = String::from("module\tclass\taction\tpick\tadder\tfa_impl\tsamples\n");
    for action in [Action::Dadda, Action::FullNoHa] {
        for adder in [Adder::Sklansky, Adder::BrentKung, Adder::KoggeStone] {
            for fa_impl in [FaImpl::Mux, FaImpl::XorMaj] {
                let mut form = genome.clone();
                form.arithmetic.rules.clear();
                form.arithmetic.default_action = action;
                form.arithmetic.default_pick = Pick::Earliest;
                form.arithmetic.adder = adder;
                form.arithmetic.fa_impl = fa_impl;
                let label = format!("mac_arrival_{action:?}_{}_{fa_impl:?}", adder.name())
                    .to_ascii_lowercase();
                let mut net = develop_mac(&form, 24);
                net.label = label;
                verify_mac(&net, 1_000, 0x6172_7269_7661_6c24).unwrap();
                let module = net.module_name();
                std::fs::write(out.join(format!("{module}.v")), net.to_verilog()).unwrap();
                let mut form_json = saved.clone();
                form_json["arithmetic"] = serde_json::from_str(&form.arithmetic.to_json()).unwrap();
                std::fs::write(out.join(format!("{module}.json")), form_json.to_string()).unwrap();
                writeln!(
                    manifest,
                    "{module}\tarrival_control\t{action:?}\tEarliest\t{}\t{fa_impl:?}\t1000",
                    adder.name()
                )
                .unwrap();
            }
        }
    }
    std::fs::write(out.join("manifest.tsv"), manifest).unwrap();
    println!("Verified and emitted 12 arrival-aware W=24 controls");
}
