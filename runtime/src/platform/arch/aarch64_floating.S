/* Adapt Scoop's explicit indirect result ABI to C pointer parameters. */
.text
.p2align 2
.globl _scoop_rt_json_parse_float
_scoop_rt_json_parse_float:
    mov x1, x0
    mov x0, x8
    b _scoop_rt_json_parse_float_storage

.p2align 2
.globl _scoop_rt_json_parse_double
_scoop_rt_json_parse_double:
    mov x1, x0
    mov x0, x8
    b _scoop_rt_json_parse_double_storage

.subsections_via_symbols
