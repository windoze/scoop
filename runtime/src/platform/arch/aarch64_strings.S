/* Adapt Scoop's explicit indirect result ABI to ordinary C pointer parameters. */
.text
.p2align 2
.globl _scoop_rt_string_get
_scoop_rt_string_get:
    mov x2, x1
    mov x1, x0
    mov x0, x8
    b _scoop_rt_string_get_storage

.p2align 2
.globl _scoop_rt_string_slice_bounds
_scoop_rt_string_slice_bounds:
    mov x3, x2
    mov x2, x1
    mov x1, x0
    mov x0, x8
    b _scoop_rt_string_slice_bounds_storage

.p2align 2
.globl _scoop_rt_string_decode_utf8
_scoop_rt_string_decode_utf8:
    mov x2, x1
    mov x1, x0
    mov x0, x8
    b _scoop_rt_string_decode_utf8_storage

.subsections_via_symbols
