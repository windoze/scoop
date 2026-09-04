#ifndef SCOOP_RT_GENERATED_ENTRIES_H
#define SCOOP_RT_GENERATED_ENTRIES_H

#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"

/* Private ABI shared by generated Scoop code and the runtime. Native FFI
 * implementations must include only runtime/include/scoop_rt.h and cannot
 * call any entry declared here directly. */

/* Image metadata emitted exactly once by codegen and consumed during runtime
 * initialization. A zero-count table still contains one null sentinel. */
typedef struct ScoopManagedGlobalDescriptor {
    void *writable_base;
    const uint64_t *scan;
} ScoopManagedGlobalDescriptor;

typedef struct ScoopImmortalObjectDescriptor {
    const void *object_start;
    uint64_t object_size;
    const ScoopTypeDescriptor *td;
} ScoopImmortalObjectDescriptor;

extern const ScoopManagedGlobalDescriptor scoop_image_managed_globals[];
extern const uint64_t scoop_image_managed_global_count;
extern const ScoopImmortalObjectDescriptor scoop_image_immortal_objects[];
extern const uint64_t scoop_image_immortal_object_count;

typedef struct ScoopInitializationCell {
    uint64_t state;
    void *owner_thread;
} ScoopInitializationCell;

typedef struct ScoopInitializationUnitDescriptor {
    uint64_t schedule;
    const char *stable_key;
    ScoopInitializationCell *cell;
    void *storage;
    void **failure_root;
    void (*initializer_entry)(void);
    void (*ensure_entry)(void);
} ScoopInitializationUnitDescriptor;

enum {
    SCOOP_INIT_EAGER_STARTUP = 0,
    SCOOP_INIT_LAZY_ACCESS = 1,
};

extern const ScoopInitializationUnitDescriptor scoop_image_initialization_units[];
extern const uint64_t scoop_image_initialization_unit_count;

/* Per-thread TLAB ABI used only by generated allocation sequences. */
typedef struct ScoopAllocationContext {
    char *cursor;
    char *limit;
} ScoopAllocationContext;

extern _Thread_local ScoopAllocationContext *scoop_rt_allocation_context;

void scoop_runtime_finish_tlab_alloc(void *object,
                                     const ScoopTypeDescriptor *td,
                                     size_t size);

/* ManagedEntry functions. The target-specific entry stub captures the direct
 * managed caller before branching to the corresponding RuntimeInternal
 * implementation declared in managed_entries.h. */
void *scoop_runtime_alloc_slow(const ScoopTypeDescriptor *td, size_t size);
void scoop_rt_safepoint(void);
const ScoopString *scoop_rt_string_concat(const ScoopString *left,
                                          const ScoopString *right);
const void *scoop_rt_array_clone(const void *object,
                                 const ScoopTypeDescriptor *target_td,
                                 uint64_t element_size,
                                 uint64_t data_offset);
void *scoop_rt_box(const ScoopTypeDescriptor *td, const void *payload,
                   uint64_t payload_size, const uint64_t *payload_scan);
void scoop_rt_gc_collect(void);
void *scoop_rt_materialize_exception(const void *caught);
uint64_t scoop_rt_init_enter(const ScoopInitializationUnitDescriptor *unit);
void scoop_rt_init_succeed(const ScoopInitializationUnitDescriptor *unit);
void scoop_rt_init_fail(const ScoopInitializationUnitDescriptor *unit,
                        void *exception);
void *scoop_rt_init_failure(const ScoopInitializationUnitDescriptor *unit);
const ScoopString *
scoop_rt_init_cycle_message(const ScoopInitializationUnitDescriptor *unit);

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
                                ScoopCallerRootEntry *entries,
                                uint64_t count);
void scoop_rt_pop_caller_roots(ScoopCallerRootFrame *frame);
void scoop_rt_push_compiler_roots(ScoopCompilerRootFrame *frame,
                                  ScoopCallerRootEntry *entries,
                                  uint64_t count);
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
void scoop_rt_gc_init(void);
void scoop_rt_gc_add_root(void **slot);
void scoop_rt_gc_add_root_object(const void *object);
void scoop_rt_gc_remove_root_object(const void *object);
void scoop_rt_init_eh(void);
void scoop_rt_initialize_image(void);
int scoop_eh_personality(int version, unsigned int actions,
                         unsigned long long exception_class, void *exception,
                         void *context);
void scoop_main(void);

#endif /* SCOOP_RT_GENERATED_ENTRIES_H */
