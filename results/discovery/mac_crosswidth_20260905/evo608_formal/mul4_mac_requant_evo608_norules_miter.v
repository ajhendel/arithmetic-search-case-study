module miter(input [3:0] a, b, input [7:0] aux, output bad);
wire [14:0] actual;
mul4_mac_requant_evo608_norules dut(a,b,aux,actual);
wire [8:0] product = a * b;
wire [8:0] total = product + {1'b0, aux};
wire [8:0] rounded = total + 9'd2;
wire [8:0] scaled = rounded >> 2;
wire saturated = scaled > 9'd15;
wire [3:0] result = saturated ? 4'd15 : scaled[3:0];
wire status = saturated | total[8] | (|total[1:0]);
assign bad = actual != {status, saturated, result, total};
endmodule
