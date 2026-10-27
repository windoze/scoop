#ifndef SCOOP_RT_MANAGED_ENTRIES_H
#define SCOOP_RT_MANAGED_ENTRIES_H

#include <stddef.h>
#include <stdint.h>

#include "generated_entries.h"

/* C implementations reached only through the target ABI's generated-code
 * entry stubs. The stubs append the direct managed caller's return PC,
 * callsite SP and FP before entering here (a Win64 call, otherwise a tail
 * branch). Runtime C code calls its
 * internal operations instead of these entry implementations. */
void scoop_rt_safepoint_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                             uintptr_t frame_pointer);
void *scoop_runtime_alloc_slow_impl(const ScoopTypeDescriptor *td, size_t size,
                                    uintptr_t return_pc, uintptr_t stack_pointer,
                                    uintptr_t frame_pointer);
void scoop_rt_gc_collect_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer);
const ScoopString *scoop_rt_string_concat_impl(const ScoopString *left,
                                               const ScoopString *right,
                                               uintptr_t return_pc,
                                               uintptr_t stack_pointer,
                                               uintptr_t frame_pointer);

uint64_t scoop_rt_init_enter_impl(const ScoopInitializationUnitDescriptorV1 *unit,
                                  uintptr_t return_pc, uintptr_t stack_pointer,
                                  uintptr_t frame_pointer);
void scoop_rt_init_succeed_impl(const ScoopInitializationUnitDescriptorV1 *unit,
                                uintptr_t return_pc, uintptr_t stack_pointer,
                                uintptr_t frame_pointer);
void scoop_rt_init_fail_impl(const ScoopInitializationUnitDescriptorV1 *unit,
                             void *exception, uintptr_t return_pc,
                             uintptr_t stack_pointer, uintptr_t frame_pointer);
void *scoop_rt_init_failure_impl(const ScoopInitializationUnitDescriptorV1 *unit,
                                 uintptr_t return_pc, uintptr_t stack_pointer,
                                 uintptr_t frame_pointer);
const ScoopString *
scoop_rt_init_cycle_message_impl(const ScoopInitializationUnitDescriptorV1 *unit,
                                 uintptr_t return_pc, uintptr_t stack_pointer,
                                 uintptr_t frame_pointer);

void *scoop_rt_context_push_impl(const uint64_t *cell, void *value,
                                const ScoopTypeDescriptor *node_td,
                                uintptr_t return_pc, uintptr_t stack_pointer,
                                uintptr_t frame_pointer);
void *scoop_rt_context_ensure_root_impl(const ScoopTypeDescriptor *task_td,
                                       uintptr_t return_pc, uintptr_t stack_pointer,
                                       uintptr_t frame_pointer);

#endif /* SCOOP_RT_MANAGED_ENTRIES_H */
