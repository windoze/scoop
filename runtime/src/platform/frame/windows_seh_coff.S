/* Win64 managed entry: preserve the caller's PC, callsite RSP and RBP.
 * The private C entry has up to six words: four registers, two stack args.
 * This shim's own frame supplies the required home area and SEH metadata. */
.text

.macro SCOOP_MANAGED_ENTRY public, implementation, pc, sp, fp
.section .text$\public,"xr",one_only,\public
.p2align 4
.globl \public
.def \public; .scl 2; .type 32; .endef
.seh_proc \public
\public:
    subq $56, %rsp
    .seh_stackalloc 56
    .seh_endprologue
    movq 56(%rsp), %rax
    movq %rax, \pc
    leaq 64(%rsp), %rax
    movq %rax, \sp
    movq %rbp, \fp
    callq \implementation
    addq $56, %rsp
    retq
.seh_endproc
.endm

SCOOP_MANAGED_ENTRY scoop_rt_safepoint, scoop_rt_safepoint_impl, %rcx, %rdx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_gc_collect, scoop_rt_gc_collect_impl, %rcx, %rdx, %r8
SCOOP_MANAGED_ENTRY scoop_rt_prepare_throwable, scoop_rt_prepare_throwable_impl, %r8, %r9, 32(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_throwable_print_stack_trace, scoop_rt_throwable_print_stack_trace_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_runtime_alloc_slow, scoop_runtime_alloc_slow_impl, %r8, %r9, 32(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_string_concat, scoop_rt_string_concat_impl, %r8, %r9, 32(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_init_enter, scoop_rt_init_enter_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_init_succeed, scoop_rt_init_succeed_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_init_fail, scoop_rt_init_fail_impl, %r8, %r9, 32(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_init_failure, scoop_rt_init_failure_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_init_cycle_message, scoop_rt_init_cycle_message_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_context_push, scoop_rt_context_push_impl, %r9, 32(%rsp), 40(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_context_ensure_root, scoop_rt_context_ensure_root_impl, %rdx, %r8, %r9
SCOOP_MANAGED_ENTRY scoop_rt_enter_native_safe, scoop_rt_enter_native_safe_impl, %r8, %r9, 32(%rsp)
SCOOP_MANAGED_ENTRY scoop_rt_enter_native_borrowed, scoop_rt_enter_native_borrowed_impl, %r8, %r9, 32(%rsp)
