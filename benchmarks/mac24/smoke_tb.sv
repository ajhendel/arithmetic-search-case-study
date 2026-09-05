`timescale 1ns/1ps
module smoke_tb;
    reg [23:0] a, b;
    reg [47:0] aux;
    wire [74:0] candidate, sibling, expected;
    integer count = 0;
    integer i;
    reg [31:0] state = 32'h60824012;
    mul24_mac_replay_candidate dut(a, b, aux, candidate);
    mul24_mac_replay_norules ablation(a, b, aux, sibling);
    mac24_reference reference_model(a, b, aux, expected);

    function [31:0] next_random(input [31:0] x);
        reg [31:0] v;
        begin
            v = x ^ (x << 13);
            v = v ^ (v >> 17);
            next_random = v ^ (v << 5);
        end
    endfunction

    task check(input [23:0] av, bv, input [47:0] cv);
        begin
            a = av; b = bv; aux = cv;
            #1;
            if (candidate !== expected || sibling !== expected)
                $fatal(1, "Mismatch a=%h b=%h acc=%h got=%h sibling=%h ref=%h",
                       a, b, aux, candidate, sibling, expected);
            count = count + 1;
        end
    endtask

    initial begin
        check(0, 0, 0);
        check(24'hffffff, 24'hffffff, 48'hffffffffffff);
        check(0, 0, 48'hffffffffffff);
        // Values immediately around rounding and saturation transitions.
        check(0, 0, 48'd2047);
        check(0, 0, 48'd2048);
        check(0, 0, 48'd2049);
        check(0, 0, (48'd16777215 << 12) - 48'd2049);
        check(0, 0, (48'd16777215 << 12) - 48'd2048);
        check(0, 0, (48'd16777216 << 12) - 48'd2049);
        check(0, 0, (48'd16777216 << 12) - 48'd2048);
        // Sparse bits and carry propagation across every accumulator bit.
        for (i = 0; i < 48; i = i + 1) begin
            check(1, 1, (48'd1 << i) - 1);
            check(24'hffffff, 1, 48'd1 << i);
        end
        for (i = 0; i < 2000; i = i + 1) begin
            state = next_random(state); a = state[23:0];
            state = next_random(state); b = state[23:0];
            state = next_random(state); aux[31:0] = state;
            state = next_random(state); aux[47:32] = state[15:0];
            check(a, b, aux);
            // Broad random products saturate almost always. Also exercise
            // the unsaturated arithmetic and rounding with smaller operands.
            check(a & 24'h000fff, b & 24'h000fff, aux & 48'h000000ffffff);
        end
        $display("PASS: %0d vectors, all 75 bits, candidate and sibling", count);
        $finish;
    end
endmodule
