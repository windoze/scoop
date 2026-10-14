#ifndef SCOOP_GC_BINDING_INTERNAL_H
#define SCOOP_GC_BINDING_INTERNAL_H

#include "../gc_binding.h"
#include "../image/registry.h"
#include "stackmap.h"

struct ScoopThreadState;

/* Startup publishes the complete, already resolved loaded-image index. */
void scoop_rt_gc_init(const ScoopImageRegistry *registry);
void scoop_gc_stackmaps_init(const ScoopStackMapIndex *index);
const ScoopStackMapRecord *scoop_gc_stackmap_lookup(uintptr_t return_pc);
void scoop_gc_visit_managed_stack(const struct ScoopThreadState *thread,
                                  ScoopGcRootVisitor visitor);
void scoop_gc_visit_managed_segment(const struct ScoopThreadState *thread, uintptr_t return_pc,
                                    uintptr_t stack_pointer, uintptr_t frame_pointer,
                                    uintptr_t managed_boundary, ScoopGcRootVisitor visitor);
#endif
