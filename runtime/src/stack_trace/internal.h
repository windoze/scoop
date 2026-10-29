#ifndef SCOOP_STACK_TRACE_INTERNAL_H
#define SCOOP_STACK_TRACE_INTERNAL_H

#include "../stack_trace.h"
#include <stdbool.h>

/* This is private immutable runtime data, never a language or slib format. */
typedef struct ScoopTraceHeader {
    uint32_t frame_count;
    uint32_t module_count;
    uint32_t flags;
    uint32_t reserved;
    uint64_t generation;
} ScoopTraceHeader;

typedef struct ScoopTraceModule {
    uintptr_t image_base;
    intptr_t load_bias;
    uint32_t path_offset;
    uint32_t path_length;
    uint32_t id_kind; /* 0: unavailable; 1: Mach-O UUID; 2: ELF build-id */
    uint32_t id_length;
    unsigned char id[SCOOP_TRACE_MAX_ID];
} ScoopTraceModule;

typedef struct ScoopTraceLoadedModule {
    ScoopTraceModule identity;
    char *path;
} ScoopTraceLoadedModule;

/* Called in NativeSafe; copies loader-owned names/identities before returning. */
uint32_t scoop_trace_modules(ScoopTraceFrames *frames, ScoopTraceLoadedModule *modules,
                             uint64_t *generation);
uint64_t scoop_trace_generation(void);

typedef struct ScoopTraceSymbol {
    char *function;
    char *filename;
    int line;
    struct ScoopTraceSymbol *next;
} ScoopTraceSymbol;

typedef struct ScoopTraceSymbols {
    ScoopTraceSymbol *first;
    ScoopTraceSymbol *last;
    bool changed;
    bool unavailable;
} ScoopTraceSymbols;

void scoop_trace_symbolize(uint64_t generation, uintptr_t pc, ScoopTraceSymbols *symbols);
void scoop_trace_symbols_free(ScoopTraceSymbols *symbols);
char *scoop_trace_copy_path(const char *path);

#endif
