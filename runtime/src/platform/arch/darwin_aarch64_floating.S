/* Return the tagged value as two integer parts, preserving payload bits. */
.text
.p2align 2
.globl _scoop_rt_json_parse_float
_scoop_rt_json_parse_float:
    stp x29, x30, [sp, #-32]!
    mov x29, sp
    mov x1, x0
    add x0, sp, #16
    bl _scoop_rt_json_parse_float_storage
    ldp x0, x1, [sp, #16]
    ldp x29, x30, [sp], #32
    ret

.p2align 2
.globl _scoop_rt_json_parse_double
_scoop_rt_json_parse_double:
    stp x29, x30, [sp, #-32]!
    mov x29, sp
    mov x1, x0
    add x0, sp, #16
    bl _scoop_rt_json_parse_double_storage
    ldp x0, x1, [sp, #16]
    ldp x29, x30, [sp], #32
    ret

.subsections_via_symbols
