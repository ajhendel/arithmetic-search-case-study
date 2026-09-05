# OpenSTA: unconstrained longest combinational path of every mapped core.
#
# Usage (inside a container or environment with OpenSTA):
#   sta -exit scripts/sta_cores.tcl
# Environment:
#   LIB      Liberty file (default: sky130_fd_sc_hd__tt_025C_1v80.lib in $PDK_LIB)
#   MAPPED   directory of flattened, Liberty-mapped netlists (results/mapped)
#   OUT      output TSV (results/sta_<corner>.tsv)
#
# Each netlist is a pure combinational block; inputs are driven by an ideal
# source, outputs see a 4-input NAND's worth of load, and the reported number
# is the worst arrival time at any output in picoseconds. This is a ranking
# instrument, not a post-route figure.

set lib     [expr {[info exists ::env(LIB)] ? $::env(LIB) : "sky130_fd_sc_hd__tt_025C_1v80.lib"}]
set mapped  [expr {[info exists ::env(MAPPED)] ? $::env(MAPPED) : "results/mapped"}]
set out     [expr {[info exists ::env(OUT)] ? $::env(OUT) : "results/sta.tsv"}]

read_liberty $lib
set fh [open $out w]
puts $fh "module\tworst_arrival_ps\tworst_output\tarea_um2"

foreach netlist [lsort [glob -nocomplain $mapped/*.v]] {
    set module [file rootname [file tail $netlist]]
    read_verilog $netlist
    link_design $module
    set_input_delay 0 [all_inputs]
    set_load 0.01 [all_outputs]
    set_input_transition 0.05 [all_inputs]
    # Worst unconstrained max path in the block: parse the text report so the
    # same numbers a human would read are the ones recorded.
    set tmp "$out.report"
    report_checks -unconstrained -path_delay max -format full > $tmp
    set fr [open $tmp r]
    set text [read $fr]
    close $fr
    file delete $tmp
    set worst 0.0
    set worst_pin "none"
    regexp {(-?[0-9.]+)\s+data arrival time} $text -> worst
    regexp {Endpoint: (\S+)} $text -> worst_pin
    set area 0.0
    foreach inst [get_cells *] {
        set cell [get_property $inst ref_name]
        set a [get_property [get_lib_cells */$cell] area]
        set area [expr {$area + $a}]
    }
    # OpenSTA reports time in the Liberty unit (ns for sky130); convert to ps.
    puts $fh [format "%s\t%.1f\t%s\t%.2f" $module [expr {$worst * 1000.0}] $worst_pin $area]
    puts [format "%s: worst arrival %.1f ps at %s, area %.2f um2" $module [expr {$worst * 1000.0}] $worst_pin $area]
}
close $fh
