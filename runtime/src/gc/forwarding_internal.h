#ifndef SCOOP_GC_FORWARDING_INTERNAL_H
#define SCOOP_GC_FORWARDING_INTERNAL_H

/* Included after heap_internal.h; one implementation for both call strategies. */
#if SCOOP_PLATFORM_INLINE_GC_FORWARDING
static inline
#endif
    void *scoop_heap_forward_object(void *object, const ScoopGcBlockMeta *source,
                                    size_t word_index) {
    if (!collection_active) {
        heap_fatal("object forwarded outside collection");
    }
    if (!object_marked(source, word_index)) {
        heap_fatal("unmarked object reached relocation");
    }
    if (source->state != SCOOP_BLOCK_EVACUATION_SOURCE || object_pinned(source, word_index)) {
        return object;
    }
    void *forwarded = source->kind == SCOOP_BLOCK_KIND_LARGE ? source->large_forwarding
                                                             : source->forwarding[word_index];
    if (forwarded == object) {
        heap_fatal("evacuation source forwards to its own address");
    }
    if (forwarded != NULL) {
        return forwarded;
    }
    heap_fatal("marked evacuation source has no planned forwarding address");
}

#endif /* SCOOP_GC_FORWARDING_INTERNAL_H */
