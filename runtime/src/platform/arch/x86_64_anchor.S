/* SysV amd64 managed entry: append the direct caller's PC, SP and FP before
 * a C prologue runs. All current entries have at most three explicit words. */
.text

.macro SCOOP_MANAGED_ENTRY public, implementation, pc, sp, fp
.section .text.\public,"ax",@progbits
.p2align 4
.globl \public
.hidden \public
.type \public,@function
\public:
    .cfi_startproc
    movq (%rsp), \pc
    leaq 8(%rsp), \sp
    movq %rbp, \fp
    jmp \implementation
    .cfi_endproc
.size \public, .-\public
.endm

SCOOP_MANAGED_ENTRY scoop_rt_safepoint, scoop_rt_safepoint_impl, %rdi, %rsi, %rdx
SCOOP_MANAGED_ENTRY scoop_rt_gc_collect, scoop_rt_gc_collect_impl, %rdi, %rsi, %rdx
SCOOP_MANAGED_ENTRY scoop_runtime_alloc_slow, scoop_runtime_alloc_slow_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_string_concat, scoop_rt_string_concat_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_box_zst, scoop_rt_box_zst_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_box_value, scoop_rt_box_value_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_materialize_exception, scoop_rt_materialize_exception_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_init_enter, scoop_rt_init_enter_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_init_succeed, scoop_rt_init_succeed_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_init_fail, scoop_rt_init_fail_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_init_failure, scoop_rt_init_failure_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_init_cycle_message, scoop_rt_init_cycle_message_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_array_clone, scoop_rt_array_clone_impl, %rcx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_context_push, scoop_rt_context_push_impl, %rcx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_context_fork, scoop_rt_context_fork_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_context_ensure_root, scoop_rt_context_ensure_root_impl, %rsi, %rdx, %rcx
SCOOP_MANAGED_ENTRY scoop_rt_enter_native_safe, scoop_rt_enter_native_safe_impl, %rdx, %rcx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_enter_native_borrowed, scoop_rt_enter_native_borrowed_impl, %rdx, %rcx, %r8

.section .note.GNU-stack,"",@progbits
