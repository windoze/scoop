#include "internal.h"

#include <backtrace.h>
#include <pthread.h>
#include <stdlib.h>

static pthread_once_t symbols_once = PTHREAD_ONCE_INIT;
static struct backtrace_state *symbols_state;
static uint64_t symbols_generation;
static bool symbols_stable;

static void ignore_error(void *data, const char *message, int number) {
    (void)data;
    (void)message;
    (void)number;
}

static int ignore_symbol(void *data, uintptr_t pc, const char *filename, int line,
                         const char *function) {
    (void)data;
    (void)pc;
    (void)filename;
    (void)line;
    (void)function;
    return 0;
}

static void initialize_symbols(void) {
    symbols_generation = scoop_trace_generation();
    symbols_state = backtrace_create_state(NULL, 1, ignore_error, NULL);
    if (symbols_state != NULL) {
        /* Force the process module index to be initialized in this generation.
         * No new state is allocated for individual exceptions or print calls. */
        backtrace_pcinfo(symbols_state, (uintptr_t)initialize_symbols, ignore_symbol,
                         ignore_error, NULL);
    }
    symbols_stable = symbols_generation == scoop_trace_generation();
}

static int collect_symbol(void *data, uintptr_t pc, const char *filename, int line,
                          const char *function) {
    (void)pc;
    ScoopTraceSymbols *symbols = data;
    if (filename == NULL && function == NULL)
        return 0;
    ScoopTraceSymbol *symbol = scoop_trace_allocate(sizeof *symbol);
    *symbol = (ScoopTraceSymbol){
        .filename = filename == NULL ? NULL : scoop_trace_copy_path(filename),
        .function = function == NULL ? NULL : scoop_trace_copy_path(function),
        .line = line,
    };
    if (symbols->last != NULL)
        symbols->last->next = symbol;
    else
        symbols->first = symbol;
    symbols->last = symbol;
    return 0;
}

static void collect_symbol_name(void *data, uintptr_t pc, const char *name,
                                uintptr_t value, uintptr_t size) {
    (void)value;
    (void)size;
    collect_symbol(data, pc, NULL, 0, name);
}

void scoop_trace_symbols_free(ScoopTraceSymbols *symbols) {
    for (ScoopTraceSymbol *symbol = symbols->first; symbol != NULL;) {
        ScoopTraceSymbol *next = symbol->next;
        free(symbol->function);
        free(symbol->filename);
        free(symbol);
        symbol = next;
    }
    symbols->first = symbols->last = NULL;
}

void scoop_trace_symbolize(uint64_t generation, uintptr_t pc, ScoopTraceSymbols *symbols) {
    pthread_once(&symbols_once, initialize_symbols);
    if (!symbols_stable || generation != symbols_generation ||
        generation != scoop_trace_generation()) {
        symbols->changed = true;
        return;
    }
    if (symbols_state == NULL) {
        symbols->unavailable = true;
        return;
    }
    backtrace_pcinfo(symbols_state, pc, collect_symbol, ignore_error, symbols);
    if (symbols->first == NULL)
        backtrace_syminfo(symbols_state, pc, collect_symbol_name, ignore_error, symbols);
    /* A loader transition during lookup invalidates even an apparently valid
     * result. Callback-owned strings have already been copied before return. */
    if (generation != scoop_trace_generation()) {
        scoop_trace_symbols_free(symbols);
        symbols->changed = true;
    } else if (symbols->first == NULL) {
        symbols->unavailable = true;
    }
}
