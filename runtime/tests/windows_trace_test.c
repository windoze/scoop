#include "stack_trace/windows.h"

#include <assert.h>
#include <stdlib.h>
#include <string.h>

static __declspec(noinline) void take_snapshot(ScoopTraceBuffer *trace) {
    ScoopTraceFrames frames;
    scoop_trace_capture_frames(&frames, (uintptr_t)__builtin_return_address(0));
    assert(frames.count >= 2);
    assert((frames.flags & SCOOP_TRACE_BOUNDARY_MISSING) == 0);
    *trace = scoop_trace_complete(&frames);
}

__declspec(noinline) void symbol_marker(ScoopTraceBuffer *trace) { take_snapshot(trace); }

static DWORD WINAPI lookup_worker(void *argument) {
    uint64_t generation = *(uint64_t *)argument;
    for (int index = 0; index < 16; index++) {
        ScoopTraceSymbols symbols = {0};
        scoop_trace_symbolize(generation, (uintptr_t)symbol_marker, &symbols);
        assert(!symbols.changed && !symbols.unavailable && symbols.first != NULL);
        assert(strcmp(symbols.first->function, "symbol_marker") == 0);
        assert(symbols.first->filename != NULL && symbols.first->line > 0);
        scoop_trace_symbols_free(&symbols);
    }
    return 0;
}

int wmain(int argc, wchar_t **argv) {
    assert(argc == 4);
    ScoopTraceBuffer trace;
    symbol_marker(&trace);
    const ScoopTraceHeader *header = trace.bytes;
    assert(header->module_count > 0 && header->generation != 0);
    const ScoopTraceFrame *frames = (const void *)(header + 1);
    const ScoopTraceModule *modules = (const void *)(frames + header->frame_count);
    assert(frames[0].module != SCOOP_TRACE_NO_MODULE);
    const ScoopTraceModule *module = &modules[frames[0].module];
    assert(module->image_base == (uintptr_t)GetModuleHandleW(NULL));
    assert(frames[0].link_pc == frames[0].query_pc - module->load_bias);
    assert(strstr((const char *)trace.bytes + module->path_offset, "scoop trace ") != NULL);

    ScoopTraceSymbols symbols = {0};
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    if (symbols.changed || symbols.unavailable || symbols.first == NULL)
        fprintf(stderr, "symbol lookup: changed=%d unavailable=%d generation=%llu current=%llu\n",
                symbols.changed, symbols.unavailable, (unsigned long long)header->generation,
                (unsigned long long)scoop_trace_generation());
    if (wcscmp(argv[3], L"addresses") == 0) {
        assert(symbols.first == NULL && symbols.unavailable);
        scoop_trace_render(trace.bytes, stdout);
        free(trace.bytes);
        return 0;
    }
    assert(!symbols.changed && !symbols.unavailable && symbols.first != NULL);
    assert(module->id_kind == 3 && module->id_length == 20);
    assert(strcmp(symbols.first->function, "symbol_marker") == 0);
    char *source = scoop_trace_utf8(argv[2]);
    assert(strcmp(symbols.first->filename, source) == 0 && symbols.first->line > 0);
    free(source);
    scoop_trace_symbols_free(&symbols);

    HANDLE threads[4];
    uint64_t generation = header->generation;
    for (size_t index = 0; index < 4; index++) {
        threads[index] = CreateThread(NULL, 0, lookup_worker, &generation, 0, NULL);
        assert(threads[index] != NULL);
    }
    assert(WaitForMultipleObjects(4, threads, TRUE, INFINITE) == WAIT_OBJECT_0);
    for (size_t index = 0; index < 4; index++)
        CloseHandle(threads[index]);

    void *original = scoop_trace_allocate(trace.length);
    memcpy(original, trace.bytes, trace.length);
    HMODULE library = LoadLibraryW(argv[1]);
    assert(library != NULL);
    symbols = (ScoopTraceSymbols){0};
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    assert(!symbols.changed && !symbols.unavailable && symbols.first != NULL);
    scoop_trace_symbols_free(&symbols);
    FARPROC added = GetProcAddress(library, "trace_library_marker");
    assert(added != NULL);
    symbols = (ScoopTraceSymbols){0};
    scoop_trace_symbolize(header->generation, (uintptr_t)added, &symbols);
    assert(!symbols.changed && !symbols.unavailable && symbols.first != NULL);
    assert(strcmp(symbols.first->function, "trace_library_marker") == 0);
    scoop_trace_symbols_free(&symbols);
    assert(FreeLibrary(library));
    symbols = (ScoopTraceSymbols){0};
    scoop_trace_symbolize(header->generation, (uintptr_t)symbol_marker, &symbols);
    assert(symbols.changed && symbols.first == NULL);
    assert(memcmp(original, trace.bytes, trace.length) == 0);
    scoop_trace_render(trace.bytes, stdout);
    free(original);
    free(trace.bytes);
    puts("Windows PDB traces and loader changes passed");
    return 0;
}
