// Independent behavioral specification for the fixed unsigned W=24 contract.
module mac24_reference(input wire [23:0] a, b,
                       input wire [47:0] aux, output wire [74:0] y);
    wire [48:0] product = a * b;
    wire [48:0] total = product + {1'b0, aux};
    wire [48:0] rounded = total + 49'd2048;
    wire [48:0] scaled = rounded >> 12;
    wire saturated = scaled > 49'd16777215;
    wire [23:0] result = saturated ? 24'hffffff : scaled[23:0];
    wire status = saturated | total[48] | (|total[11:0]);
    assign y = {status, saturated, result, total};
endmodule
