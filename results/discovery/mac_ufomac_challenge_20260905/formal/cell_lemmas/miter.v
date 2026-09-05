module miter(input a, b, c, output bad);
wire pp, hs, hc, fs, fc, ms, mc;
gen_and2 pp_dut(a, b, pp);
gen_ha ha_dut(a, b, hs, hc);
gen_fa fa_dut(a, b, c, fs, fc);
gen_famux mux_dut(a, b, c, ms, mc);
wire [2:0] two_inputs = {2'b0,a} + {2'b0,b};
wire [2:0] three_inputs = two_inputs + {2'b0,c};
assign bad = (pp !== (a & b)) | ({1'b0,hc,hs} !== two_inputs)
           | ({1'b0,fc,fs} !== three_inputs) | ({1'b0,mc,ms} !== three_inputs);
endmodule
