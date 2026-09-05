//! Width-parametric multiplier core generator.
//!
//! Every core is an unsigned `W x W -> 2W` multiplier built from explicit
//! arithmetic cells (partial-product AND, half adder, full adder, prefix
//! generate/propagate cells). The cells are emitted as `keep_hierarchy`
//! Verilog modules so synthesis preserves the chosen topology instead of
//! flattening every candidate back into the same network.
//!
//! A candidate is a (reduction, final adder) pair. Reductions turn the
//! partial-product columns into at most two rows; final adders resolve the
//! carries. The same generator emits 4-, 8-, 16- and 32-bit instances so a
//! claimed construction can be checked for width scaling rather than as a
//! one-off 4x4 graph.

use std::fmt::Write as _;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wire {
    Const0,
    Const1,
    /// Operand bit: `A(i)` is `a[i]`, `B(i)` is `b[i]`.
    A(usize),
    B(usize),
    /// Optional third operand, used by fused multiply-accumulate workloads.
    Aux(usize),
    Net(usize),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Cell {
    And2 {
        a: Wire,
        b: Wire,
        y: usize,
    },
    Or2 {
        a: Wire,
        b: Wire,
        y: usize,
    },
    Mux2 {
        sel: Wire,
        a: Wire,
        b: Wire,
        y: usize,
    },
    Xor2 {
        a: Wire,
        b: Wire,
        y: usize,
    },
    Xor3 {
        a: Wire,
        b: Wire,
        c: Wire,
        y: usize,
    },
    Ha {
        a: Wire,
        b: Wire,
        s: usize,
        c: usize,
    },
    Fa {
        a: Wire,
        b: Wire,
        cin: Wire,
        s: usize,
        c: usize,
    },
    /// Full adder with a mux-based carry: s = (a ^ b) ^ cin, c = (a ^ b) ? cin : a.
    /// Same relation as `Fa`, different physical realization.
    FaMux {
        a: Wire,
        b: Wire,
        cin: Wire,
        s: usize,
        c: usize,
    },
    /// g = a & b, p = a ^ b
    Pg {
        a: Wire,
        b: Wire,
        g: usize,
        p: usize,
    },
    /// g = gh | (ph & gl), p = ph & pl
    Black {
        gh: Wire,
        ph: Wire,
        gl: Wire,
        pl: Wire,
        g: usize,
        p: usize,
    },
    /// g = gh | (ph & gl)
    Gray {
        gh: Wire,
        ph: Wire,
        gl: Wire,
        g: usize,
    },
}

impl Cell {
    pub fn kind(&self) -> &'static str {
        match self {
            Cell::And2 { .. } => "and2",
            Cell::Or2 { .. } => "or2",
            Cell::Mux2 { .. } => "mux2",
            Cell::Xor2 { .. } => "xor2",
            Cell::Xor3 { .. } => "xor3",
            Cell::Ha { .. } => "ha",
            Cell::Fa { .. } => "fa",
            Cell::FaMux { .. } => "famux",
            Cell::Pg { .. } => "pg",
            Cell::Black { .. } => "black",
            Cell::Gray { .. } => "gray",
        }
    }

    pub(crate) fn inputs(&self) -> Vec<Wire> {
        match *self {
            Cell::And2 { a, b, .. }
            | Cell::Or2 { a, b, .. }
            | Cell::Xor2 { a, b, .. }
            | Cell::Ha { a, b, .. } => vec![a, b],
            Cell::Mux2 { sel, a, b, .. } => vec![sel, a, b],
            Cell::Xor3 { a, b, c, .. } => vec![a, b, c],
            Cell::Fa { a, b, cin, .. } | Cell::FaMux { a, b, cin, .. } => vec![a, b, cin],
            Cell::Pg { a, b, .. } => vec![a, b],
            Cell::Black { gh, ph, gl, pl, .. } => vec![gh, ph, gl, pl],
            Cell::Gray { gh, ph, gl, .. } => vec![gh, ph, gl],
        }
    }

    pub(crate) fn outputs(&self) -> Vec<usize> {
        match *self {
            Cell::And2 { y, .. }
            | Cell::Or2 { y, .. }
            | Cell::Mux2 { y, .. }
            | Cell::Xor2 { y, .. }
            | Cell::Xor3 { y, .. } => vec![y],
            Cell::Ha { s, c, .. } | Cell::Fa { s, c, .. } | Cell::FaMux { s, c, .. } => vec![s, c],
            Cell::Pg { g, p, .. } | Cell::Black { g, p, .. } => vec![g, p],
            Cell::Gray { g, .. } => vec![g],
        }
    }

    /// Simple-gate cost in the AND/OR/XOR vocabulary used elsewhere in the
    /// repository: a two-input gate costs one, XOR3 two, HA two, FA five,
    /// PG two, black three, gray two.
    pub fn simple_gate_cost(&self) -> usize {
        match self {
            Cell::And2 { .. } | Cell::Or2 { .. } | Cell::Xor2 { .. } => 1,
            Cell::Mux2 { .. } => 3,
            Cell::Xor3 { .. } | Cell::Ha { .. } | Cell::Pg { .. } | Cell::Gray { .. } => 2,
            Cell::Black { .. } => 3,
            Cell::Fa { .. } | Cell::FaMux { .. } => 5,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Reduction {
    Array,
    Wallace,
    Dadda,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Adder {
    Ripple,
    KoggeStone,
    Sklansky,
    BrentKung,
    HanCarlson,
}

pub const REDUCTIONS: [Reduction; 3] = [Reduction::Array, Reduction::Wallace, Reduction::Dadda];
pub const ADDERS: [Adder; 5] = [
    Adder::Ripple,
    Adder::KoggeStone,
    Adder::Sklansky,
    Adder::BrentKung,
    Adder::HanCarlson,
];

impl Reduction {
    pub fn name(self) -> &'static str {
        match self {
            Reduction::Array => "array",
            Reduction::Wallace => "wallace",
            Reduction::Dadda => "dadda",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        REDUCTIONS.iter().copied().find(|r| r.name() == name)
    }
}

impl Adder {
    pub fn name(self) -> &'static str {
        match self {
            Adder::Ripple => "ripple",
            Adder::KoggeStone => "koggestone",
            Adder::Sklansky => "sklansky",
            Adder::BrentKung => "brentkung",
            Adder::HanCarlson => "hancarlson",
        }
    }

    pub fn parse(name: &str) -> Option<Self> {
        ADDERS.iter().copied().find(|a| a.name() == name)
    }
}

#[derive(Clone, Debug)]
pub struct Netlist {
    pub width: usize,
    pub reduction: Reduction,
    pub adder: Adder,
    pub cells: Vec<Cell>,
    pub outputs: Vec<Wire>,
    /// Non-empty for evolved cores, which are not named by (reduction, adder).
    pub label: String,
    pub aux_width: usize,
    nets: usize,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Counts {
    pub and2: usize,
    pub or2: usize,
    pub mux2: usize,
    pub xor2: usize,
    pub xor3: usize,
    pub ha: usize,
    pub fa: usize,
    pub famux: usize,
    pub pg: usize,
    pub black: usize,
    pub gray: usize,
    pub cells: usize,
    pub simple_gates: usize,
    /// Longest path in cells (unit delay per cell).
    pub depth: usize,
    /// Longest path in the simple-gate depth model: XOR-based cells count as
    /// two levels, black cells two, AND/gray one.
    pub gate_depth: usize,
}

/// Technology-independent pressure indicators for routing. These are cheap
/// graph measurements, not substitutes for placement or extracted parasitics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RoutingProxy {
    pub sink_pins: usize,
    pub fanout_excess_sq: usize,
    pub max_fanout: usize,
    pub topological_span: usize,
    pub score: usize,
}

impl Netlist {
    pub(crate) fn new(width: usize, reduction: Reduction, adder: Adder) -> Self {
        Netlist {
            width,
            reduction,
            adder,
            cells: Vec::new(),
            outputs: Vec::new(),
            label: String::new(),
            aux_width: 0,
            nets: 0,
        }
    }

    pub fn routing_proxy(&self) -> RoutingProxy {
        let mut producers = vec![0usize; self.nets];
        for (index, cell) in self.cells.iter().enumerate() {
            for output in cell.outputs() {
                producers[output] = index + 1;
            }
        }
        let mut net_fanout = vec![0usize; self.nets];
        let mut input_fanout = vec![0usize; self.width * 2 + self.aux_width];
        let mut sink_pins = 0usize;
        let mut topological_span = 0usize;
        for (index, cell) in self.cells.iter().enumerate() {
            for input in cell.inputs() {
                sink_pins += 1;
                match input {
                    Wire::Net(net) => {
                        net_fanout[net] += 1;
                        topological_span += (index + 1).saturating_sub(producers[net]);
                    }
                    Wire::A(bit) => input_fanout[bit] += 1,
                    Wire::B(bit) => input_fanout[self.width + bit] += 1,
                    Wire::Aux(bit) => input_fanout[self.width * 2 + bit] += 1,
                    Wire::Const0 | Wire::Const1 => {}
                }
            }
        }
        for output in &self.outputs {
            if let Wire::Net(net) = *output {
                sink_pins += 1;
                net_fanout[net] += 1;
                topological_span += self.cells.len() + 1 - producers[net];
            }
        }
        let mut all_fanout = net_fanout;
        all_fanout.extend(input_fanout);
        let max_fanout = all_fanout.iter().copied().max().unwrap_or(0);
        let fanout_excess_sq = all_fanout
            .iter()
            .map(|fanout| fanout.saturating_sub(1).pow(2))
            .sum();
        // Sink pins approximate wire demand; quadratic excess penalizes hubs;
        // normalized construction span distinguishes long-lived dependencies.
        let normalized_span = topological_span / self.cells.len().max(1);
        RoutingProxy {
            sink_pins,
            fanout_excess_sq,
            max_fanout,
            topological_span,
            score: sink_pins + 4 * fanout_excess_sq + normalized_span,
        }
    }

    pub fn net_count(&self) -> usize {
        self.nets
    }

    pub fn module_name(&self) -> String {
        if !self.label.is_empty() {
            return format!("mul{}_{}", self.width, self.label);
        }
        format!(
            "mul{}_{}_{}",
            self.width,
            self.reduction.name(),
            self.adder.name()
        )
    }

    pub(crate) fn net(&mut self) -> usize {
        self.nets += 1;
        self.nets - 1
    }

    pub(crate) fn and2(&mut self, a: Wire, b: Wire) -> Wire {
        if a == Wire::Const0 || b == Wire::Const0 {
            return Wire::Const0;
        }
        if a == Wire::Const1 {
            return b;
        }
        if b == Wire::Const1 {
            return a;
        }
        let y = self.net();
        self.cells.push(Cell::And2 { a, b, y });
        Wire::Net(y)
    }

    pub(crate) fn xor2(&mut self, a: Wire, b: Wire) -> Wire {
        if a == Wire::Const0 {
            return b;
        }
        if b == Wire::Const0 {
            return a;
        }
        let y = self.net();
        self.cells.push(Cell::Xor2 { a, b, y });
        Wire::Net(y)
    }

    pub(crate) fn or2(&mut self, a: Wire, b: Wire) -> Wire {
        if a == Wire::Const1 || b == Wire::Const1 {
            return Wire::Const1;
        }
        if a == Wire::Const0 {
            return b;
        }
        if b == Wire::Const0 || a == b {
            return a;
        }
        let y = self.net();
        self.cells.push(Cell::Or2 { a, b, y });
        Wire::Net(y)
    }

    pub(crate) fn mux2(&mut self, sel: Wire, a: Wire, b: Wire) -> Wire {
        if a == b {
            return a;
        }
        if sel == Wire::Const0 {
            return a;
        }
        if sel == Wire::Const1 {
            return b;
        }
        let y = self.net();
        self.cells.push(Cell::Mux2 { sel, a, b, y });
        Wire::Net(y)
    }

    pub(crate) fn xor3(&mut self, a: Wire, b: Wire, c: Wire) -> Wire {
        let y = self.net();
        self.cells.push(Cell::Xor3 { a, b, c, y });
        Wire::Net(y)
    }

    pub(crate) fn ha(&mut self, a: Wire, b: Wire) -> (Wire, Wire) {
        let s = self.net();
        let c = self.net();
        self.cells.push(Cell::Ha { a, b, s, c });
        (Wire::Net(s), Wire::Net(c))
    }

    pub(crate) fn fa(&mut self, a: Wire, b: Wire, cin: Wire) -> (Wire, Wire) {
        let s = self.net();
        let c = self.net();
        self.cells.push(Cell::Fa { a, b, cin, s, c });
        (Wire::Net(s), Wire::Net(c))
    }

    pub(crate) fn fa_mux(&mut self, a: Wire, b: Wire, cin: Wire) -> (Wire, Wire) {
        let s = self.net();
        let c = self.net();
        self.cells.push(Cell::FaMux { a, b, cin, s, c });
        (Wire::Net(s), Wire::Net(c))
    }

    fn pg(&mut self, a: Wire, b: Wire) -> (Wire, Wire) {
        let g = self.net();
        let p = self.net();
        self.cells.push(Cell::Pg { a, b, g, p });
        (Wire::Net(g), Wire::Net(p))
    }

    /// Prefix combine with constant folding on generate inputs. `need_p` is
    /// false when the node spans down to bit zero, where its group propagate
    /// is never consumed and a gray cell suffices.
    fn combine(&mut self, high: (Wire, Wire), low: (Wire, Wire), need_p: bool) -> (Wire, Wire) {
        let (gh, ph) = high;
        let (gl, pl) = low;
        if gl == Wire::Const0 {
            let p = if need_p {
                self.and2(ph, pl)
            } else {
                Wire::Const0
            };
            return (gh, p);
        }
        if gh == Wire::Const0 {
            let g = self.and2(ph, gl);
            let p = if need_p {
                self.and2(ph, pl)
            } else {
                Wire::Const0
            };
            return (g, p);
        }
        if need_p {
            let g = self.net();
            let p = self.net();
            self.cells.push(Cell::Black {
                gh,
                ph,
                gl,
                pl,
                g,
                p,
            });
            (Wire::Net(g), Wire::Net(p))
        } else {
            let g = self.net();
            self.cells.push(Cell::Gray { gh, ph, gl, g });
            (Wire::Net(g), Wire::Const0)
        }
    }

    /// Remove cells none of whose outputs reach a primary output.
    pub(crate) fn prune(&mut self) {
        loop {
            let mut used = vec![false; self.nets];
            for wire in &self.outputs {
                if let Wire::Net(n) = wire {
                    used[*n] = true;
                }
            }
            for cell in &self.cells {
                for wire in cell.inputs() {
                    if let Wire::Net(n) = wire {
                        used[n] = true;
                    }
                }
            }
            let before = self.cells.len();
            self.cells
                .retain(|cell| cell.outputs().iter().any(|n| used[*n]));
            if self.cells.len() == before {
                break;
            }
        }
    }

    pub fn counts(&self) -> Counts {
        let mut counts = Counts::default();
        let mut level = vec![0usize; self.nets];
        let mut gate_level = vec![0usize; self.nets];
        for cell in &self.cells {
            match cell {
                Cell::And2 { .. } => counts.and2 += 1,
                Cell::Or2 { .. } => counts.or2 += 1,
                Cell::Mux2 { .. } => counts.mux2 += 1,
                Cell::Xor2 { .. } => counts.xor2 += 1,
                Cell::Xor3 { .. } => counts.xor3 += 1,
                Cell::Ha { .. } => counts.ha += 1,
                Cell::Fa { .. } => counts.fa += 1,
                Cell::FaMux { .. } => counts.famux += 1,
                Cell::Pg { .. } => counts.pg += 1,
                Cell::Black { .. } => counts.black += 1,
                Cell::Gray { .. } => counts.gray += 1,
            }
            counts.cells += 1;
            counts.simple_gates += cell.simple_gate_cost();
            let input_level = cell
                .inputs()
                .iter()
                .map(|w| match w {
                    Wire::Net(n) => level[*n],
                    _ => 0,
                })
                .max()
                .unwrap_or(0);
            let input_gate_level = cell
                .inputs()
                .iter()
                .map(|w| match w {
                    Wire::Net(n) => gate_level[*n],
                    _ => 0,
                })
                .max()
                .unwrap_or(0);
            let gate_levels = match cell {
                Cell::And2 { .. } | Cell::Or2 { .. } | Cell::Gray { .. } => 1,
                Cell::Mux2 { .. } => 2,
                Cell::Xor2 { .. } | Cell::Ha { .. } | Cell::Pg { .. } | Cell::Black { .. } => 2,
                Cell::Xor3 { .. } => 2,
                Cell::Fa { .. } | Cell::FaMux { .. } => 3,
            };
            for out in cell.outputs() {
                level[out] = input_level + 1;
                gate_level[out] = input_gate_level + gate_levels;
                counts.depth = counts.depth.max(level[out]);
                counts.gate_depth = counts.gate_depth.max(gate_level[out]);
            }
        }
        counts
    }

    /// Evaluate the netlist for one operand pair. Cells are stored in
    /// construction order, which is topological.
    pub fn eval(&self, a: u64, b: u64) -> u64 {
        self.eval_with_aux(a, b, 0)
    }

    pub fn eval_with_aux(&self, a: u64, b: u64, aux: u64) -> u64 {
        assert!(
            self.outputs.len() <= 64,
            "use eval_with_aux_u128 for wide output bundles"
        );
        self.eval_with_aux_u128(a, b, aux) as u64
    }

    /// Evaluate a multi-root bundle with up to 128 output bits.
    pub fn eval_with_aux_u128(&self, a: u64, b: u64, aux: u64) -> u128 {
        assert!(self.outputs.len() <= 128);
        let mut value = vec![false; self.nets];
        let read = |w: Wire, value: &Vec<bool>| -> bool {
            match w {
                Wire::Const0 => false,
                Wire::Const1 => true,
                Wire::A(i) => (a >> i) & 1 == 1,
                Wire::B(i) => (b >> i) & 1 == 1,
                Wire::Aux(i) => (aux >> i) & 1 == 1,
                Wire::Net(n) => value[n],
            }
        };
        for cell in &self.cells {
            match *cell {
                Cell::And2 { a, b, y } => value[y] = read(a, &value) & read(b, &value),
                Cell::Or2 { a, b, y } => value[y] = read(a, &value) | read(b, &value),
                Cell::Mux2 { sel, a, b, y } => {
                    value[y] = if read(sel, &value) {
                        read(b, &value)
                    } else {
                        read(a, &value)
                    }
                }
                Cell::Xor2 { a, b, y } => value[y] = read(a, &value) ^ read(b, &value),
                Cell::Xor3 { a, b, c, y } => {
                    value[y] = read(a, &value) ^ read(b, &value) ^ read(c, &value)
                }
                Cell::Ha { a, b, s, c } => {
                    let (x, y) = (read(a, &value), read(b, &value));
                    value[s] = x ^ y;
                    value[c] = x & y;
                }
                Cell::Fa { a, b, cin, s, c } | Cell::FaMux { a, b, cin, s, c } => {
                    let (x, y, z) = (read(a, &value), read(b, &value), read(cin, &value));
                    value[s] = x ^ y ^ z;
                    value[c] = (x & y) | (x & z) | (y & z);
                }
                Cell::Pg { a, b, g, p } => {
                    let (x, y) = (read(a, &value), read(b, &value));
                    value[g] = x & y;
                    value[p] = x ^ y;
                }
                Cell::Black {
                    gh,
                    ph,
                    gl,
                    pl,
                    g,
                    p,
                } => {
                    let (xgh, xph, xgl, xpl) = (
                        read(gh, &value),
                        read(ph, &value),
                        read(gl, &value),
                        read(pl, &value),
                    );
                    value[g] = xgh | (xph & xgl);
                    value[p] = xph & xpl;
                }
                Cell::Gray { gh, ph, gl, g } => {
                    value[g] = read(gh, &value) | (read(ph, &value) & read(gl, &value));
                }
            }
        }
        let mut out = 0u128;
        for (i, wire) in self.outputs.iter().enumerate() {
            if read(*wire, &value) {
                out |= 1 << i;
            }
        }
        out
    }

    /// Evaluate 64 operand pairs in parallel. Bit `lane` of each input word
    /// and output word belongs to the same independent evaluation.
    pub fn eval_batch64(&self, a: &[u64], b: &[u64]) -> Vec<u64> {
        self.eval_batch64_with_aux(a, b, &vec![0; self.aux_width])
    }

    pub fn eval_batch64_with_aux(&self, a: &[u64], b: &[u64], aux: &[u64]) -> Vec<u64> {
        assert_eq!(a.len(), self.width);
        assert_eq!(b.len(), self.width);
        assert_eq!(aux.len(), self.aux_width);
        let mut value = vec![0u64; self.nets];
        let read = |w: Wire, value: &[u64]| -> u64 {
            match w {
                Wire::Const0 => 0,
                Wire::Const1 => u64::MAX,
                Wire::A(i) => a[i],
                Wire::B(i) => b[i],
                Wire::Aux(i) => aux[i],
                Wire::Net(n) => value[n],
            }
        };
        for cell in &self.cells {
            match *cell {
                Cell::And2 { a, b, y } => value[y] = read(a, &value) & read(b, &value),
                Cell::Or2 { a, b, y } => value[y] = read(a, &value) | read(b, &value),
                Cell::Mux2 { sel, a, b, y } => {
                    let s = read(sel, &value);
                    value[y] = (!s & read(a, &value)) | (s & read(b, &value));
                }
                Cell::Xor2 { a, b, y } => value[y] = read(a, &value) ^ read(b, &value),
                Cell::Xor3 { a, b, c, y } => {
                    value[y] = read(a, &value) ^ read(b, &value) ^ read(c, &value)
                }
                Cell::Ha { a, b, s, c } => {
                    let (x, y) = (read(a, &value), read(b, &value));
                    value[s] = x ^ y;
                    value[c] = x & y;
                }
                Cell::Fa { a, b, cin, s, c } | Cell::FaMux { a, b, cin, s, c } => {
                    let (x, y, z) = (read(a, &value), read(b, &value), read(cin, &value));
                    value[s] = x ^ y ^ z;
                    value[c] = (x & y) | (x & z) | (y & z);
                }
                Cell::Pg { a, b, g, p } => {
                    let (x, y) = (read(a, &value), read(b, &value));
                    value[g] = x & y;
                    value[p] = x ^ y;
                }
                Cell::Black {
                    gh,
                    ph,
                    gl,
                    pl,
                    g,
                    p,
                } => {
                    let (xg, xp, yg, yp) = (
                        read(gh, &value),
                        read(ph, &value),
                        read(gl, &value),
                        read(pl, &value),
                    );
                    value[g] = xg | (xp & yg);
                    value[p] = xp & yp;
                }
                Cell::Gray { gh, ph, gl, g } => {
                    value[g] = read(gh, &value) | (read(ph, &value) & read(gl, &value));
                }
            }
        }
        self.outputs.iter().map(|&w| read(w, &value)).collect()
    }

    pub fn to_verilog(&self) -> String {
        let width = self.width;
        let output_width = self.outputs.len();
        let mut v = String::new();
        let _ = writeln!(
            v,
            "// Generated arithmetic netlist. Do not edit.\n// {}: unsigned {w}x{w} -> {out} bits, {} reduction, {} final adder.",
            self.module_name(),
            self.reduction.name(),
            self.adder.name(),
            w = width,
            out = output_width
        );
        if self.aux_width == 0 {
            let _ = writeln!(
                v,
                "module {}(input wire [{}:0] a, input wire [{}:0] b, output wire [{}:0] y);",
                self.module_name(),
                width - 1,
                width - 1,
                output_width - 1
            );
        } else {
            let _ = writeln!(
                v,
                "module {}(input wire [{}:0] a, input wire [{}:0] b, input wire [{}:0] aux, output wire [{}:0] y);",
                self.module_name(), width - 1, width - 1, self.aux_width - 1, output_width - 1
            );
        }
        if self.nets > 0 {
            let _ = writeln!(v, "    wire [{}:0] n;", self.nets - 1);
        }
        let name = |w: Wire| -> String {
            match w {
                Wire::Const0 => "1'b0".to_string(),
                Wire::Const1 => "1'b1".to_string(),
                Wire::A(i) => format!("a[{i}]"),
                Wire::B(i) => format!("b[{i}]"),
                Wire::Aux(i) => format!("aux[{i}]"),
                Wire::Net(n) => format!("n[{n}]"),
            }
        };
        for (index, cell) in self.cells.iter().enumerate() {
            let line = match *cell {
                Cell::And2 { a, b, y } => {
                    format!("gen_and2 u{index}({}, {}, n[{y}]);", name(a), name(b))
                }
                Cell::Or2 { a, b, y } => {
                    format!("gen_or2 u{index}({}, {}, n[{y}]);", name(a), name(b))
                }
                Cell::Mux2 { sel, a, b, y } => format!(
                    "gen_mux2 u{index}({}, {}, {}, n[{y}]);",
                    name(sel),
                    name(a),
                    name(b)
                ),
                Cell::Xor2 { a, b, y } => {
                    format!("gen_xor2 u{index}({}, {}, n[{y}]);", name(a), name(b))
                }
                Cell::Xor3 { a, b, c, y } => format!(
                    "gen_xor3 u{index}({}, {}, {}, n[{y}]);",
                    name(a),
                    name(b),
                    name(c)
                ),
                Cell::Ha { a, b, s, c } => {
                    format!("gen_ha u{index}({}, {}, n[{s}], n[{c}]);", name(a), name(b))
                }
                Cell::Fa { a, b, cin, s, c } => format!(
                    "gen_fa u{index}({}, {}, {}, n[{s}], n[{c}]);",
                    name(a),
                    name(b),
                    name(cin)
                ),
                Cell::FaMux { a, b, cin, s, c } => format!(
                    "gen_famux u{index}({}, {}, {}, n[{s}], n[{c}]);",
                    name(a),
                    name(b),
                    name(cin)
                ),
                Cell::Pg { a, b, g, p } => {
                    format!("gen_pg u{index}({}, {}, n[{g}], n[{p}]);", name(a), name(b))
                }
                Cell::Black {
                    gh,
                    ph,
                    gl,
                    pl,
                    g,
                    p,
                } => format!(
                    "gen_black u{index}({}, {}, {}, {}, n[{g}], n[{p}]);",
                    name(gh),
                    name(ph),
                    name(gl),
                    name(pl)
                ),
                Cell::Gray { gh, ph, gl, g } => format!(
                    "gen_gray u{index}({}, {}, {}, n[{g}]);",
                    name(gh),
                    name(ph),
                    name(gl)
                ),
            };
            let _ = writeln!(v, "    {line}");
        }
        for (i, wire) in self.outputs.iter().enumerate() {
            let _ = writeln!(v, "    assign y[{i}] = {};", name(*wire));
        }
        let _ = writeln!(v, "endmodule");
        v
    }
}

/// The preserved cell library shared by every generated core.
pub const CELLS_VERILOG: &str = r#"// Generated by `cargo run --release -- gen-cores`. Do not edit.
// Arithmetic cells are kept as hierarchy so synthesis preserves the
// generated topology. Remove the attribute to let ABC flatten a candidate.
(* keep_hierarchy *)
module gen_and2(input wire a, input wire b, output wire y);
    assign y = a & b;
endmodule

(* keep_hierarchy *)
module gen_or2(input wire a, input wire b, output wire y);
    assign y = a | b;
endmodule

(* keep_hierarchy *)
module gen_mux2(input wire sel, input wire a, input wire b, output wire y);
    assign y = sel ? b : a;
endmodule

(* keep_hierarchy *)
module gen_xor2(input wire a, input wire b, output wire y);
    assign y = a ^ b;
endmodule

(* keep_hierarchy *)
module gen_xor3(input wire a, input wire b, input wire c, output wire y);
    assign y = a ^ b ^ c;
endmodule

(* keep_hierarchy *)
module gen_ha(input wire a, input wire b, output wire s, output wire c);
    assign s = a ^ b;
    assign c = a & b;
endmodule

(* keep_hierarchy *)
module gen_fa(input wire a, input wire b, input wire cin, output wire s, output wire c);
    assign s = a ^ b ^ cin;
    assign c = (a & b) | (a & cin) | (b & cin);
endmodule

(* keep_hierarchy *)
module gen_famux(input wire a, input wire b, input wire cin, output wire s, output wire c);
    wire p = a ^ b;
    assign s = p ^ cin;
    assign c = p ? cin : a;
endmodule

(* keep_hierarchy *)
module gen_pg(input wire a, input wire b, output wire g, output wire p);
    assign g = a & b;
    assign p = a ^ b;
endmodule

(* keep_hierarchy *)
module gen_black(input wire gh, input wire ph, input wire gl, input wire pl, output wire g, output wire p);
    assign g = gh | (ph & gl);
    assign p = ph & pl;
endmodule

(* keep_hierarchy *)
module gen_gray(input wire gh, input wire ph, input wire gl, output wire g);
    assign g = gh | (ph & gl);
endmodule
"#;

/// Build one multiplier core.
pub fn generate(width: usize, reduction: Reduction, adder: Adder) -> Netlist {
    assert!((2..=32).contains(&width), "width must be between 2 and 32");
    let mut net = Netlist::new(width, reduction, adder);
    let columns = 2 * width;

    // Partial products, one column per weight.
    let mut pp = vec![vec![Wire::Const0; width]; width];
    for (i, row) in pp.iter_mut().enumerate() {
        for (j, bit) in row.iter_mut().enumerate() {
            *bit = net.and2(Wire::A(j), Wire::B(i));
        }
    }

    let (rows, carry_rows) = match reduction {
        Reduction::Array => reduce_array(&mut net, &pp, columns),
        Reduction::Wallace => {
            let mut cols = partial_product_columns(&pp, columns);
            reduce_wallace(&mut net, &mut cols);
            split_rows(cols)
        }
        Reduction::Dadda => {
            let mut cols = partial_product_columns(&pp, columns);
            reduce_dadda(&mut net, &mut cols);
            split_rows(cols)
        }
    };

    let outputs = match adder {
        Adder::Ripple => final_ripple(&mut net, &rows, &carry_rows),
        prefix => final_prefix(&mut net, &rows, &carry_rows, prefix),
    };
    net.outputs = outputs;
    net.prune();
    net
}

fn partial_product_columns(pp: &[Vec<Wire>], columns: usize) -> Vec<Vec<Wire>> {
    let width = pp.len();
    let mut cols = vec![Vec::new(); columns];
    for i in 0..width {
        for j in 0..width {
            cols[i + j].push(pp[i][j]);
        }
    }
    cols
}

/// Row-by-row carry-save array: each partial-product row is added to the
/// running sum with the carries of the previous row entering diagonally.
/// Returns (sum vector, carry vector) per column, `Const0` where absent.
fn reduce_array(net: &mut Netlist, pp: &[Vec<Wire>], columns: usize) -> (Vec<Wire>, Vec<Wire>) {
    let width = pp.len();
    let mut sum = vec![Wire::Const0; columns];
    let mut carry = vec![Wire::Const0; columns];
    sum[..width].copy_from_slice(&pp[0][..width]);
    for i in 1..width {
        let mut next_carry = vec![Wire::Const0; columns];
        for c in i..columns {
            let mut inputs = Vec::new();
            if sum[c] != Wire::Const0 {
                inputs.push(sum[c]);
            }
            if c - i < width {
                inputs.push(pp[i][c - i]);
            }
            if carry[c] != Wire::Const0 {
                inputs.push(carry[c]);
            }
            match inputs.len() {
                0 => {}
                1 => sum[c] = inputs[0],
                2 => {
                    let (s, co) = net.ha(inputs[0], inputs[1]);
                    sum[c] = s;
                    if c + 1 < columns {
                        next_carry[c + 1] = co;
                    }
                }
                _ => {
                    let (s, co) = net.fa(inputs[0], inputs[1], inputs[2]);
                    sum[c] = s;
                    if c + 1 < columns {
                        next_carry[c + 1] = co;
                    }
                }
            }
        }
        carry = next_carry;
    }
    (sum, carry)
}

/// Classic Wallace reduction: every stage uses as many full adders as each
/// column allows, a half adder on a remainder of two, and passes single bits.
fn reduce_wallace(net: &mut Netlist, cols: &mut Vec<Vec<Wire>>) {
    let columns = cols.len();
    while cols.iter().any(|c| c.len() > 2) {
        let mut next = vec![Vec::new(); columns];
        for c in 0..columns {
            let bits = std::mem::take(&mut cols[c]);
            if bits.len() <= 2 {
                next[c].extend(bits);
                continue;
            }
            let mut iter = bits.into_iter().peekable();
            while let Some(a) = iter.next() {
                let b = iter.next();
                let d = iter.next();
                match (b, d) {
                    (Some(b), Some(d)) => {
                        let (s, co) = net.fa(a, b, d);
                        next[c].push(s);
                        if c + 1 < columns {
                            next[c + 1].push(co);
                        }
                    }
                    (Some(b), None) => {
                        let (s, co) = net.ha(a, b);
                        next[c].push(s);
                        if c + 1 < columns {
                            next[c + 1].push(co);
                        }
                    }
                    (None, _) => next[c].push(a),
                }
            }
        }
        *cols = next;
    }
}

/// Dadda reduction: reduce each stage only as far as the next height in the
/// sequence 2, 3, 4, 6, 9, 13, ... using the minimum number of adders.
fn reduce_dadda(net: &mut Netlist, cols: &mut Vec<Vec<Wire>>) {
    let columns = cols.len();
    let max_height = cols.iter().map(Vec::len).max().unwrap_or(0);
    let mut heights = vec![2usize];
    while *heights.last().unwrap() < max_height {
        let last = *heights.last().unwrap();
        heights.push(last * 3 / 2);
    }
    for &target in heights.iter().rev().skip(1) {
        let mut next: Vec<Vec<Wire>> = vec![Vec::new(); columns];
        for c in 0..columns {
            let mut bits = std::mem::take(&mut cols[c]);
            bits.reverse(); // pop from the front in original order
            loop {
                let height = bits.len() + next[c].len();
                if height <= target {
                    break;
                }
                if height - target >= 2 && bits.len() >= 3 {
                    let a = bits.pop().unwrap();
                    let b = bits.pop().unwrap();
                    let d = bits.pop().unwrap();
                    let (s, co) = net.fa(a, b, d);
                    next[c].push(s);
                    if c + 1 < columns {
                        next[c + 1].push(co);
                    }
                } else if bits.len() >= 2 {
                    let a = bits.pop().unwrap();
                    let b = bits.pop().unwrap();
                    let (s, co) = net.ha(a, b);
                    next[c].push(s);
                    if c + 1 < columns {
                        next[c + 1].push(co);
                    }
                } else {
                    break;
                }
            }
            while let Some(bit) = bits.pop() {
                next[c].push(bit);
            }
        }
        *cols = next;
    }
    debug_assert!(cols.iter().all(|c| c.len() <= 2));
}

pub(crate) fn split_rows(cols: Vec<Vec<Wire>>) -> (Vec<Wire>, Vec<Wire>) {
    let mut rows = vec![Wire::Const0; cols.len()];
    let mut carries = vec![Wire::Const0; cols.len()];
    for (c, bits) in cols.into_iter().enumerate() {
        assert!(bits.len() <= 2);
        if let Some(a) = bits.first() {
            rows[c] = *a;
        }
        if let Some(b) = bits.get(1) {
            carries[c] = *b;
        }
    }
    (rows, carries)
}

pub(crate) fn final_ripple(net: &mut Netlist, rows: &[Wire], carries: &[Wire]) -> Vec<Wire> {
    let columns = rows.len();
    let mut outputs = vec![Wire::Const0; columns];
    let mut cin = Wire::Const0;
    for c in 0..columns {
        let inputs: Vec<Wire> = [rows[c], carries[c], cin]
            .into_iter()
            .filter(|w| *w != Wire::Const0)
            .collect();
        let last = c + 1 == columns;
        match inputs.len() {
            0 => {
                outputs[c] = Wire::Const0;
                cin = Wire::Const0;
            }
            1 => {
                outputs[c] = inputs[0];
                cin = Wire::Const0;
            }
            2 => {
                if last {
                    outputs[c] = net.xor2(inputs[0], inputs[1]);
                } else {
                    let (s, co) = net.ha(inputs[0], inputs[1]);
                    outputs[c] = s;
                    cin = co;
                }
            }
            _ => {
                if last {
                    outputs[c] = net.xor3(inputs[0], inputs[1], inputs[2]);
                } else {
                    let (s, co) = net.fa(inputs[0], inputs[1], inputs[2]);
                    outputs[c] = s;
                    cin = co;
                }
            }
        }
    }
    outputs
}

/// Parallel-prefix final adder over the columns from the first two-bit
/// column upward. Lower columns pass through unchanged.
pub(crate) fn final_prefix(
    net: &mut Netlist,
    rows: &[Wire],
    carries: &[Wire],
    adder: Adder,
) -> Vec<Wire> {
    let columns = rows.len();
    let mut outputs = vec![Wire::Const0; columns];
    let lo = match (0..columns).find(|&c| rows[c] != Wire::Const0 && carries[c] != Wire::Const0) {
        Some(lo) => lo,
        None => {
            outputs.copy_from_slice(rows);
            return outputs;
        }
    };
    outputs[..lo].copy_from_slice(&rows[..lo]);
    let n = columns - lo;

    // Bit-level generate/propagate; a single-bit column has g = 0, p = bit.
    let mut node: Vec<(Wire, Wire)> = Vec::with_capacity(n);
    let mut bit_p: Vec<Wire> = Vec::with_capacity(n);
    for k in 0..n {
        let c = lo + k;
        let (a, b) = (rows[c], carries[c]);
        if a != Wire::Const0 && b != Wire::Const0 {
            let (g, p) = net.pg(a, b);
            node.push((g, p));
            bit_p.push(p);
        } else {
            let p = if a != Wire::Const0 { a } else { b };
            node.push((Wire::Const0, p));
            bit_p.push(p);
        }
    }
    // Only carries into bits 1..n-1 are consumed, so the group generate of
    // node n-1 is dead; the prune pass removes what only fed it.
    let mut span_low: Vec<usize> = (0..n).collect();

    let combine = |net: &mut Netlist,
                   node: &mut Vec<(Wire, Wire)>,
                   span_low: &mut Vec<usize>,
                   i: usize,
                   j: usize,
                   snapshot: &Vec<(Wire, Wire)>,
                   snapshot_low: &Vec<usize>| {
        // node i absorbs node j (j < i) from the snapshot of the previous level.
        let high = snapshot[i];
        let low = snapshot[j];
        let new_low = snapshot_low[j];
        let need_p = new_low != 0;
        let combined = net.combine(high, low, need_p);
        node[i] = combined;
        span_low[i] = new_low;
    };

    match adder {
        Adder::KoggeStone => {
            let mut d = 1;
            while d < n {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                for i in d..n {
                    if snapshot_low[i] != 0 {
                        combine(
                            net,
                            &mut node,
                            &mut span_low,
                            i,
                            i - d,
                            &snapshot,
                            &snapshot_low,
                        );
                    }
                }
                d *= 2;
            }
        }
        Adder::Sklansky => {
            let mut k = 0;
            while (1 << k) < n {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                for i in 0..n {
                    if (i >> k) & 1 == 1 && snapshot_low[i] != 0 {
                        let j = ((i >> k) << k) - 1;
                        combine(
                            net,
                            &mut node,
                            &mut span_low,
                            i,
                            j,
                            &snapshot,
                            &snapshot_low,
                        );
                    }
                }
                k += 1;
            }
        }
        Adder::BrentKung => {
            // Up-sweep.
            let mut d = 1;
            while d < n {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                let mut i = 2 * d - 1;
                while i < n {
                    combine(
                        net,
                        &mut node,
                        &mut span_low,
                        i,
                        i - d,
                        &snapshot,
                        &snapshot_low,
                    );
                    i += 2 * d;
                }
                d *= 2;
            }
            // Down-sweep.
            let mut d = d / 4;
            while d >= 1 {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                let mut i = 3 * d - 1;
                while i < n {
                    if snapshot_low[i] != 0 {
                        combine(
                            net,
                            &mut node,
                            &mut span_low,
                            i,
                            i - d,
                            &snapshot,
                            &snapshot_low,
                        );
                    }
                    i += 2 * d;
                }
                d /= 2;
            }
        }
        Adder::HanCarlson => {
            // Odd positions absorb their even neighbour, then Kogge-Stone on
            // the odd positions only, then even positions absorb i-1.
            {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                for i in (1..n).step_by(2) {
                    combine(
                        net,
                        &mut node,
                        &mut span_low,
                        i,
                        i - 1,
                        &snapshot,
                        &snapshot_low,
                    );
                }
            }
            let mut d = 2;
            while d < n {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                for i in (1..n).step_by(2) {
                    if i >= d && snapshot_low[i] != 0 {
                        combine(
                            net,
                            &mut node,
                            &mut span_low,
                            i,
                            i - d,
                            &snapshot,
                            &snapshot_low,
                        );
                    }
                }
                d *= 2;
            }
            {
                let snapshot = node.clone();
                let snapshot_low = span_low.clone();
                for i in (2..n).step_by(2) {
                    if snapshot_low[i] != 0 {
                        combine(
                            net,
                            &mut node,
                            &mut span_low,
                            i,
                            i - 1,
                            &snapshot,
                            &snapshot_low,
                        );
                    }
                }
            }
        }
        Adder::Ripple => unreachable!(),
    }

    for (k, &low) in span_low.iter().enumerate().take(n) {
        debug_assert!(k + 1 == n || low == 0, "prefix network incomplete at {k}");
    }

    outputs[lo] = bit_p[0];
    for k in 1..n {
        let carry_in = node[k - 1].0;
        outputs[lo + k] = net.xor2(bit_p[k], carry_in);
    }
    outputs
}

/// Exhaustive check for widths up to 8, seeded random check above.
pub fn verify(net: &Netlist, random_samples: usize, seed: u64) -> Result<(), String> {
    let width = net.width;
    let mask = if width == 64 {
        u64::MAX
    } else {
        (1u64 << width) - 1
    };
    let check = |a: u64, b: u64| -> Result<(), String> {
        let expected = if 2 * width <= 64 {
            (a as u128 * b as u128) as u64
        } else {
            unreachable!()
        };
        let got = net.eval(a, b);
        if got != expected {
            return Err(format!(
                "{} a={a:#x} b={b:#x} expected={expected:#x} got={got:#x}",
                net.module_name()
            ));
        }
        Ok(())
    };
    if width <= 8 {
        let rows = 1usize << (2 * width);
        for base in (0..rows).step_by(64) {
            let mut a_bits = vec![0u64; width];
            let mut b_bits = vec![0u64; width];
            let mut expected = vec![0u64; 2 * width];
            for lane in 0..64 {
                let row = base + lane;
                if row >= rows {
                    break;
                }
                let av = (row & mask as usize) as u64;
                let bv = ((row >> width) & mask as usize) as u64;
                let product = av * bv;
                for bit in 0..width {
                    a_bits[bit] |= ((av >> bit) & 1) << lane;
                    b_bits[bit] |= ((bv >> bit) & 1) << lane;
                }
                for (bit, word) in expected.iter_mut().enumerate() {
                    *word |= ((product >> bit) & 1) << lane;
                }
            }
            let got = net.eval_batch64(&a_bits, &b_bits);
            if let Some(bit) = got.iter().zip(&expected).position(|(x, y)| x != y) {
                let differing = got[bit] ^ expected[bit];
                let lane = differing.trailing_zeros() as usize;
                let row = base + lane;
                let av = (row & mask as usize) as u64;
                let bv = ((row >> width) & mask as usize) as u64;
                return Err(format!(
                    "{} a={av:#x} b={bv:#x} differs at output bit {bit}",
                    net.module_name()
                ));
            }
        }
    } else {
        let mut rng = crate::Rng::new(seed);
        for base in (0..random_samples).step_by(64) {
            let lanes = (random_samples - base).min(64);
            let mut a_bits = vec![0u64; width];
            let mut b_bits = vec![0u64; width];
            let mut expected = vec![0u64; 2 * width];
            let mut pairs = Vec::with_capacity(lanes);
            for lane in 0..lanes {
                let av = rng.next_u64() & mask;
                let bv = rng.next_u64() & mask;
                let product = av.wrapping_mul(bv);
                pairs.push((av, bv));
                for bit in 0..width {
                    a_bits[bit] |= ((av >> bit) & 1) << lane;
                    b_bits[bit] |= ((bv >> bit) & 1) << lane;
                }
                for (bit, word) in expected.iter_mut().enumerate() {
                    *word |= ((product >> bit) & 1) << lane;
                }
            }
            let got = net.eval_batch64(&a_bits, &b_bits);
            if let Some(bit) = got.iter().zip(&expected).position(|(x, y)| x != y) {
                let valid = if lanes == 64 {
                    u64::MAX
                } else {
                    (1u64 << lanes) - 1
                };
                let lane = ((got[bit] ^ expected[bit]) & valid).trailing_zeros() as usize;
                let (av, bv) = pairs[lane];
                return Err(format!(
                    "{} a={av:#x} b={bv:#x} differs at output bit {bit}",
                    net.module_name()
                ));
            }
        }
        // Corners.
        for &a in &[0u64, 1, mask, mask - 1, mask >> 1, (mask >> 1) + 1] {
            for &b in &[0u64, 1, mask, mask - 1, mask >> 1, (mask >> 1) + 1] {
                check(a, b)?;
            }
        }
    }
    Ok(())
}

/// Testbench that checks every generated module of one width against `a * b`.
pub fn testbench_verilog(width: usize, modules: &[String]) -> String {
    let mut v = String::new();
    let _ = writeln!(
        v,
        "// Generated by `cargo run --release -- gen-cores`. Do not edit."
    );
    let _ = writeln!(v, "module tb_mul{width};");
    let _ = writeln!(v, "    reg [{}:0] a, b;", width - 1);
    let _ = writeln!(v, "    wire [{}:0] expected = a * b;", 2 * width - 1);
    for m in modules {
        let _ = writeln!(v, "    wire [{}:0] y_{m};", 2 * width - 1);
        let _ = writeln!(v, "    {m} u_{m}(.a(a), .b(b), .y(y_{m}));");
    }
    let _ = writeln!(v, "    integer i, bad;");
    let _ = writeln!(v, "    task check; begin");
    let _ = writeln!(v, "        #1;");
    for m in modules {
        let _ = writeln!(
            v,
            "        if (y_{m} !== expected) begin bad = bad + 1; if (bad < 8) $display(\"MISMATCH {m} a=%h b=%h expected=%h got=%h\", a, b, expected, y_{m}); end"
        );
    }
    let _ = writeln!(v, "    end endtask");
    let _ = writeln!(v, "    initial begin");
    let _ = writeln!(v, "        bad = 0;");
    if width <= 8 {
        let _ = writeln!(
            v,
            "        for (i = 0; i < {}; i = i + 1) begin a = i[{}:0]; b = i[{}:{}]; check; end",
            1usize << (2 * width),
            width - 1,
            2 * width - 1,
            width
        );
    } else {
        let _ = writeln!(v, "        for (i = 0; i < 200000; i = i + 1) begin a = $urandom; b = $urandom; check; end");
        let _ = writeln!(v, "        a = 0; b = 0; check;");
        let _ = writeln!(
            v,
            "        a = {{{width}{{1'b1}}}}; b = {{{width}{{1'b1}}}}; check;"
        );
        let _ = writeln!(v, "        a = {{{width}{{1'b1}}}}; b = 1; check;");
        let _ = writeln!(v, "        a = 1; b = {{{width}{{1'b1}}}}; check;");
    }
    let _ = writeln!(
        v,
        "        if (bad != 0) begin $display(\"FAIL tb_mul{width} bad=%0d\", bad); $fatal(1); end"
    );
    let _ = writeln!(
        v,
        "        $display(\"GENERATED_CORES_PASS width={width} modules={}\");",
        modules.len()
    );
    let _ = writeln!(v, "        $finish;");
    let _ = writeln!(v, "    end");
    let _ = writeln!(v, "endmodule");
    v
}

/// Yosys script proving one generated module equivalent to `a * b` by SAT.
pub fn equivalence_script(width: usize, module: &str, dir: &str) -> String {
    format!(
        "# Generated by `cargo run --release -- gen-cores`. Do not edit.\n\
read_verilog {dir}/cells.v {dir}/{module}.v\n\
read_verilog <<EOT\n\
module mul_ref{width}(input wire [{hi}:0] a, input wire [{hi}:0] b, output wire [{hi2}:0] y);\n\
    assign y = a * b;\n\
endmodule\n\
EOT\n\
attrmap -modattr -remove keep_hierarchy=1\n\
hierarchy -check\n\
proc\n\
flatten\n\
miter -equiv -flatten -make_outputs mul_ref{width} {module} miter_{module}\n\
hierarchy -top miter_{module}\n\
flatten\n\
opt\n\
sat -verify -prove trigger 0 miter_{module}\n",
        hi = width - 1,
        hi2 = 2 * width - 1
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn routing_proxy_penalizes_a_high_fanout_hub() {
        let mut shared = Netlist::new(2, Reduction::Array, Adder::Ripple);
        let hub = shared.and2(Wire::A(0), Wire::B(0));
        let x = shared.xor2(hub, Wire::A(1));
        let y = shared.xor2(hub, Wire::B(1));
        shared.outputs = vec![x, y, hub];

        let mut local = Netlist::new(2, Reduction::Array, Adder::Ripple);
        let h0 = local.and2(Wire::A(0), Wire::B(0));
        let h1 = local.and2(Wire::A(0), Wire::B(0));
        let x = local.xor2(h0, Wire::A(1));
        let y = local.xor2(h1, Wire::B(1));
        local.outputs = vec![x, y];

        assert!(shared.routing_proxy().max_fanout > local.routing_proxy().max_fanout);
        assert!(shared.routing_proxy().fanout_excess_sq > local.routing_proxy().fanout_excess_sq);
    }

    #[test]
    fn batch64_matches_scalar_evaluator() {
        for &adder in &ADDERS {
            let net = generate(8, Reduction::Dadda, adder);
            let mut a_bits = vec![0u64; 8];
            let mut b_bits = vec![0u64; 8];
            let mut pairs = Vec::new();
            for lane in 0..64 {
                let a = ((lane * 73 + 19) & 0xff) as u64;
                let b = ((lane * 151 + 7) & 0xff) as u64;
                pairs.push((a, b));
                for bit in 0..8 {
                    a_bits[bit] |= ((a >> bit) & 1) << lane;
                    b_bits[bit] |= ((b >> bit) & 1) << lane;
                }
            }
            let batch = net.eval_batch64(&a_bits, &b_bits);
            for (lane, &(a, b)) in pairs.iter().enumerate() {
                let reconstructed = batch
                    .iter()
                    .enumerate()
                    .fold(0u64, |v, (bit, word)| v | (((word >> lane) & 1) << bit));
                assert_eq!(reconstructed, net.eval(a, b));
            }
        }
    }

    #[test]
    fn every_reduction_and_adder_is_exact_at_4_and_8_bits() {
        for &width in &[2usize, 3, 4, 5, 8] {
            for &reduction in &REDUCTIONS {
                for &adder in &ADDERS {
                    let net = generate(width, reduction, adder);
                    verify(&net, 0, 1).unwrap();
                }
            }
        }
    }

    #[test]
    fn wide_instances_pass_random_and_corner_checks() {
        for &width in &[12usize, 16, 32] {
            for &reduction in &REDUCTIONS {
                for &adder in &ADDERS {
                    let net = generate(width, reduction, adder);
                    verify(&net, 20_000, 7).unwrap();
                }
            }
        }
    }

    #[test]
    fn four_bit_array_matches_textbook_cell_count() {
        let net = generate(4, Reduction::Array, Adder::Ripple);
        let counts = net.counts();
        assert_eq!(counts.and2, 16);
        // Sixteen partial products reduce with 4 HA + 8 FA, the top cell
        // being an XOR3 because its carry out is provably zero.
        assert_eq!(counts.ha + counts.fa + counts.xor3, 12);
    }

    #[test]
    fn prefix_adders_are_shallower_than_ripple_at_16_bits() {
        let ripple = generate(16, Reduction::Dadda, Adder::Ripple).counts().depth;
        for &adder in &[
            Adder::KoggeStone,
            Adder::Sklansky,
            Adder::BrentKung,
            Adder::HanCarlson,
        ] {
            let depth = generate(16, Reduction::Dadda, adder).counts().depth;
            assert!(
                depth < ripple,
                "{:?} depth {depth} not below ripple {ripple}",
                adder
            );
        }
    }

    #[test]
    fn dadda_uses_no_more_adders_than_wallace() {
        for &width in &[4usize, 8, 16] {
            let wallace = generate(width, Reduction::Wallace, Adder::Ripple).counts();
            let dadda = generate(width, Reduction::Dadda, Adder::Ripple).counts();
            assert!(dadda.ha + dadda.fa <= wallace.ha + wallace.fa);
        }
    }

    #[test]
    fn verilog_emission_names_every_cell_and_output() {
        let net = generate(4, Reduction::Dadda, Adder::KoggeStone);
        let text = net.to_verilog();
        assert!(text.contains("module mul4_dadda_koggestone("));
        assert_eq!(text.matches("assign y[").count(), 8);
        assert_eq!(text.matches("\n    gen_").count(), net.cells.len());
    }
}
