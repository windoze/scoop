#ifndef SCOOP_RT_GENERATED_ENTRIES_H
#define SCOOP_RT_GENERATED_ENTRIES_H

#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"

/* Private ABI shared by generated Scoop code and the runtime. Native FFI
 * implementations must include only runtime/include/scoop_rt.h and cannot
 * call any entry declared here directly. */

/* Per-thread TLAB ABI used only by generated allocation sequences. */
typedef struct ScoopAllocationContext {
    char *cursor;
    char *limit;
} ScoopAllocationContext;

extern _Thread_local ScoopAllocationContext *scoop_rt_allocation_context;

void scoop_runtime_finish_tlab_alloc(void *object, const ScoopTypeDescriptor *td,
                                     size_t size);

/* ManagedEntry functions. The target-specific entry stub captures the direct
 * managed caller before branching to the corresponding RuntimeInternal
 * implementation declared in managed_entries.h. */
void *scoop_runtime_alloc_slow(const ScoopTypeDescriptor *td, size_t size);
void scoop_rt_safepoint(void);
const ScoopString *scoop_rt_string_concat(const ScoopString *left,
                                          const ScoopString *right);
const void *scoop_rt_array_clone(const void *object,
                                 const ScoopTypeDescriptor *source_td,
                                 const ScoopTypeDescriptor *target_td);
void *scoop_rt_box_zst(const ScoopTypeDescriptor *td);
void *scoop_rt_box_value(const ScoopTypeDescriptor *td, const void *source);
/* NoGC leaves: exact type validation and copying cannot safepoint. */
void scoop_rt_unbox_zst(const void *object, const ScoopTypeDescriptor *expected_td);
void scoop_rt_unbox_value(const void *object, const ScoopTypeDescriptor *expected_td,
                          void *destination);
void scoop_rt_gc_collect(void);
void *scoop_rt_materialize_exception(const void *caught);
_Noreturn void scoop_rt_throw(const void *object);
_Noreturn void scoop_rt_rethrow(void);
void *scoop_rt_begin_catch(void *raw_exception);
void scoop_rt_end_catch(void);
uint64_t scoop_rt_init_enter(const ScoopInitializationUnitDescriptorV1 *unit);
void scoop_rt_init_succeed(const ScoopInitializationUnitDescriptorV1 *unit);
void scoop_rt_init_fail(const ScoopInitializationUnitDescriptorV1 *unit,
                        void *exception);
void *scoop_rt_init_failure(const ScoopInitializationUnitDescriptorV1 *unit);
const ScoopString *
scoop_rt_init_cycle_message(const ScoopInitializationUnitDescriptorV1 *unit);

/* Compiler-published roots that remain live across one outbound native call.
 * Each entry scans one addressable value using a generated recursive scan. */
typedef struct ScoopCallerRootEntry {
    void *base;
    const uint64_t *scan;
} ScoopCallerRootEntry;

typedef struct ScoopCallerRootFrame {
    struct ScoopCallerRootFrame *previous;
    ScoopCallerRootEntry *entries;
    uint64_t count;
} ScoopCallerRootFrame;

/* Roots spilled around a managed invoke. This chain is independent from the
 * outbound-native caller-root chain and is unwound dynamically. */
typedef struct ScoopCompilerRootFrame {
    struct ScoopCompilerRootFrame *previous;
    ScoopCallerRootEntry *entries;
    uint64_t count;
} ScoopCompilerRootFrame;

/* Stack-owned outbound transition record. Generated code treats every field
 * as opaque while the transition is active. */
typedef struct ScoopThreadTransition {
    struct ScoopThreadTransition *previous;
    ScoopCallerRootFrame *caller_roots;
    uintptr_t managed_return_pc;
    uintptr_t managed_stack_pointer;
    uintptr_t managed_frame_pointer;
    uintptr_t managed_stack_low;
    uintptr_t managed_stack_high;
    uint32_t previous_mode;
    uint32_t native_mode;
} ScoopThreadTransition;

void scoop_rt_push_caller_roots(ScoopCallerRootFrame *frame,
                                ScoopCallerRootEntry *entries, uint64_t count);
void scoop_rt_pop_caller_roots(ScoopCallerRootFrame *frame);
void scoop_rt_push_compiler_roots(ScoopCompilerRootFrame *frame,
                                  ScoopCallerRootEntry *entries, uint64_t count);
void scoop_rt_pop_compiler_roots(ScoopCompilerRootFrame *frame);
void scoop_rt_pop_top_compiler_roots(void);
void scoop_rt_enter_native_safe(ScoopThreadTransition *transition,
                                uintptr_t managed_stack_low);
void scoop_rt_leave_native_safe(ScoopThreadTransition *transition);
void scoop_rt_enter_native_borrowed(ScoopThreadTransition *transition,
                                    uintptr_t managed_stack_low);
void scoop_rt_leave_native_borrowed(ScoopThreadTransition *transition);

/* Generated write barriers mark this pre-biased card table after heap stores. */
extern unsigned char *scoop_gc_card_table;

/* Other compiler/runtime ABI declarations that are not part of the native
 * FFI-author surface. */
void scoop_rt_gc_add_root(void **slot);
void scoop_rt_gc_add_root_object(const void *object);
void scoop_rt_gc_remove_root_object(const void *object);

#endif /* SCOOP_RT_GENERATED_ENTRIES_H */
