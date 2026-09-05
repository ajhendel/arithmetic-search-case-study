module miter(input [7:0] a, b, input [15:0] aux, output bad);
wire [26:0] actual;
mul8_mac_requant_evo608_norules dut(a,b,aux,actual);
wire [16:0] product = a * b;
wire [16:0] total = product + {1'b0, aux};
wire [16:0] rounded = total + 17'd8;
wire [16:0] scaled = rounded >> 4;
wire saturated = scaled > 17'd255;
wire [7:0] result = saturated ? 8'd255 : scaled[7:0];
wire status = saturated | total[16] | (|total[3:0]);
assign bad = actual != {status, saturated, result, total};
endmodule
