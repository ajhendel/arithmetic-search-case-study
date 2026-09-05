read_liberty $::env(LIB)
set fh [open "$::env(OUT)/arcs.tsv" w]
puts $fh "module\tinput\toutput\tdelay_ns"
foreach netlist [lsort [glob $::env(OUT)/mapped/*.v]] {
 set top [file rootname [file tail $netlist]]
 read_verilog $netlist
 link_design $top
 set_input_delay 0 [all_inputs]
 set_input_transition 0.05 [all_inputs]
 set_load 0.01 [all_outputs]
 foreach pin [all_inputs] {
  set name [get_full_name $pin]
  foreach output {s c} {
   set report "$::env(OUT)/${top}_${name}_${output}.rpt"
   report_checks -unconstrained -from $pin -to [get_ports $output] -path_delay max -format full -digits 6 > $report
   set fr [open $report r]
   set content [read $fr]
   close $fr
   if {![regexp {([0-9.]+)\s+data arrival time} $content -> delay]} { error "missing arc $name $output" }
   puts $fh "$top\t$name\t$output\t$delay"
  }
 }
}
close $fh
