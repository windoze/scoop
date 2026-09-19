/* Darwin/AArch64 ManagedEntry stubs.
 *
 * Each stub runs before a C prologue can overwrite the direct generated
 * caller context. It appends {LR, SP, FP} to the ordinary arguments and
 * tail-branches to the signature-matched C implementation. The C function
 * therefore returns directly to the generated caller after popping its
 * stack-owned ScoopManagedAnchor. */

.text
.p2align 2

.macro SCOOP_MANAGED_ENTRY_0 public, implementation
.globl _\public
_\public:
    mov x0, x30
    mov x1, sp
    mov x2, x29
    b _\implementation
.endm

.macro SCOOP_MANAGED_ENTRY_1 public, implementation
.globl _\public
_\public:
    mov x1, x30
    mov x2, sp
    mov x3, x29
    b _\implementation
.endm

.macro SCOOP_MANAGED_ENTRY_2 public, implementation
.globl _\public
_\public:
    mov x2, x30
    mov x3, sp
    mov x4, x29
    b _\implementation
.endm

.macro SCOOP_MANAGED_ENTRY_3 public, implementation
.globl _\public
_\public:
    mov x3, x30
    mov x4, sp
    mov x5, x29
    b _\implementation
.endm

.macro SCOOP_MANAGED_ENTRY_4 public, implementation
.globl _\public
_\public:
    mov x4, x30
    mov x5, sp
    mov x6, x29
    b _\implementation
.endm

SCOOP_MANAGED_ENTRY_0 scoop_rt_safepoint, scoop_rt_safepoint_impl
SCOOP_MANAGED_ENTRY_2 scoop_runtime_alloc_slow, scoop_runtime_alloc_slow_impl
SCOOP_MANAGED_ENTRY_0 scoop_rt_gc_collect, scoop_rt_gc_collect_impl
SCOOP_MANAGED_ENTRY_2 scoop_rt_string_concat, scoop_rt_string_concat_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_box_zst, scoop_rt_box_zst_impl
SCOOP_MANAGED_ENTRY_2 scoop_rt_box_value, scoop_rt_box_value_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_materialize_exception, scoop_rt_materialize_exception_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_init_enter, scoop_rt_init_enter_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_init_succeed, scoop_rt_init_succeed_impl
SCOOP_MANAGED_ENTRY_2 scoop_rt_init_fail, scoop_rt_init_fail_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_init_failure, scoop_rt_init_failure_impl
SCOOP_MANAGED_ENTRY_1 scoop_rt_init_cycle_message, scoop_rt_init_cycle_message_impl
SCOOP_MANAGED_ENTRY_3 scoop_rt_array_clone, scoop_rt_array_clone_impl
SCOOP_MANAGED_ENTRY_2 scoop_rt_enter_native_safe, scoop_rt_enter_native_safe_impl
SCOOP_MANAGED_ENTRY_2 scoop_rt_enter_native_borrowed, scoop_rt_enter_native_borrowed_impl

.subsections_via_symbols
