pub mod evo;
pub mod gen;
pub mod mac;
pub mod mac_import;
pub mod multiroot;
pub mod workload;

use serde_json::Value;
use std::collections::HashMap;

pub const INPUTS: usize = 8;
pub const OUTPUTS: usize = 4;
pub const WORDS: usize = 4;
pub const ROWS: usize = 256;

pub type Signature = [u64; WORDS];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Op {
    And,
    Or,
    Xor,
    Nand,
    Nor,
    Xnor,
    Mux,
}

impl Op {
    const COUNT: u64 = 7;

    fn from_index(index: u64) -> Self {
        match index % Self::COUNT {
            0 => Self::And,
            1 => Self::Or,
            2 => Self::Xor,
            3 => Self::Nand,
            4 => Self::Nor,
            5 => Self::Xnor,
            _ => Self::Mux,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Node {
    pub op: Op,
    pub a: usize,
    pub b: usize,
    pub c: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Circuit {
    pub nodes: Vec<Node>,
    pub outputs: [usize; OUTPUTS],
}

pub const FABRIC_LAYERS: usize = 6;
pub const FABRIC_WIDTH: usize = 8;
pub const POST_FABRIC_LAYERS: usize = 4;
pub const POST_FABRIC_WIDTH: usize = 4;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FabricNode {
    pub op: Op,
    pub a: usize,
    pub b: usize,
    pub c: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FabricConfig {
    pub layers: Vec<Vec<FabricNode>>,
    pub outputs: [usize; OUTPUTS],
    pub useful_nodes: usize,
    pub pass_nodes: usize,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MapError {
    TooDeep {
        required: usize,
        available: usize,
    },
    LayerOverflow {
        layer: usize,
        required: usize,
        available: usize,
    },
    MissingSource {
        layer: usize,
        source: usize,
    },
    MissingOutput {
        output: usize,
        source: usize,
    },
}

pub fn circuit_from_yosys_json(json: &str, module: &str) -> Result<Circuit, String> {
    let root: Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    let design = root["modules"][module]
        .as_object()
        .ok_or_else(|| format!("module {module:?} not found"))?;
    let ports = design["ports"]
        .as_object()
        .ok_or_else(|| "missing ports".to_string())?;
    let bit_number = |value: &Value| {
        value
            .as_u64()
            .map(|number| number as usize)
            .ok_or_else(|| format!("unsupported constant or bit {value}"))
    };
    let port_bits = |name: &str| -> Result<Vec<usize>, String> {
        ports[name]["bits"]
            .as_array()
            .ok_or_else(|| format!("missing port {name:?}"))?
            .iter()
            .map(bit_number)
            .collect()
    };
    let inputs = port_bits("x")?;
    if inputs.len() != INPUTS {
        return Err(format!(
            "expected {INPUTS} input bits, found {}",
            inputs.len()
        ));
    }
    let output_bits = port_bits("y")?;
    if output_bits.len() != OUTPUTS {
        return Err(format!(
            "expected {OUTPUTS} output bits, found {}",
            output_bits.len()
        ));
    }
    let mut wires: HashMap<usize, usize> = inputs
        .into_iter()
        .enumerate()
        .map(|(source, bit)| (bit, source))
        .collect();
    let mut pending: Vec<_> = design["cells"]
        .as_object()
        .ok_or_else(|| "missing cells".to_string())?
        .values()
        .cloned()
        .collect();
    let mut nodes = Vec::new();

    while !pending.is_empty() {
        let before = pending.len();
        let mut deferred = Vec::new();
        for cell in pending {
            let kind = cell["type"]
                .as_str()
                .ok_or_else(|| "cell missing type".to_string())?;
            let connections = cell["connections"]
                .as_object()
                .ok_or_else(|| "cell missing connections".to_string())?;
            let bit = |port: &str| -> Result<usize, String> {
                bit_number(
                    connections[port]
                        .as_array()
                        .and_then(|bits| bits.first())
                        .ok_or_else(|| format!("cell {kind} missing port {port}"))?,
                )
            };
            let source = |port: &str| -> Option<usize> {
                bit(port).ok().and_then(|id| wires.get(&id).copied())
            };
            let converted = match kind {
                "$_AND_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::And,
                    a,
                    b,
                    c: 0,
                }),
                "$_OR_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::Or,
                    a,
                    b,
                    c: 0,
                }),
                "$_XOR_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::Xor,
                    a,
                    b,
                    c: 0,
                }),
                "$_XNOR_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::Xnor,
                    a,
                    b,
                    c: 0,
                }),
                "$_NAND_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::Nand,
                    a,
                    b,
                    c: 0,
                }),
                "$_NOR_" => source("A").zip(source("B")).map(|(a, b)| Node {
                    op: Op::Nor,
                    a,
                    b,
                    c: 0,
                }),
                "$_NOT_" => source("A").map(|a| Node {
                    op: Op::Nand,
                    a,
                    b: a,
                    c: 0,
                }),
                "$_MUX_" => source("S")
                    .zip(source("B"))
                    .zip(source("A"))
                    .map(|((a, b), c)| Node {
                        op: Op::Mux,
                        a,
                        b,
                        c,
                    }),
                _ => return Err(format!("unsupported Yosys cell {kind}")),
            };
            if let Some(node) = converted {
                let output = bit("Y")?;
                wires.insert(output, INPUTS + nodes.len());
                nodes.push(node);
            } else {
                deferred.push(cell);
            }
        }
        if deferred.len() == before {
            return Err(format!(
                "could not topologically order {} cells",
                deferred.len()
            ));
        }
        pending = deferred;
    }
    let outputs: Vec<usize> = output_bits
        .into_iter()
        .map(|bit| {
            wires
                .get(&bit)
                .copied()
                .ok_or_else(|| format!("undriven output bit {bit}"))
        })
        .collect::<Result<_, _>>()?;
    Ok(Circuit {
        nodes,
        outputs: outputs.try_into().expect("output count checked"),
    })
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FabricRank {
    pub unmappable: usize,
    pub depth: usize,
    pub layer_overflow: usize,
    pub occupied_nodes: usize,
    pub active_nodes: usize,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Score {
    pub correct_bits: u32,
    pub active_nodes: usize,
    pub depth: usize,
    pub max_fanout: usize,
}

impl Score {
    pub fn exact(self) -> bool {
        self.correct_bits as usize == ROWS * OUTPUTS
    }

    pub fn better_than(self, other: Self) -> bool {
        (
            self.correct_bits,
            usize::MAX - self.active_nodes,
            usize::MAX - self.depth,
        ) > (
            other.correct_bits,
            usize::MAX - other.active_nodes,
            usize::MAX - other.depth,
        )
    }
}

#[derive(Clone, Copy, Debug)]
pub struct Rng(u64);

impl Rng {
    pub fn new(seed: u64) -> Self {
        Self(seed.max(1))
    }

    pub fn next_u64(&mut self) -> u64 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 7;
        x ^= x << 17;
        self.0 = x;
        x
    }

    pub fn below(&mut self, upper: usize) -> usize {
        assert!(upper > 0, "random upper bound must be non-zero");
        (self.next_u64() as usize) % upper
    }
}

pub fn input_signatures() -> [Signature; INPUTS] {
    let mut inputs = [[0u64; WORDS]; INPUTS];
    for row in 0..ROWS {
        for (input, signature) in inputs.iter_mut().enumerate() {
            if ((row >> input) & 1) != 0 {
                signature[row / 64] |= 1u64 << (row % 64);
            }
        }
    }
    inputs
}

pub fn target_nibble_xor() -> [Signature; OUTPUTS] {
    let inputs = input_signatures();
    std::array::from_fn(|i| xor(inputs[i], inputs[i + 4]))
}

pub fn target_mixed() -> [Signature; OUTPUTS] {
    let x = input_signatures();
    [
        xor(x[0], x[4]),
        majority3(x[1], x[2], x[5]),
        xnor(xor(x[0], x[1]), xor(x[4], x[5])),
        mux(x[7], and(x[2], x[3]), or(x[5], x[6])),
    ]
}

pub fn named_target(name: &str) -> Option<[Signature; OUTPUTS]> {
    match name {
        "xor" => Some(target_nibble_xor()),
        "mixed" => Some(target_mixed()),
        "add4" => Some(target_from_fn(|row| {
            (((row & 0x0f) + ((row >> 4) & 0x0f)) & 0x0f) as u8
        })),
        "mul_lo" => Some(target_from_fn(|row| {
            (((row & 0x0f) * ((row >> 4) & 0x0f)) & 0x0f) as u8
        })),
        "mul_hi" => Some(target_from_fn(|row| {
            ((((row & 0x0f) * ((row >> 4) & 0x0f)) >> 4) & 0x0f) as u8
        })),
        // Unsigned Q0.4 multiply, rounded to nearest representable 4-bit value.
        // Inputs encode fractions a/16 and b/16; the output encodes
        // round(a*b/16). The maximum is 14, so rounding cannot overflow.
        "mul_q4_round" => Some(target_from_fn(|row| {
            let product = (row & 0x0f) * ((row >> 4) & 0x0f);
            ((product + 8) >> 4) as u8
        })),
        // Four-bit saturating integer multiply. Unlike mul_lo, overflow clamps
        // rather than wrapping, which is useful in bounded DSP/control paths.
        "mul_sat4" => Some(target_from_fn(|row| {
            let product = (row & 0x0f) * ((row >> 4) & 0x0f);
            product.min(15) as u8
        })),
        // Post-processing targets treat the eight inputs as an already
        // computed product. They define the hybrid shared-core boundary.
        "post_lo" => Some(target_from_fn(|product| (product & 0x0f) as u8)),
        "post_hi" => Some(target_from_fn(|product| ((product >> 4) & 0x0f) as u8)),
        "post_q4_round" => Some(target_from_fn(|product| {
            (((product + 8) >> 4) & 0x0f) as u8
        })),
        "post_sat4" => Some(target_from_fn(|product| {
            if product > 15 {
                15
            } else {
                product as u8
            }
        })),
        "popcount" => Some(target_from_fn(|row| (row as u8).count_ones() as u8)),
        "priority" => Some(target_from_fn(|row| {
            let byte = row as u8;
            if byte == 0 {
                0
            } else {
                0b1000 | (7 - byte.leading_zeros() as u8)
            }
        })),
        "compare" => Some(target_from_fn(|row| {
            let a = (row & 0x0f) as u8;
            let b = ((row >> 4) & 0x0f) as u8;
            u8::from(a < b)
                | (u8::from(a == b) << 1)
                | (u8::from(a > b) << 2)
                | (u8::from(sign4(a) < sign4(b)) << 3)
        })),
        "crc4" => Some(target_from_fn(crc4)),
        "aes_lo" => Some(target_from_fn(|row| AES_SBOX[row] & 0x0f)),
        "aes_hi" => Some(target_from_fn(|row| AES_SBOX[row] >> 4)),
        "planted8" => Some(planted_target(8, 0x800d)),
        "planted12" => Some(planted_target(12, 0x1200d)),
        "planted16" => Some(planted_target(16, 0x1600d)),
        "planted24" => Some(planted_target(24, 0x2400d)),
        "planted32" => Some(planted_target(32, 0x3200d)),
        _ => None,
    }
}

pub const TARGET_NAMES: &[&str] = &[
    "xor",
    "mixed",
    "add4",
    "mul_lo",
    "mul_hi",
    "mul_q4_round",
    "mul_sat4",
    "post_lo",
    "post_hi",
    "post_q4_round",
    "post_sat4",
    "popcount",
    "priority",
    "compare",
    "crc4",
    "aes_lo",
    "aes_hi",
    "planted8",
    "planted12",
    "planted16",
    "planted24",
    "planted32",
];

fn target_from_fn(function: impl Fn(usize) -> u8) -> [Signature; OUTPUTS] {
    let mut target = [[0u64; WORDS]; OUTPUTS];
    for row in 0..ROWS {
        let value = function(row);
        for (output, signature) in target.iter_mut().enumerate() {
            if ((value >> output) & 1) != 0 {
                signature[row / 64] |= 1u64 << (row % 64);
            }
        }
    }
    target
}

fn planted_target(nodes: usize, seed: u64) -> [Signature; OUTPUTS] {
    let mut rng = Rng::new(seed);
    let mut circuit = Circuit::random(nodes, &mut rng);
    // Make every output depend on a node in the latter half of the planted DAG.
    for output in &mut circuit.outputs {
        *output = INPUTS + nodes / 2 + rng.below(nodes - nodes / 2);
    }
    circuit.signatures(&input_signatures())
}

fn crc4(row: usize) -> u8 {
    let mut crc = 0u8;
    for bit in (0..8).rev() {
        let feedback = ((crc >> 3) ^ ((row as u8 >> bit) & 1)) & 1;
        crc = (crc << 1) & 0x0f;
        if feedback != 0 {
            crc ^= 0b0011; // x^4 + x + 1
        }
    }
    crc
}

fn sign4(value: u8) -> i8 {
    ((value << 4) as i8) >> 4
}

const AES_SBOX: [u8; 256] = [
    0x63, 0x7c, 0x77, 0x7b, 0xf2, 0x6b, 0x6f, 0xc5, 0x30, 0x01, 0x67, 0x2b, 0xfe, 0xd7, 0xab, 0x76,
    0xca, 0x82, 0xc9, 0x7d, 0xfa, 0x59, 0x47, 0xf0, 0xad, 0xd4, 0xa2, 0xaf, 0x9c, 0xa4, 0x72, 0xc0,
    0xb7, 0xfd, 0x93, 0x26, 0x36, 0x3f, 0xf7, 0xcc, 0x34, 0xa5, 0xe5, 0xf1, 0x71, 0xd8, 0x31, 0x15,
    0x04, 0xc7, 0x23, 0xc3, 0x18, 0x96, 0x05, 0x9a, 0x07, 0x12, 0x80, 0xe2, 0xeb, 0x27, 0xb2, 0x75,
    0x09, 0x83, 0x2c, 0x1a, 0x1b, 0x6e, 0x5a, 0xa0, 0x52, 0x3b, 0xd6, 0xb3, 0x29, 0xe3, 0x2f, 0x84,
    0x53, 0xd1, 0x00, 0xed, 0x20, 0xfc, 0xb1, 0x5b, 0x6a, 0xcb, 0xbe, 0x39, 0x4a, 0x4c, 0x58, 0xcf,
    0xd0, 0xef, 0xaa, 0xfb, 0x43, 0x4d, 0x33, 0x85, 0x45, 0xf9, 0x02, 0x7f, 0x50, 0x3c, 0x9f, 0xa8,
    0x51, 0xa3, 0x40, 0x8f, 0x92, 0x9d, 0x38, 0xf5, 0xbc, 0xb6, 0xda, 0x21, 0x10, 0xff, 0xf3, 0xd2,
    0xcd, 0x0c, 0x13, 0xec, 0x5f, 0x97, 0x44, 0x17, 0xc4, 0xa7, 0x7e, 0x3d, 0x64, 0x5d, 0x19, 0x73,
    0x60, 0x81, 0x4f, 0xdc, 0x22, 0x2a, 0x90, 0x88, 0x46, 0xee, 0xb8, 0x14, 0xde, 0x5e, 0x0b, 0xdb,
    0xe0, 0x32, 0x3a, 0x0a, 0x49, 0x06, 0x24, 0x5c, 0xc2, 0xd3, 0xac, 0x62, 0x91, 0x95, 0xe4, 0x79,
    0xe7, 0xc8, 0x37, 0x6d, 0x8d, 0xd5, 0x4e, 0xa9, 0x6c, 0x56, 0xf4, 0xea, 0x65, 0x7a, 0xae, 0x08,
    0xba, 0x78, 0x25, 0x2e, 0x1c, 0xa6, 0xb4, 0xc6, 0xe8, 0xdd, 0x74, 0x1f, 0x4b, 0xbd, 0x8b, 0x8a,
    0x70, 0x3e, 0xb5, 0x66, 0x48, 0x03, 0xf6, 0x0e, 0x61, 0x35, 0x57, 0xb9, 0x86, 0xc1, 0x1d, 0x9e,
    0xe1, 0xf8, 0x98, 0x11, 0x69, 0xd9, 0x8e, 0x94, 0x9b, 0x1e, 0x87, 0xe9, 0xce, 0x55, 0x28, 0xdf,
    0x8c, 0xa1, 0x89, 0x0d, 0xbf, 0xe6, 0x42, 0x68, 0x41, 0x99, 0x2d, 0x0f, 0xb0, 0x54, 0xbb, 0x16,
];

pub fn baseline_nibble_xor(node_count: usize) -> Circuit {
    assert!(node_count >= OUTPUTS);
    let mut nodes = Vec::with_capacity(node_count);
    for i in 0..OUTPUTS {
        nodes.push(Node {
            op: Op::Xor,
            a: i,
            b: i + 4,
            c: 0,
        });
    }
    while nodes.len() < node_count {
        nodes.push(Node {
            op: Op::And,
            a: 0,
            b: 0,
            c: 0,
        });
    }
    Circuit {
        nodes,
        outputs: [8, 9, 10, 11],
    }
}

pub fn baseline_mixed(node_count: usize) -> Circuit {
    assert!(node_count >= 12);
    let mut nodes = vec![
        Node {
            op: Op::Xor,
            a: 0,
            b: 4,
            c: 0,
        }, // 8
        Node {
            op: Op::And,
            a: 1,
            b: 2,
            c: 0,
        }, // 9
        Node {
            op: Op::And,
            a: 1,
            b: 5,
            c: 0,
        }, // 10
        Node {
            op: Op::And,
            a: 2,
            b: 5,
            c: 0,
        }, // 11
        Node {
            op: Op::Or,
            a: 9,
            b: 10,
            c: 0,
        }, // 12
        Node {
            op: Op::Or,
            a: 12,
            b: 11,
            c: 0,
        }, // 13 majority
        Node {
            op: Op::Xor,
            a: 0,
            b: 1,
            c: 0,
        }, // 14
        Node {
            op: Op::Xor,
            a: 4,
            b: 5,
            c: 0,
        }, // 15
        Node {
            op: Op::Xnor,
            a: 14,
            b: 15,
            c: 0,
        }, // 16 equivalence
        Node {
            op: Op::And,
            a: 2,
            b: 3,
            c: 0,
        }, // 17
        Node {
            op: Op::Or,
            a: 5,
            b: 6,
            c: 0,
        }, // 18
        Node {
            op: Op::Mux,
            a: 7,
            b: 17,
            c: 18,
        }, // 19
    ];
    while nodes.len() < node_count {
        nodes.push(Node {
            op: Op::And,
            a: 0,
            b: 0,
            c: 0,
        });
    }
    Circuit {
        nodes,
        outputs: [8, 13, 16, 19],
    }
}

pub fn bdd_synthesize(target: &[Signature; OUTPUTS]) -> Circuit {
    let mut circuit = Circuit {
        nodes: vec![
            Node {
                op: Op::Xor,
                a: 0,
                b: 0,
                c: 0,
            },
            Node {
                op: Op::Xnor,
                a: 0,
                b: 0,
                c: 0,
            },
        ],
        outputs: [0; OUTPUTS],
    };
    let zero = INPUTS;
    let one = INPUTS + 1;
    let mut memo = HashMap::<(usize, Vec<u8>), usize>::new();

    for (output, signature) in target.iter().enumerate() {
        let values: Vec<u8> = (0..ROWS)
            .map(|row| ((signature[row / 64] >> (row % 64)) & 1) as u8)
            .collect();
        circuit.outputs[output] = bdd_node(
            &values,
            INPUTS - 1,
            zero,
            one,
            &mut circuit.nodes,
            &mut memo,
        );
    }
    circuit
}

fn bdd_node(
    values: &[u8],
    variable: usize,
    zero: usize,
    one: usize,
    nodes: &mut Vec<Node>,
    memo: &mut HashMap<(usize, Vec<u8>), usize>,
) -> usize {
    if values.iter().all(|&value| value == 0) {
        return zero;
    }
    if values.iter().all(|&value| value == 1) {
        return one;
    }
    let key = (variable, values.to_vec());
    if let Some(&source) = memo.get(&key) {
        return source;
    }
    let half = values.len() / 2;
    let low = bdd_node(
        &values[..half],
        variable.saturating_sub(1),
        zero,
        one,
        nodes,
        memo,
    );
    let high = bdd_node(
        &values[half..],
        variable.saturating_sub(1),
        zero,
        one,
        nodes,
        memo,
    );
    if low == high {
        return low;
    }
    let source = INPUTS + nodes.len();
    nodes.push(Node {
        op: Op::Mux,
        a: variable,
        b: high,
        c: low,
    });
    memo.insert(key, source);
    source
}

impl Circuit {
    pub fn random(node_count: usize, rng: &mut Rng) -> Self {
        let nodes = (0..node_count)
            .map(|i| {
                let sources = INPUTS + i;
                Node {
                    op: Op::from_index(rng.next_u64()),
                    a: rng.below(sources),
                    b: rng.below(sources),
                    c: rng.below(sources),
                }
            })
            .collect();
        let source_count = INPUTS + node_count;
        let outputs = std::array::from_fn(|_| rng.below(source_count));
        Self { nodes, outputs }
    }

    pub fn mutate(&mut self, rng: &mut Rng, mutations: usize) {
        for _ in 0..mutations {
            if rng.below(5) == 0 {
                self.outputs[rng.below(OUTPUTS)] = rng.below(INPUTS + self.nodes.len());
                continue;
            }
            let i = rng.below(self.nodes.len());
            let sources = INPUTS + i;
            match rng.below(4) {
                0 => self.nodes[i].op = Op::from_index(rng.next_u64()),
                1 => self.nodes[i].a = rng.below(sources),
                2 => self.nodes[i].b = rng.below(sources),
                _ => self.nodes[i].c = rng.below(sources),
            }
        }
    }

    fn bypass_node(&mut self, index: usize, rng: &mut Rng) {
        let old_source = INPUTS + index;
        let node = self.nodes[index];
        let choices = if node.op == Op::Mux {
            [node.a, node.b, node.c]
        } else {
            [node.a, node.b, node.a]
        };
        let replacement = choices[rng.below(choices.len())];
        for later in self.nodes.iter_mut().skip(index + 1) {
            if later.a == old_source {
                later.a = replacement;
            }
            if later.b == old_source {
                later.b = replacement;
            }
            if later.c == old_source {
                later.c = replacement;
            }
        }
        for output in &mut self.outputs {
            if *output == old_source {
                *output = replacement;
            }
        }
    }

    pub fn signatures(&self, inputs: &[Signature; INPUTS]) -> [Signature; OUTPUTS] {
        let mut values = Vec::with_capacity(INPUTS + self.nodes.len());
        values.extend_from_slice(inputs);
        for node in &self.nodes {
            let a = values[node.a];
            let b = values[node.b];
            let c = values[node.c];
            values.push(match node.op {
                Op::And => and(a, b),
                Op::Or => or(a, b),
                Op::Xor => xor(a, b),
                Op::Nand => not(and(a, b)),
                Op::Nor => not(or(a, b)),
                Op::Xnor => xnor(a, b),
                Op::Mux => mux(a, b, c),
            });
        }
        std::array::from_fn(|i| values[self.outputs[i]])
    }

    pub fn evaluate_row(&self, inputs: [bool; INPUTS]) -> [bool; OUTPUTS] {
        let mut values = Vec::with_capacity(INPUTS + self.nodes.len());
        values.extend_from_slice(&inputs);
        for node in &self.nodes {
            let a = values[node.a];
            let b = values[node.b];
            let c = values[node.c];
            values.push(match node.op {
                Op::And => a & b,
                Op::Or => a | b,
                Op::Xor => a ^ b,
                Op::Nand => !(a & b),
                Op::Nor => !(a | b),
                Op::Xnor => !(a ^ b),
                Op::Mux => {
                    if a {
                        b
                    } else {
                        c
                    }
                }
            });
        }
        std::array::from_fn(|i| values[self.outputs[i]])
    }

    pub fn score(&self, target: &[Signature; OUTPUTS]) -> Score {
        let actual = self.signatures(&input_signatures());
        let correct_bits = actual
            .iter()
            .zip(target)
            .flat_map(|(a, b)| a.iter().zip(b))
            .map(|(a, b)| (!(a ^ b)).count_ones())
            .sum();
        let (active_nodes, depth, max_fanout) = self.structural_metrics();
        Score {
            correct_bits,
            active_nodes,
            depth,
            max_fanout,
        }
    }

    pub fn structural_metrics(&self) -> (usize, usize, usize) {
        let mut active = vec![false; self.nodes.len()];
        let mut stack = self.outputs.to_vec();
        while let Some(source) = stack.pop() {
            if source < INPUTS {
                continue;
            }
            let i = source - INPUTS;
            if active[i] {
                continue;
            }
            active[i] = true;
            let node = self.nodes[i];
            stack.extend([node.a, node.b]);
            if node.op == Op::Mux {
                stack.push(node.c);
            }
        }

        let mut depths = vec![0usize; INPUTS + self.nodes.len()];
        let mut fanout = vec![0usize; INPUTS + self.nodes.len()];
        for (i, node) in self.nodes.iter().enumerate() {
            if !active[i] {
                continue;
            }
            let mut sources = vec![node.a, node.b];
            if node.op == Op::Mux {
                sources.push(node.c);
            }
            depths[INPUTS + i] = 1 + sources.iter().map(|&s| depths[s]).max().unwrap_or(0);
            for source in sources {
                fanout[source] += 1;
            }
        }
        for &output in &self.outputs {
            fanout[output] += 1;
        }
        (
            active.iter().filter(|&&used| used).count(),
            self.outputs.iter().map(|&o| depths[o]).max().unwrap_or(0),
            fanout.into_iter().max().unwrap_or(0),
        )
    }

    pub fn canonical_key(&self) -> String {
        let mut active = vec![false; self.nodes.len()];
        let mut stack = self.outputs.to_vec();
        while let Some(source) = stack.pop() {
            if source < INPUTS {
                continue;
            }
            let i = source - INPUTS;
            if active[i] {
                continue;
            }
            active[i] = true;
            let node = self.nodes[i];
            stack.extend([node.a, node.b]);
            if node.op == Op::Mux {
                stack.push(node.c);
            }
        }

        let mut remap = vec![usize::MAX; INPUTS + self.nodes.len()];
        for (i, slot) in remap.iter_mut().take(INPUTS).enumerate() {
            *slot = i;
        }
        let mut records = Vec::new();
        let mut next = INPUTS;
        for (i, node) in self.nodes.iter().enumerate() {
            if !active[i] {
                continue;
            }
            let mut a = remap[node.a];
            let mut b = remap[node.b];
            if node.op != Op::Mux && a > b {
                std::mem::swap(&mut a, &mut b);
            }
            let c = if node.op == Op::Mux { remap[node.c] } else { 0 };
            records.push(format!("{:?}:{a}:{b}:{c}", node.op));
            remap[INPUTS + i] = next;
            next += 1;
        }
        let outputs: Vec<_> = self.outputs.iter().map(|&o| remap[o]).collect();
        format!("{}=>{outputs:?}", records.join("|"))
    }

    pub fn map_to_compact_fabric(&self) -> Result<FabricConfig, MapError> {
        let mut active = vec![false; self.nodes.len()];
        let mut stack = self.outputs.to_vec();
        while let Some(source) = stack.pop() {
            if source < INPUTS {
                continue;
            }
            let index = source - INPUTS;
            if active[index] {
                continue;
            }
            active[index] = true;
            let node = self.nodes[index];
            stack.extend([node.a, node.b]);
            if node.op == Op::Mux {
                stack.push(node.c);
            }
        }

        let mut depth = vec![0usize; INPUTS + self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if !active[index] {
                continue;
            }
            let mut required = depth[node.a].max(depth[node.b]);
            if node.op == Op::Mux {
                required = required.max(depth[node.c]);
            }
            depth[INPUTS + index] = required + 1;
        }
        let required_depth = self
            .outputs
            .iter()
            .map(|&source| depth[source])
            .max()
            .unwrap_or(0);
        if required_depth > FABRIC_LAYERS {
            return Err(MapError::TooDeep {
                required: required_depth,
                available: FABRIC_LAYERS,
            });
        }

        let mut last_use = vec![0usize; INPUTS + self.nodes.len()];
        for (index, node) in self.nodes.iter().enumerate() {
            if !active[index] {
                continue;
            }
            let consumer_depth = depth[INPUTS + index];
            last_use[node.a] = last_use[node.a].max(consumer_depth);
            last_use[node.b] = last_use[node.b].max(consumer_depth);
            if node.op == Op::Mux {
                last_use[node.c] = last_use[node.c].max(consumer_depth);
            }
        }
        for &source in &self.outputs {
            if source >= INPUTS {
                last_use[source] = FABRIC_LAYERS + 1;
            }
        }

        let dummy = FabricNode {
            op: Op::And,
            a: 0,
            b: 0,
            c: 0,
        };
        let mut layers = Vec::with_capacity(FABRIC_LAYERS);
        let mut previous = HashMap::<usize, usize>::new();
        let mut pass_nodes = 0;

        for layer in 0..FABRIC_LAYERS {
            let logical_depth = layer + 1;
            let mut entries = Vec::<(usize, FabricNode)>::new();

            for (&source, &slot) in &previous {
                if last_use[source] > logical_depth {
                    entries.push((
                        source,
                        FabricNode {
                            op: Op::And,
                            a: INPUTS + slot,
                            b: INPUTS + slot,
                            c: 0,
                        },
                    ));
                    pass_nodes += 1;
                }
            }
            entries.sort_by_key(|(source, _)| *source);

            for (index, node) in self.nodes.iter().enumerate() {
                let source = INPUTS + index;
                if active[index] && depth[source] == logical_depth {
                    let selector = |dependency: usize| -> Result<usize, MapError> {
                        if dependency < INPUTS {
                            Ok(dependency)
                        } else {
                            previous.get(&dependency).map(|slot| INPUTS + slot).ok_or(
                                MapError::MissingSource {
                                    layer,
                                    source: dependency,
                                },
                            )
                        }
                    };
                    entries.push((
                        source,
                        FabricNode {
                            op: node.op,
                            a: selector(node.a)?,
                            b: selector(node.b)?,
                            c: if node.op == Op::Mux {
                                selector(node.c)?
                            } else {
                                0
                            },
                        },
                    ));
                }
            }

            if entries.len() > FABRIC_WIDTH {
                return Err(MapError::LayerOverflow {
                    layer,
                    required: entries.len(),
                    available: FABRIC_WIDTH,
                });
            }
            let mut physical = [dummy; FABRIC_WIDTH];
            let mut next = HashMap::new();
            for (slot, (source, node)) in entries.into_iter().enumerate() {
                physical[slot] = node;
                next.insert(source, slot);
            }
            layers.push(physical.to_vec());
            previous = next;
        }

        let mut outputs = [0usize; OUTPUTS];
        for (index, &source) in self.outputs.iter().enumerate() {
            outputs[index] = if source < INPUTS {
                source
            } else {
                INPUTS
                    + *previous.get(&source).ok_or(MapError::MissingOutput {
                        output: index,
                        source,
                    })?
            };
        }
        Ok(FabricConfig {
            layers,
            outputs,
            useful_nodes: active.iter().filter(|&&used| used).count(),
            pass_nodes,
        })
    }

    pub fn compact_fabric_rank(&self) -> FabricRank {
        let (active_nodes, depth, _) = self.structural_metrics();
        match self.map_to_compact_fabric() {
            Ok(mapped) => FabricRank {
                unmappable: 0,
                depth,
                layer_overflow: 0,
                occupied_nodes: mapped.useful_nodes + mapped.pass_nodes,
                active_nodes,
            },
            Err(MapError::LayerOverflow {
                required,
                available,
                ..
            }) => FabricRank {
                unmappable: 1,
                depth,
                layer_overflow: required - available,
                occupied_nodes: usize::MAX,
                active_nodes,
            },
            Err(_) => FabricRank {
                unmappable: 1,
                depth,
                layer_overflow: 0,
                occupied_nodes: usize::MAX,
                active_nodes,
            },
        }
    }

    pub fn map_to_compact_fabric_scheduled(
        &self,
        attempts: usize,
        seed: u64,
    ) -> Result<FabricConfig, MapError> {
        self.map_to_fabric_scheduled(FABRIC_LAYERS, FABRIC_WIDTH, attempts, seed)
    }

    pub fn map_to_post_fabric(&self, attempts: usize, seed: u64) -> Result<FabricConfig, MapError> {
        self.map_to_fabric_scheduled(POST_FABRIC_LAYERS, POST_FABRIC_WIDTH, attempts, seed)
    }

    pub fn map_to_fabric_scheduled(
        &self,
        layer_count: usize,
        width: usize,
        attempts: usize,
        seed: u64,
    ) -> Result<FabricConfig, MapError> {
        let mut active = vec![false; self.nodes.len()];
        let mut stack = self.outputs.to_vec();
        while let Some(source) = stack.pop() {
            if source < INPUTS {
                continue;
            }
            let index = source - INPUTS;
            if active[index] {
                continue;
            }
            active[index] = true;
            let node = self.nodes[index];
            stack.extend([node.a, node.b]);
            if node.op == Op::Mux {
                stack.push(node.c);
            }
        }

        let mut distance = vec![0usize; self.nodes.len()];
        for &output in &self.outputs {
            if output >= INPUTS {
                distance[output - INPUTS] = 1;
            }
        }
        for index in (0..self.nodes.len()).rev() {
            if !active[index] {
                continue;
            }
            let next_distance = distance[index] + 1;
            for source in dependencies(self.nodes[index]) {
                if source >= INPUTS {
                    distance[source - INPUTS] = distance[source - INPUTS].max(next_distance);
                }
            }
        }

        let dummy = FabricNode {
            op: Op::And,
            a: 0,
            b: 0,
            c: 0,
        };
        let mut rng = Rng::new(seed);
        let mut best_error = MapError::LayerOverflow {
            layer: 0,
            required: width + 1,
            available: width,
        };

        for _ in 0..attempts.max(1) {
            let mut scheduled = vec![false; self.nodes.len()];
            let mut previous = HashMap::<usize, usize>::new();
            let mut layers = Vec::with_capacity(layer_count);
            let mut pass_nodes = 0;
            let mut failed = false;

            for layer in 0..layer_count {
                let mut ready: Vec<usize> = self
                    .nodes
                    .iter()
                    .enumerate()
                    .filter(|(index, node)| {
                        active[*index]
                            && !scheduled[*index]
                            && dependencies(**node)
                                .iter()
                                .all(|&source| source < INPUTS || previous.contains_key(&source))
                    })
                    .map(|(index, _)| index)
                    .collect();
                ready.sort_by_key(|&index| {
                    let latest = layer_count.saturating_sub(distance[index]);
                    (latest, usize::MAX - distance[index], rng.next_u64())
                });

                let mut selected = Vec::<usize>::new();
                for index in ready {
                    let mut tentative = selected.clone();
                    tentative.push(index);
                    let carry = required_carries(self, &active, &scheduled, &tentative, &previous);
                    if carry.len() + tentative.len() <= width {
                        selected = tentative;
                    }
                }

                let overdue = active.iter().enumerate().any(|(index, &used)| {
                    used && !scheduled[index]
                        && !selected.contains(&index)
                        && layer_count.saturating_sub(distance[index]) <= layer
                });
                if overdue {
                    best_error = MapError::LayerOverflow {
                        layer,
                        required: width + 1,
                        available: width,
                    };
                    failed = true;
                    break;
                }

                let carry = required_carries(self, &active, &scheduled, &selected, &previous);
                let mut entries = Vec::<(usize, FabricNode)>::new();
                for source in carry {
                    let slot = previous[&source];
                    entries.push((
                        source,
                        FabricNode {
                            op: Op::And,
                            a: INPUTS + slot,
                            b: INPUTS + slot,
                            c: 0,
                        },
                    ));
                    pass_nodes += 1;
                }
                for index in selected {
                    let node = self.nodes[index];
                    let selector = |source: usize| -> Result<usize, MapError> {
                        if source < INPUTS {
                            Ok(source)
                        } else {
                            previous
                                .get(&source)
                                .map(|slot| INPUTS + slot)
                                .ok_or(MapError::MissingSource { layer, source })
                        }
                    };
                    entries.push((
                        INPUTS + index,
                        FabricNode {
                            op: node.op,
                            a: selector(node.a)?,
                            b: selector(node.b)?,
                            c: if node.op == Op::Mux {
                                selector(node.c)?
                            } else {
                                0
                            },
                        },
                    ));
                    scheduled[index] = true;
                }
                let mut physical = vec![dummy; width];
                let mut next = HashMap::new();
                for (slot, (source, node)) in entries.into_iter().enumerate() {
                    physical[slot] = node;
                    next.insert(source, slot);
                }
                layers.push(physical);
                previous = next;
            }
            if failed
                || active
                    .iter()
                    .enumerate()
                    .any(|(i, &used)| used && !scheduled[i])
            {
                continue;
            }
            let mut outputs = [0usize; OUTPUTS];
            for (index, &source) in self.outputs.iter().enumerate() {
                outputs[index] = if source < INPUTS {
                    source
                } else if let Some(slot) = previous.get(&source) {
                    INPUTS + slot
                } else {
                    failed = true;
                    break;
                };
            }
            if !failed {
                return Ok(FabricConfig {
                    layers,
                    outputs,
                    useful_nodes: active.iter().filter(|&&used| used).count(),
                    pass_nodes,
                });
            }
        }
        Err(best_error)
    }
}

fn dependencies(node: Node) -> Vec<usize> {
    let mut result = vec![node.a, node.b];
    if node.op == Op::Mux {
        result.push(node.c);
    }
    result
}

fn required_carries(
    circuit: &Circuit,
    active: &[bool],
    scheduled: &[bool],
    selected: &[usize],
    previous: &HashMap<usize, usize>,
) -> Vec<usize> {
    let mut carry: Vec<usize> = previous
        .keys()
        .copied()
        .filter(|&source| {
            circuit.outputs.contains(&source)
                || circuit.nodes.iter().enumerate().any(|(index, &node)| {
                    active[index]
                        && !scheduled[index]
                        && !selected.contains(&index)
                        && dependencies(node).contains(&source)
                })
        })
        .collect();
    carry.sort_unstable();
    carry
}

impl FabricConfig {
    pub fn evaluate_row(&self, inputs: [bool; INPUTS]) -> [bool; OUTPUTS] {
        let mut previous = vec![false; self.layers.first().map(Vec::len).unwrap_or(0)];
        for layer in &self.layers {
            let sources = |selector: usize| {
                if selector < INPUTS {
                    inputs[selector]
                } else {
                    previous[selector - INPUTS]
                }
            };
            let mut next = vec![false; layer.len()];
            for (slot, node) in layer.iter().enumerate() {
                let a = sources(node.a);
                let b = sources(node.b);
                let c = sources(node.c);
                next[slot] = match node.op {
                    Op::And => a & b,
                    Op::Or => a | b,
                    Op::Xor => a ^ b,
                    Op::Nand => !(a & b),
                    Op::Nor => !(a | b),
                    Op::Xnor => !(a ^ b),
                    Op::Mux => {
                        if a {
                            b
                        } else {
                            c
                        }
                    }
                };
            }
            previous = next;
        }
        std::array::from_fn(|index| {
            let selector = self.outputs[index];
            if selector < INPUTS {
                inputs[selector]
            } else {
                previous[selector - INPUTS]
            }
        })
    }
}

pub fn refine_exact(
    seed_circuit: &Circuit,
    target: &[Signature; OUTPUTS],
    attempts: usize,
    seed: u64,
) -> (Circuit, Score, usize) {
    let mut rng = Rng::new(seed);
    let mut parent = seed_circuit.clone();
    let mut parent_score = parent.score(target);
    assert!(parent_score.exact(), "refinement seed must be exact");
    let mut accepted = 0;

    for _ in 0..attempts {
        let mut child = parent.clone();
        if rng.below(3) == 0 {
            child.bypass_node(rng.below(child.nodes.len()), &mut rng);
        } else {
            child.mutate(&mut rng, 1);
        }
        let score = child.score(target);
        if score.exact()
            && (score.better_than(parent_score)
                || (score == parent_score && child.canonical_key() != parent.canonical_key()))
        {
            parent = child;
            parent_score = score;
            accepted += 1;
        }
    }
    (parent, parent_score, accepted)
}

pub fn refine_for_compact_fabric(
    seed_circuit: &Circuit,
    target: &[Signature; OUTPUTS],
    attempts: usize,
    seed: u64,
) -> (Circuit, Score, FabricRank, usize) {
    let mut rng = Rng::new(seed);
    let mut parent = seed_circuit.clone();
    let mut parent_score = parent.score(target);
    let mut parent_rank = parent.compact_fabric_rank();
    assert!(parent_score.exact(), "fabric refinement seed must be exact");
    let mut accepted = 0;

    for _ in 0..attempts {
        let mut child = parent.clone();
        if rng.below(3) == 0 {
            child.bypass_node(rng.below(child.nodes.len()), &mut rng);
        } else {
            child.mutate(&mut rng, 1);
        }
        let score = child.score(target);
        if !score.exact() {
            continue;
        }
        let rank = child.compact_fabric_rank();
        if rank < parent_rank
            || (rank == parent_rank && child.canonical_key() != parent.canonical_key())
        {
            parent = child;
            parent_score = score;
            parent_rank = rank;
            accepted += 1;
        }
    }
    (parent, parent_score, parent_rank, accepted)
}

pub fn refine_for_fabric_shape(
    seed_circuit: &Circuit,
    target: &[Signature; OUTPUTS],
    attempts: usize,
    seed: u64,
    layers: usize,
    width: usize,
) -> (Circuit, Score, FabricConfig, usize, usize) {
    let mut rng = Rng::new(seed);
    let mut parent = seed_circuit.clone();
    let mut parent_score = parent.score(target);
    assert!(parent_score.exact(), "fabric refinement seed must be exact");
    let mut parent_map = parent
        .map_to_fabric_scheduled(layers, width, 10_000, seed ^ 0x51ced)
        .expect("fabric refinement seed must fit the required shape");
    let mut accepted = 0;
    let mut rejected_unmappable = 0;

    // A direct wiring or constant circuit is already minimal and has no
    // mutation target. This is a normal synthesis result, not an error.
    if parent.nodes.is_empty() {
        return (
            parent,
            parent_score,
            parent_map,
            accepted,
            rejected_unmappable,
        );
    }

    for _ in 0..attempts {
        let mut child = parent.clone();
        if rng.below(3) == 0 {
            child.bypass_node(rng.below(child.nodes.len()), &mut rng);
        } else {
            child.mutate(&mut rng, 1);
        }
        let score = child.score(target);
        if !score.exact() {
            continue;
        }
        let Ok(mapped) = child.map_to_fabric_scheduled(layers, width, 1, rng.next_u64()) else {
            rejected_unmappable += 1;
            continue;
        };
        let rank = (
            mapped.useful_nodes + mapped.pass_nodes,
            score.active_nodes,
            score.depth,
            score.max_fanout,
        );
        let parent_rank = (
            parent_map.useful_nodes + parent_map.pass_nodes,
            parent_score.active_nodes,
            parent_score.depth,
            parent_score.max_fanout,
        );
        if rank < parent_rank
            || (rank == parent_rank && child.canonical_key() != parent.canonical_key())
        {
            parent = child;
            parent_score = score;
            parent_map = mapped;
            accepted += 1;
        }
    }
    (
        parent,
        parent_score,
        parent_map,
        accepted,
        rejected_unmappable,
    )
}

pub fn evolve(
    target: &[Signature; OUTPUTS],
    node_count: usize,
    generations: usize,
    offspring: usize,
    seed: u64,
) -> (Circuit, Score, usize) {
    let mut rng = Rng::new(seed);
    let mut parent = Circuit::random(node_count, &mut rng);
    let mut parent_score = parent.score(target);
    let mut evaluated = 1;

    for _ in 0..generations {
        let mut best = parent.clone();
        let mut best_score = parent_score;
        for _ in 0..offspring {
            let mut child = parent.clone();
            let mutation_count = 1 + rng.below(3);
            child.mutate(&mut rng, mutation_count);
            let score = child.score(target);
            evaluated += 1;
            if score.better_than(best_score) || (score == best_score && rng.below(2) == 0) {
                best = child;
                best_score = score;
            }
        }
        if best_score.better_than(parent_score)
            || (best_score.correct_bits == parent_score.correct_bits && rng.below(4) == 0)
        {
            parent = best;
            parent_score = best_score;
        }
        if parent_score.exact() {
            break;
        }
    }
    (parent, parent_score, evaluated)
}

fn and(a: Signature, b: Signature) -> Signature {
    std::array::from_fn(|i| a[i] & b[i])
}

fn or(a: Signature, b: Signature) -> Signature {
    std::array::from_fn(|i| a[i] | b[i])
}

fn xor(a: Signature, b: Signature) -> Signature {
    std::array::from_fn(|i| a[i] ^ b[i])
}

fn xnor(a: Signature, b: Signature) -> Signature {
    not(xor(a, b))
}

fn not(a: Signature) -> Signature {
    std::array::from_fn(|i| !a[i])
}

fn mux(select: Signature, when_true: Signature, when_false: Signature) -> Signature {
    std::array::from_fn(|i| (select[i] & when_true[i]) | (!select[i] & when_false[i]))
}

fn majority3(a: Signature, b: Signature, c: Signature) -> Signature {
    or(or(and(a, b), and(a, c)), and(b, c))
}
