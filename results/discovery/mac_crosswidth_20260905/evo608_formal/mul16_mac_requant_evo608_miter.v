module miter(input [15:0] a, b, input [31:0] aux, output bad);
wire [50:0] actual;
mul16_mac_requant_evo608 dut(a,b,aux,actual);
wire [32:0] product = a * b;
wire [32:0] total = product + {1'b0, aux};
wire [32:0] rounded = total + 33'd128;
wire [32:0] scaled = rounded >> 8;
wire saturated = scaled > 33'd65535;
wire [15:0] result = saturated ? 16'd65535 : scaled[15:0];
wire status = saturated | total[32] | (|total[7:0]);
assign bad = actual != {status, saturated, result, total};
endmodule
