#include "windows.h"

#include <dbghelp.h>
#include <stdlib.h>

/* DbgHelp is single threaded, including initialization and buffer ownership. */
static SRWLOCK symbols_lock = SRWLOCK_INIT;
static bool symbols_initialized;
static HANDLE symbols_process;
static uint64_t symbols_generation;
static bool symbols_stable;

static void initialize_symbols(void) {
    symbols_initialized = true;
    symbols_generation = scoop_trace_generation();
    HANDLE process = GetCurrentProcess();
    if (!DuplicateHandle(process, process, process, &symbols_process, 0, FALSE,
                         DUPLICATE_SAME_ACCESS))
        return;
    SymSetOptions(SYMOPT_DEFERRED_LOADS | SYMOPT_LOAD_LINES | SYMOPT_UNDNAME |
                  SYMOPT_FAIL_CRITICAL_ERRORS | SYMOPT_NO_PROMPTS);
    if (!SymInitializeW(symbols_process, NULL, FALSE)) {
        CloseHandle(symbols_process);
        symbols_process = NULL;
    }
    symbols_stable = symbols_generation != 0 && symbols_generation == scoop_trace_generation();
}

void scoop_trace_symbolize(uint64_t generation, uintptr_t pc, ScoopTraceSymbols *symbols) {
    AcquireSRWLockExclusive(&symbols_lock);
    if (!symbols_initialized)
        initialize_symbols();
    if (generation == 0 || symbols_generation == 0 || symbols_process == NULL) {
        symbols->unavailable = true;
    } else if (!symbols_stable || generation != symbols_generation ||
               generation != scoop_trace_generation()) {
        symbols->changed = true;
    } else {
        HMODULE module = NULL;
        if (GetModuleHandleExW(GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, (LPCWSTR)pc, &module)) {
            WCHAR *path = scoop_trace_module_path(module);
            if (path != NULL) {
                /* PEs store a relocatable PDB basename. Search next to this
                 * image, independently of the caller's working directory. */
                WCHAR *slash = wcsrchr(path, L'\\');
                if (slash != NULL) {
                    *slash = 0;
                    SymSetSearchPathW(symbols_process, path);
                    *slash = L'\\';
                }
                const unsigned char *base = (const void *)module;
                const IMAGE_DOS_HEADER *dos = (const void *)base;
                const IMAGE_NT_HEADERS64 *pe = (const void *)(base + dos->e_lfanew);
                SymLoadModuleExW(symbols_process, NULL, path, NULL, (DWORD64)module,
                                 pe->OptionalHeader.SizeOfImage, NULL, 0);
                free(path);
            }
        }
        union {
            SYMBOL_INFOW symbol;
            unsigned char bytes[sizeof(SYMBOL_INFOW) + MAX_SYM_NAME * sizeof(WCHAR)];
        } buffer = {0};
        buffer.symbol.SizeOfStruct = sizeof(SYMBOL_INFOW);
        buffer.symbol.MaxNameLen = MAX_SYM_NAME;
        DWORD64 displacement;
        char *name = NULL, *filename = NULL;
        int line_number = 0;
        if (SymFromAddrW(symbols_process, pc, &displacement, &buffer.symbol))
            name = scoop_trace_utf8(buffer.symbol.Name);
        IMAGEHLP_LINEW64 line = {.SizeOfStruct = sizeof line};
        DWORD line_displacement;
        if (SymGetLineFromAddrW64(symbols_process, pc, &line_displacement, &line)) {
            filename = scoop_trace_utf8(line.FileName);
            line_number = (int)line.LineNumber;
        }
        if (name != NULL || filename != NULL) {
            ScoopTraceSymbol *symbol = scoop_trace_allocate(sizeof *symbol);
            *symbol =
                (ScoopTraceSymbol){.function = name, .filename = filename, .line = line_number};
            symbols->first = symbols->last = symbol;
        }
        if (module != NULL)
            FreeLibrary(module);
        if (generation != scoop_trace_generation()) {
            scoop_trace_symbols_free(symbols);
            symbols->changed = true;
        } else if (symbols->first == NULL) {
            symbols->unavailable = true;
        }
    }
    ReleaseSRWLockExclusive(&symbols_lock);
}
