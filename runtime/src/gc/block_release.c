/* Retire block storage only after all roots and outgoing references are updated. */
#include <stdlib.h>

#include "heap_internal.h"
#include "../platform/platform.h"

static void retire_metadata(ScoopGcBlockMeta *block, ScoopGcBlockState state) {
    if (!active_head(block)) {
        heap_fatal("attempted to retire an inactive block");
    }
    free(block->forwarding);
    block->forwarding = NULL;
    block->state = state;
    block->kind = SCOOP_BLOCK_KIND_NONE;
    block->generation = SCOOP_GC_OLD;
    block->exact_size = 0;
    block->live_bytes = 0;
    block->movable_live_bytes = 0;
    block->large_forwarding = NULL;
    block->large_published = false;
    block->large_marked = false;
    block->large_pinned = false;
    block->large_scanned = false;
    committed_bytes -= scoop_heap_block_bytes(block);
    active_block_heads--;
}

void scoop_heap_release_block(ScoopGcBlockMeta *block) {
    retire_metadata(block, SCOOP_BLOCK_FREE);
    if (!block->region->large) {
        block->next_free = scoop_gc_heap_state.free_blocks;
        scoop_gc_heap_state.free_blocks = block;
    }
}

void scoop_heap_quarantine_block(ScoopGcBlockMeta *block) {
    const ScoopThreadVmOps *vm = scoop_platform_bundle()->thread_vm;
    size_t page_size = vm->page_size();
    void *base = block_base(block);
    size_t size = scoop_heap_block_bytes(block);
    if (page_size == 0 || (uintptr_t)base % page_size != 0 || size % page_size != 0) {
        heap_fatal("GC block storage is not compatible with platform page protection");
    }
    ScoopPlatformError error = {0};
    if (!vm->protect_none(base, size, &error)) {
        heap_fatal(scoop_platform_error_message(error.code));
    }
    retire_metadata(block, SCOOP_BLOCK_QUARANTINED);
}
