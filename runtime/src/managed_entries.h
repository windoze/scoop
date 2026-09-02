#ifndef SCOOP_RT_MANAGED_ENTRIES_H
#define SCOOP_RT_MANAGED_ENTRIES_H

#include <stddef.h>
#include <stdint.h>

#include "scoop_rt.h"

/* C implementations reached only through the target ABI's generated-code
 * entry stubs. The stubs append the direct managed caller's return PC,
 * callsite SP and FP before tail-branching here. Runtime C code calls its
 * internal operations instead of these entry implementations. */
void scoop_rt_safepoint_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                             uintptr_t frame_pointer);
void *scoop_runtime_alloc_slow_impl(const ScoopTypeDescriptor *td, size_t size,
                                    uintptr_t return_pc,
                                    uintptr_t stack_pointer,
                                    uintptr_t frame_pointer);
void scoop_rt_gc_collect_impl(uintptr_t return_pc, uintptr_t stack_pointer,
                              uintptr_t frame_pointer);
const ScoopString *scoop_rt_string_concat_impl(
    const ScoopString *left, const ScoopString *right, uintptr_t return_pc,
    uintptr_t stack_pointer, uintptr_t frame_pointer);
void *scoop_rt_box_impl(const ScoopTypeDescriptor *td, const void *payload,
                        uint64_t payload_size, const uint64_t *payload_scan,
                        uintptr_t return_pc, uintptr_t stack_pointer,
                        uintptr_t frame_pointer);
void *scoop_rt_materialize_exception_impl(const void *caught,
                                          uintptr_t return_pc,
                                          uintptr_t stack_pointer,
                                          uintptr_t frame_pointer);
const void *scoop_rt_array_clone_impl(const void *object,
                                      const ScoopTypeDescriptor *target_td,
                                      uint64_t element_size,
                                      uint64_t data_offset,
                                      uintptr_t return_pc,
                                      uintptr_t stack_pointer,
                                      uintptr_t frame_pointer);

#endif /* SCOOP_RT_MANAGED_ENTRIES_H */
