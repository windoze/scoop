#ifndef SCOOP_GC_COPY_METADATA_H
#define SCOOP_GC_COPY_METADATA_H

#include "allocation_metadata.h"

/* No consumer can follow a copied entry until all copy tasks have finished. */
typedef struct ScoopGcCopyMetadata {
    ScoopGcBlockMeta *block;
    size_t word;
    uint64_t starts;
    uint64_t line_live[GC_LINE_BITMAP_WORDS];
    uint64_t bytes_before;
} ScoopGcCopyMetadata;

static inline void scoop_gc_copy_publish_word(ScoopGcCopyMetadata *metadata) {
    metadata->block->marks[metadata->word] |= metadata->starts;
    scoop_heap_publish_allocation_bits(&metadata->block->starts[metadata->word], metadata->starts,
                                       __ATOMIC_RELEASE, true);
    metadata->starts = 0;
}

static inline void scoop_gc_copy_finish_metadata(ScoopGcCopyMetadata *metadata, uint64_t bytes) {
    ScoopGcBlockMeta *block = metadata->block;
    if (block == NULL) {
        return;
    }
    scoop_gc_copy_publish_word(metadata);
    for (size_t index = 0; index < GC_LINE_BITMAP_WORDS; index++) {
        block->line_live[index] |= metadata->line_live[index];
    }
    uint64_t copied = bytes - metadata->bytes_before;
    block->live_bytes += copied;
    block->movable_live_bytes += copied;
    metadata->block = NULL;
}

/* The reserved block has one writer, including when it contains retained objects. */
static inline void scoop_gc_copy_record_metadata(ScoopGcCopyMetadata *metadata,
                                                 ScoopGcBlockMeta *block, size_t offset,
                                                 size_t size, uint64_t bytes_before) {
    if (metadata->block != block) {
        scoop_gc_copy_finish_metadata(metadata, bytes_before);
        *metadata = (ScoopGcCopyMetadata){.block = block, .bytes_before = bytes_before};
    }
    size_t bit = offset / sizeof(uint64_t);
    size_t word = bit / 64;
    if (metadata->word != word && metadata->starts != 0) {
        scoop_gc_copy_publish_word(metadata);
    }
    metadata->word = word;
    metadata->starts |= UINT64_C(1) << (bit % 64);
    size_t first = offset / GC_LINE_SIZE;
    size_t last = (offset + size - 1) / GC_LINE_SIZE;
    for (size_t line = first; line <= last; line++) {
        metadata->line_live[line / 64] |= UINT64_C(1) << (line % 64);
    }
}

#endif
