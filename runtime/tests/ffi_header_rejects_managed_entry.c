#include "scoop_rt.h"

void native_ffi_header_probe(void) {
    /* NativeBorrowedEntry remains available to Scoop-ABI implementations. */
    scoop_runtime_gc_collect();

    /* This ManagedEntry must be absent from the native FFI-author surface. */
    scoop_rt_gc_collect();
}
