# Place and globally route one frozen Liberty-mapped combinational core.
# Environment: NETLIST, TOP, OUT, optional UTILIZATION (default 45).
set platform /OpenROAD-flow-scripts/flow/platforms/sky130hd
set netlist $::env(NETLIST)
set top $::env(TOP)
set out $::env(OUT)
set util [expr {[info exists ::env(UTILIZATION)] ? $::env(UTILIZATION) : 45}]

read_lef $platform/lef/sky130_fd_sc_hd.tlef
read_lef $platform/lef/sky130_fd_sc_hd_merged.lef
read_liberty $platform/lib/sky130_fd_sc_hd__tt_025C_1v80.lib
read_verilog $netlist
link_design $top
source $platform/setRC.tcl

initialize_floorplan -utilization $util -aspect_ratio 1.0 -core_space 5 -site unithd
source $platform/make_tracks.tcl
place_pins -hor_layers met3 -ver_layers met2
set_input_transition 0.05 [all_inputs]
set_load 0.01 [all_outputs]
set_max_delay 10.0 -from [all_inputs] -to [all_outputs]
global_placement -density 0.55
detailed_placement
set_routing_layers -signal met1-met5
global_route
if {[info exists ::env(DETAILED)] && $::env(DETAILED) == 1} {
    detailed_route -output_drc $out.drc.rpt
    extract_parasitics -lef_rc
    write_spef $out.spef
} else {
    estimate_parasitics -global_routing
}

report_checks -unconstrained -path_delay max -format full > $out.checks.rpt
report_design_area
write_def $out.def
write_db $out.odb
