#include "stack_trace/internal.h"
#include <assert.h>
#include <dlfcn.h>
#include <stdlib.h>
#include <string.h>

static volatile int observed;
void symbol_marker(void) __asm__("scoop$1$trace_test");
__attribute__((noinline)) void symbol_marker(void) { observed++; }

int main(int argc, char **argv) {
    assert(argc == 3);
    symbol_marker();
    ScoopTraceFrames frames = {.count = 1};
    frames.frames[0] = (ScoopTraceFrame){
        .raw_pc = (uintptr_t)symbol_marker,
        .query_pc = (uintptr_t)symbol_marker,
        .module = SCOOP_TRACE_NO_MODULE,
    };
    ScoopTraceBuffer trace = scoop_trace_complete(&frames);
    ScoopTraceHeader *header = trace.bytes;
    void *original = scoop_trace_allocate(trace.length);
    memcpy(original, trace.bytes, trace.length);
    ScoopTraceSymbols symbols = {0};
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    assert(!symbols.changed && !symbols.unavailable);
    assert(symbols.first != NULL);
    assert(strcmp(symbols.first->function, "symbol_marker") == 0);
    assert(strcmp(symbols.first->filename, argv[2]) == 0);
    assert(symbols.first->line > 0);
    scoop_trace_symbols_free(&symbols);

    void *library = dlopen(argv[1], RTLD_NOW | RTLD_LOCAL);
    assert(library != NULL);
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    assert(symbols.changed && symbols.first == NULL);
    assert(dlclose(library) == 0);
    symbols = (ScoopTraceSymbols){0};
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    assert(symbols.changed && symbols.first == NULL);
    assert(memcmp(original, trace.bytes, trace.length) == 0);
    free(original);
    free(trace.bytes);
    puts("trace symbols and loader changes passed");
    return 0;
}
